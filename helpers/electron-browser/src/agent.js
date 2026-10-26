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

/**
 * Clicks at a place in the page, as the reader would.
 * @param {Page} page The page.
 * @param {number} x Across, in logical pixels.
 * @param {number} y Down, in logical pixels.
 * @returns {ToolOutcome} The answer.
 */
function clickAt(page, x, y) {
  const at = { x: Math.round(x), y: Math.round(y) };
  page.contents.sendInputEvent({ type: 'mouseMove', ...at, modifiers: [] });
  page.contents.sendInputEvent({ type: 'mouseDown', ...at, button: 'left', clickCount: 1, modifiers: [] });
  page.contents.sendInputEvent({ type: 'mouseUp', ...at, button: 'left', clickCount: 1, modifiers: [] });
  return done(`Clicked at ${at.x}, ${at.y}.`);
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
 * Opens an address and ends when the page has loaded, fails to, or takes too long.
 * @param {Page} page The page.
 * @param {string} url Where to go.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function navigate(page, url) {
  if (!mayOpen(url)) return failed('Only http and https addresses can be opened.');
  /** @type {Promise<ToolOutcome>} */
  const late = new Promise((resolve) => setTimeout(() => resolve(failed('The page took too long to load.')), LOADING));
  /** @type {Promise<ToolOutcome>} */
  const load = page.contents.loadURL(url).then(
    () => done('The page loaded.'),
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
        `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' }); el.scrollIntoView({ block: 'center' }); el.focus(); if (el.select) el.select(); return JSON.stringify({ ok: true }); })()`,
      );
      if (found.error) return failed(found.error);
      await protocol(page).call('Input.insertText', { text: args.text });
      if (args.submit === true) pressEnter(page);
      return done('Typed.');
    }
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
