'use strict';

/**
 * The tools agents call against a page: looking at it, reading it, and acting on it, all through
 * the page's DevTools protocol.
 */

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */

/** The most characters of a tool's text that are given back. */
const MOST_TEXT = 20000;

/** How long loading a page may take, in milliseconds. */
const LOADING = 20000;

/** How long the network must be still for a page to count as settled, in milliseconds. */
const STILL = 500;

/** The longest a page is waited for to settle after it loaded, in milliseconds. */
const SETTLING = 5000;

/** How often the network is looked at while waiting for a page to settle, in milliseconds. */
const LOOK = 50;

/** How long a click is given to start something, such as a request or a page, before the network is looked at, in milliseconds. */
const CLICK_REACTS = 150;

/** How long the network must be still after a click, in milliseconds. */
const CLICK_STILL = 250;

/** The longest a click waits for what it started, in milliseconds. */
const CLICK_SETTLING = 3000;

/** The name of the scripting world the tools read pages in. */
const WORLD = 'pm-agent';

/** The script that lists what can be clicked or typed in, and what the page says. */
const SNAPSHOT = `(() => {
  const wanted = 'a,button,input,select,textarea,summary,[role=button],[role=link],[role=checkbox],[role=tab],[role=menuitem],[onclick],[contenteditable=""],[contenteditable=true],h1,h2,h3,h4,label';
  const seen = [];
  const lines = [];
  const visible = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none';
  };
  for (const el of document.querySelectorAll(wanted)) {
    if (!visible(el)) continue;
    const text = (el.innerText || el.value || el.getAttribute('aria-label') || el.placeholder || el.title || '')
      .trim().replace(/\\s+/g, ' ').slice(0, 80);
    const type = el.type ? ':' + el.type : '';
    seen.push(el);
    lines.push('[' + (seen.length - 1) + '] ' + el.tagName.toLowerCase() + type + (text ? ' "' + text + '"' : '') + (el.id ? ' #' + el.id : ''));
    if (seen.length >= 300) break;
  }
  window.__pmRefs = seen;
  return JSON.stringify({
    url: location.href,
    title: document.title,
    text: (document.body ? document.body.innerText : '').trim().slice(0, 3000),
    lines,
  });
})()`;

/**
 * A tool's answer: it worked.
 * @param {string} text What it says.
 * @param {string | null} [image] Where a picture it took was saved, when it took one.
 * @returns {ToolOutcome} The answer.
 */
function done(text, image = null) {
  return { ok: true, text, image };
}

/**
 * A tool's answer: it failed.
 * @param {string} text Why.
 * @returns {ToolOutcome} The answer.
 */
function failed(text) {
  return { ok: false, text, image: null };
}

/**
 * Text cut to at most `most` characters.
 * @param {string} text The text.
 * @param {number} most The most characters to keep.
 * @returns {string} The text, with a mark where it was cut.
 */
function cut(text, most) {
  return text.length > most ? `${text.slice(0, most)}\n[cut]` : text;
}

/**
 * A JavaScript expression that finds the element the arguments name.
 * @param {Json} args What the agent gave the tool.
 * @returns {string | null} The expression, or nothing when the arguments name no element.
 */
function target(args) {
  if (Number.isInteger(args.ref) && args.ref >= 0) return `(window.__pmRefs || [])[${args.ref}]`;
  if (typeof args.selector === 'string') return `document.querySelector(${JSON.stringify(args.selector)})`;
  return null;
}

/**
 * Runs `expression` in a scripting context.
 * @param {Page} page The page, which must have a protocol to speak.
 * @param {number | null} context The scripting context, or nothing for the page's own.
 * @param {string} expression The script.
 * @returns {Promise<any>} The value the script gave; rejects with the script's failure.
 */
async function evaluate(page, context, expression) {
  /** @type {Json} */
  const params = { expression, returnByValue: true, awaitPromise: true };
  if (context !== null) params.contextId = context;
  const reply = await protocol(page).call('Runtime.evaluate', params);
  if (reply.exceptionDetails) {
    const details = reply.exceptionDetails;
    throw new Error((details.exception && details.exception.description) || details.text || 'the script failed');
  }
  return reply.result.value;
}

/**
 * The page's protocol.
 * @param {Page} page The page.
 * @returns {import('./cdp').Cdp} Its protocol; throws when it has none.
 */
function protocol(page) {
  if (!page.cdp) throw new Error('the page has no protocol');
  return page.cdp;
}

/**
 * The scripting context the tools read the page in.
 * @param {Page} page The page.
 * @returns {Promise<number>} The context; rejects when the page has no frame or cannot be read.
 */
async function world(page) {
  const tree = await protocol(page).call('Page.getFrameTree').catch(() => null);
  if (!tree) throw new Error('the page has no frame yet');
  const made = await protocol(page)
    .call('Page.createIsolatedWorld', {
      frameId: tree.frameTree.frame.id,
      worldName: WORLD,
      grantUniveralAccess: false,
    })
    .catch(() => null);
  if (!made) throw new Error('the page could not be read');
  return made.executionContextId;
}

/**
 * Runs `script` in the tools' world and parses the JSON text it returns.
 * @param {Page} page The page.
 * @param {string} script A script that returns JSON text.
 * @returns {Promise<Json>} What it said, or nothing when it said something that is not JSON.
 */
async function inWorld(page, script) {
  const value = await evaluate(page, await world(page), script);
  try {
    return JSON.parse(value);
  } catch (_) {
    return {};
  }
}

/** The script that says what is at a point of the page, as a tag, an id and the start of its text. */
const DESCRIBE = `(x, y) => {
    const el = document.elementFromPoint(x, y);
    if (!el) return 'nothing';
    const text = (el.innerText || el.value || el.getAttribute('aria-label') || '').trim().replace(/\\s+/g, ' ').slice(0, 50);
    return el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + (text ? ' "' + text + '"' : '');
  }`;

/**
 * Clicks at a place in the page, as the reader would, and waits for what the click started: a page that loads, or requests that are under way.
 * @param {Page} page The page.
 * @param {number} x Across, in logical pixels.
 * @param {number} y Down, in logical pixels.
 * @returns {Promise<ToolOutcome>} The answer, saying what was hit and where the page is after.
 */
async function clickAt(page, x, y) {
  const at = { x: Math.round(x), y: Math.round(y) };
  const hit = await evaluate(page, await world(page), `(${DESCRIBE})(${at.x}, ${at.y})`).catch(() => 'unknown');
  page.contents.sendInputEvent({ type: 'mouseMove', ...at, modifiers: [] });
  page.contents.sendInputEvent({ type: 'mouseDown', ...at, button: 'left', clickCount: 1, modifiers: [] });
  page.contents.sendInputEvent({ type: 'mouseUp', ...at, button: 'left', clickCount: 1, modifiers: [] });
  await new Promise((resolve) => setTimeout(resolve, CLICK_REACTS));
  await settled(page, CLICK_STILL, CLICK_SETTLING);
  return done(`Clicked ${hit} at ${at.x}, ${at.y}. The page is at ${page.contents.getURL()}.`);
}

/**
 * Presses Enter in the page.
 * @param {Page} page The page.
 * @returns {void}
 */
function pressEnter(page) {
  page.contents.sendInputEvent({ type: 'keyDown', keyCode: 'Enter' });
  page.contents.sendInputEvent({ type: 'char', keyCode: 'Enter' });
  page.contents.sendInputEvent({ type: 'keyUp', keyCode: 'Enter' });
}

/**
 * The names Electron gives keys that the web names differently.
 * @type {Readonly<Record<string, string>>}
 */
const KEY_NAMES = {
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Esc: 'Escape',
  Return: 'Enter',
  ' ': 'Space',
  Del: 'Delete',
};

/** The modifiers a key press may hold. */
const KEY_MODIFIERS = ['shift', 'control', 'alt', 'meta'];

/**
 * Presses a key in the page, with the modifiers held.
 * @param {Page} page The page.
 * @param {string} key The key, such as `Escape`, `Tab`, `ArrowDown`, `F5` or `a`.
 * @param {Json} held The modifiers held: some of `shift`, `control`, `alt` and `meta`.
 * @returns {ToolOutcome} The answer.
 */
function pressKey(page, key, held) {
  const modifiers = /** @type {Array<'shift' | 'control' | 'alt' | 'meta'>} */ (
    (Array.isArray(held) ? held : []).filter((name) => KEY_MODIFIERS.includes(name))
  );
  const keyCode = KEY_NAMES[key] || (key.length === 1 ? key.toUpperCase() : key);
  page.contents.sendInputEvent({ type: 'keyDown', keyCode, modifiers });
  if (key.length === 1 && !modifiers.includes('control') && !modifiers.includes('meta')) {
    page.contents.sendInputEvent({ type: 'char', keyCode: key, modifiers });
  }
  page.contents.sendInputEvent({ type: 'keyUp', keyCode, modifiers });
  return done(`Pressed ${[...modifiers, key].join('+')}.`);
}

/**
 * Runs `act`, which moves through the page's history or loads it again, and ends when the page has loaded and settled, or a page of the same document took its place, or it fails or takes too long.
 * A move between two states of the same document, which single-page apps make, never stops loading, so it counts as arriving.
 * @param {Page} page The page.
 * @param {() => void} act What to do.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function loading(page, act) {
  /** @type {Promise<ToolOutcome>} */
  const late = new Promise((resolve) => setTimeout(() => resolve(failed('The page took too long to load.')), LOADING));
  const since = Date.now();
  /** @type {Promise<ToolOutcome>} */
  const arriving = new Promise((resolve) => {
    /** @type {() => void} */
    const arrive = () => {
      page.contents.off('did-stop-loading', arrive);
      page.contents.off('did-navigate-in-page', arrive);
      page.contents.off('did-fail-load', fail);
      settled(page, STILL, SETTLING).then(() => resolve(done(arrived(page, since))));
    };
    /** @type {(event: unknown, code: number, reason: string, address: string, main: boolean) => void} */
    const fail = (_event, code, reason, address, main) => {
      if (!main || code === -3) return;
      page.contents.off('did-stop-loading', arrive);
      page.contents.off('did-navigate-in-page', arrive);
      page.contents.off('did-fail-load', fail);
      resolve(failed(`The page did not load: ${reason} (${address})`));
    };
    page.contents.once('did-stop-loading', arrive);
    page.contents.once('did-navigate-in-page', arrive);
    page.contents.on('did-fail-load', fail);
  });
  act();
  return Promise.race([arriving, late]);
}

/**
 * Whether an agent may send the page to an address: a web address or the blank page, never a local file or an internal page.
 * @param {string} url The address.
 * @returns {boolean} Whether it may.
 */
function mayOpen(url) {
  if (url === 'about:blank') return true;
  try {
    const { protocol: scheme } = new URL(url);
    return scheme === 'http:' || scheme === 'https:';
  } catch (_) {
    return false;
  }
}

/**
 * Waits for the page to settle after it loaded: nothing is on its way and the network has been still for `quiet` milliseconds, or `most` milliseconds have passed.
 * @param {Page} page The page.
 * @param {number} quiet How long the network must be still.
 * @param {number} most The longest to wait.
 * @returns {Promise<void>} Ends when it has settled or the time is up.
 */
async function settled(page, quiet, most) {
  if (!page.recorder) return;
  const until = Date.now() + most;
  while (!page.recorder.still(quiet) && Date.now() < until) {
    await new Promise((resolve) => setTimeout(resolve, LOOK));
  }
}

/**
 * What a page that loaded looks like: where it is, what it is called and what its document answered, when it answered since `since`; a page the engine kept from before, as going back or forward can, says it came from the cache.
 * @param {Page} page The page.
 * @param {number} since When the move began, in milliseconds.
 * @returns {string} The sentence.
 */
function arrived(page, since) {
  const answer = page.recorder && page.recorder.document;
  const fresh = answer && answer.url === page.contents.getURL() && answer.at >= since;
  const status =
    fresh && answer && !answer.cached
      ? ` (HTTP ${answer.status})`
      : ' (not fetched again, so the status is not known)';
  return `The page loaded: ${page.contents.getURL()}${status}, titled "${page.contents.getTitle()}".`;
}

/**
 * Whether the page may move `step` places through its history, or load again when `step` is nothing: the page it would show is one an agent may open.
 * @param {Page} page The page.
 * @param {number} step How far through the history, such as -1 for back; 0 for the page it shows.
 * @returns {boolean} Whether it may.
 */
function mayVisit(page, step) {
  const history = page.contents.navigationHistory;
  const entry = history.getEntryAtIndex(history.getActiveIndex() + step);
  return entry ? mayOpen(entry.url) : false;
}

/**
 * Opens an address and ends when the page has loaded and settled, fails to, or takes too long.
 * @param {Page} page The page.
 * @param {string} url Where to go.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function navigate(page, url) {
  if (!mayOpen(url)) return failed('Only http and https addresses can be opened.');
  /** @type {Promise<ToolOutcome>} */
  const late = new Promise((resolve) => setTimeout(() => resolve(failed('The page took too long to load.')), LOADING));
  const since = Date.now();
  /** @type {Promise<ToolOutcome>} */
  const load = page.contents.loadURL(url).then(
    async () => {
      await settled(page, STILL, SETTLING);
      return done(arrived(page, since));
    },
    (/** @type {Error} */ error) => failed(`The page did not load: ${error.message}`),
  );
  return Promise.race([load, late]);
}

/**
 * Runs a tool against a page.
 * @param {Page} page The page.
 * @param {string} tool The tool's name, such as `browser_click`.
 * @param {Json} args What the agent gave it.
 * @returns {Promise<ToolOutcome>} What it came to.
 */
async function run(page, tool, args) {
  switch (tool) {
    case 'browser_navigate':
      return typeof args.url === 'string' ? navigate(page, args.url) : failed('no url was given');
    case 'browser_snapshot': {
      const found = await inWorld(page, SNAPSHOT);
      const lines = Array.isArray(found.lines) ? found.lines : [];
      const text = `URL: ${found.url || ''}\nTitle: ${found.title || ''}\n\nElements:\n${lines.join('\n')}\n\nText:\n${found.text || ''}`;
      return done(cut(text, MOST_TEXT));
    }
    case 'browser_screenshot': {
      await settled(page, 0, SETTLING / 2);
      const reply = await protocol(page).call('Page.captureScreenshot', { format: 'png' }).catch(() => null);
      if (!reply || !reply.data) return failed('no picture could be taken');
      return done('Screenshot taken.', page.surface.savePicture(Buffer.from(reply.data, 'base64')));
    }
    case 'browser_click': {
      if (typeof args.x === 'number' && typeof args.y === 'number') return clickAt(page, args.x, args.y);
      const element = target(args);
      if (!element) return failed('name an element with ref or selector, or give x and y');
      const found = await inWorld(
        page,
        `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' }); el.scrollIntoView({ block: 'center', inline: 'center' }); const r = el.getBoundingClientRect(); return JSON.stringify({ x: r.x + r.width / 2, y: r.y + r.height / 2 }); })()`,
      );
      if (typeof found.x === 'number' && typeof found.y === 'number') return clickAt(page, found.x, found.y);
      return failed(found.error || 'the element could not be found');
    }
    case 'browser_type': {
      const element = target(args);
      if (typeof args.text !== 'string' || !element) return failed('give text, and an element by ref or selector');
      const found = await inWorld(
        page,
        `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' }); const kind = el.tagName === 'SELECT' ? 'list' : el.type === 'checkbox' || el.type === 'radio' ? 'box' : ''; if (kind) return JSON.stringify({ kind }); el.scrollIntoView({ block: 'center' }); el.focus(); if (el.select) el.select(); return JSON.stringify({ ok: true }); })()`,
      );
      if (found.error) return failed(found.error);
      if (found.kind) return failed(`A ${found.kind} cannot be typed in; use browser_select_option for a list and browser_click for a box.`);
      await protocol(page).call('Input.insertText', { text: args.text });
      if (args.submit === true) pressEnter(page);
      return done('Typed.');
    }
    case 'browser_select_option': {
      const element = target(args);
      if (!element || typeof args.value !== 'string') return failed('give the option as value, and the list by ref or selector');
      const found = await inWorld(
        page,
        `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' }); if (el.tagName !== 'SELECT') return JSON.stringify({ error: 'the element is not a list' }); const want = ${JSON.stringify(args.value)}; const option = [...el.options].find((o) => o.value === want) || [...el.options].find((o) => o.text.trim() === want); if (!option) return JSON.stringify({ error: 'the list has no such option; it has: ' + [...el.options].map((o) => (o.text.trim() && o.text.trim() !== o.value ? o.value + ' (' + o.text.trim() + ')' : o.value)).join(', ') }); el.value = option.value; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); return JSON.stringify({ chosen: option.text.trim() || option.value }); })()`,
      );
      if (found.error) return failed(found.error);
      return done(`Chose ${found.chosen}.`);
    }
    case 'browser_press_key': {
      if (typeof args.key !== 'string' || args.key === '') return failed('no key was given');
      const element = target(args);
      if (element) {
        const found = await inWorld(
          page,
          `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' }); el.focus(); return JSON.stringify({ ok: true }); })()`,
        );
        if (found.error) return failed(found.error);
      }
      return pressKey(page, args.key, args.modifiers);
    }
    case 'browser_scroll': {
      const element = target(args);
      const direction = typeof args.direction === 'string' ? args.direction : '';
      const amount = Number.isFinite(args.amount) ? Number(args.amount) : 0;
      const found = await inWorld(
        page,
        `(() => {
          const el = ${element || 'null'};
          const named = ${JSON.stringify(direction)};
          if (${element ? 'true' : 'false'} && !el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' });
          if (el && !named) {
            el.scrollIntoView({ block: 'center', inline: 'center' });
          } else {
            const box = el || document.scrollingElement || document.documentElement;
            const by = ${amount} || (named === 'left' || named === 'right' ? box.clientWidth : box.clientHeight) * 0.9;
            if (named === 'top') box.scrollTo({ top: 0 });
            else if (named === 'bottom') box.scrollTo({ top: box.scrollHeight });
            else if (named === 'up') box.scrollBy({ top: -by });
            else if (named === 'left') box.scrollBy({ left: -by });
            else if (named === 'right') box.scrollBy({ left: by });
            else box.scrollBy({ top: by });
          }
          const root = el && named ? el : document.scrollingElement || document.documentElement;
          return JSON.stringify({ top: Math.round(root.scrollTop), most: Math.max(0, root.scrollHeight - root.clientHeight), height: root.scrollHeight });
        })()`,
      );
      if (found.error) return failed(found.error);
      return done(`Scrolled to ${found.top} of at most ${found.most} (the page is ${found.height} tall).`);
    }
    case 'browser_back': {
      if (!page.contents.navigationHistory.canGoBack()) return failed('There is no earlier page.');
      if (!mayVisit(page, -1)) return failed('Only http and https addresses can be opened.');
      return loading(page, () => page.contents.navigationHistory.goBack());
    }
    case 'browser_forward': {
      if (!page.contents.navigationHistory.canGoForward()) return failed('There is no later page.');
      if (!mayVisit(page, 1)) return failed('Only http and https addresses can be opened.');
      return loading(page, () => page.contents.navigationHistory.goForward());
    }
    case 'browser_reload':
      if (!mayVisit(page, 0)) return failed('Only http and https addresses can be opened.');
      return loading(page, () => page.contents.reload());
    case 'browser_evaluate': {
      if (typeof args.expression !== 'string') return failed('no expression was given');
      const value = await evaluate(page, null, args.expression);
      return done(cut(JSON.stringify(value === undefined ? null : value), MOST_TEXT));
    }
    case 'browser_console':
      return done(recorder(page).consoleText('The console is empty.'));
    case 'browser_network':
      return done(recorder(page).networkText('No requests were made.'));
    case 'browser_wait': {
      const timeout = Math.min(Number.isFinite(args.timeout_ms) ? args.timeout_ms : 10000, 30000);
      let condition = 'false';
      if (typeof args.text === 'string') {
        condition = `(document.body ? document.body.innerText : '').includes(${JSON.stringify(args.text)})`;
      } else if (typeof args.selector === 'string') {
        condition = `!!document.querySelector(${JSON.stringify(args.selector)})`;
      }
      const script = `new Promise((resolve) => { const start = Date.now(); const tick = () => { if (${condition}) resolve(true); else if (Date.now() - start > ${timeout}) resolve(false); else setTimeout(tick, 100); }; tick(); })`;
      const found = await evaluate(page, await world(page), script);
      return found === true ? done('Found.') : failed('Timed out.');
    }
    default:
      return failed(`there is no tool ${tool}`);
  }
}

/**
 * The page's recorder.
 * @param {Page} page The page.
 * @returns {import('./recorder').Recorder} Its recorder; throws when it has none.
 */
function recorder(page) {
  if (!page.recorder) throw new Error('the page has no recorder');
  return page.recorder;
}

module.exports = { run };
