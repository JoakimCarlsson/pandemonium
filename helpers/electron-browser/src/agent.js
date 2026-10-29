'use strict';

/**
 * The tools agents call against a page: looking at it, reading it, and acting on it, all through
 * the page's DevTools protocol.
 */

const origins = require('./origins');

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
 * Whether an agent may send the page to an address: a web address or the blank page, never a local file or an internal page, and, when the reader restricted agents, only a local host or an origin they listed.
 * @param {string} url The address.
 * @returns {string | null} Why it may not, or nothing when it may.
 */
function mayOpen(url) {
  return origins.mayOpen(url);
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
 * @returns {string | null} Why it may not, or nothing when it may.
 */
function mayVisit(page, step) {
  const history = page.contents.navigationHistory;
  const entry = history.getEntryAtIndex(history.getActiveIndex() + step);
  return entry ? mayOpen(entry.url) : 'There is no such page in the history.';
}

/**
 * Opens an address and ends when the page has loaded and settled, fails to, or takes too long.
 * @param {Page} page The page.
 * @param {string} url Where to go.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function navigate(page, url) {
  const refusal = mayOpen(url);
  if (refusal) return failed(refusal);
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

/** The text a script says when an element is gone: references go stale when the page changes. */
const NO_ELEMENT = 'no such element; take a new browser_snapshot';

/**
 * A script that measures an element.
 * @param {string} element An expression that finds it.
 * @param {boolean} scroll Whether to bring it to the middle of the view first.
 * @returns {string} The script, which says its box as JSON, or an error.
 */
function measure(element, scroll) {
  return `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: ${JSON.stringify(NO_ELEMENT)} }); ${scroll ? "el.scrollIntoView({ block: 'center', inline: 'center' });" : ''} const r = el.getBoundingClientRect(); return JSON.stringify({ x: r.x, y: r.y, width: r.width, height: r.height, scrollX: window.scrollX, scrollY: window.scrollY, viewportWidth: window.innerWidth, viewportHeight: window.innerHeight }); })()`;
}

/**
 * The place an agent means: the middle of an element, or a point.
 * @param {Page} page The page.
 * @param {Json} spec A `ref` or `selector`, or `x` and `y`.
 * @returns {Promise<{ x: number, y: number } | ToolOutcome>} The point in the page's view, or why there is none.
 */
async function pointOf(page, spec) {
  if (typeof spec.x === 'number' && typeof spec.y === 'number') return { x: spec.x, y: spec.y };
  const element = target(spec);
  if (!element) return failed('name an element with ref or selector, or give x and y');
  const found = await inWorld(page, measure(element, true));
  if (typeof found.x !== 'number') return failed(found.error || 'the element could not be found');
  return { x: found.x + found.width / 2, y: found.y + found.height / 2 };
}

/**
 * Says what is at a point of the page.
 * @param {Page} page The page.
 * @param {{ x: number, y: number }} at The point.
 * @returns {Promise<string>} A tag, id and some text, or `unknown`.
 */
function describeAt(page, at) {
  return world(page)
    .then((context) => evaluate(page, context, `(${DESCRIBE})(${Math.round(at.x)}, ${Math.round(at.y)})`))
    .catch(() => 'unknown');
}

/**
 * Waits for `ms` milliseconds.
 * @param {number} ms How long.
 * @returns {Promise<void>} Resolves when it has passed.
 */
function pause(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** How long the page is given to react to the pointer arriving, in milliseconds. */
const HOVER_REACTS = 200;

/** How many steps the pointer takes between the two ends of a drag. */
const DRAG_STEPS = 8;

/**
 * Moves the pointer over an element or a point, which makes the page show what hovering shows, such as menus and tooltips.
 * @param {Page} page The page.
 * @param {Json} args `ref` or `selector`, or `x` and `y`.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function hover(page, args) {
  const at = await pointOf(page, args);
  if ('ok' in at) return at;
  const spot = { x: Math.round(at.x), y: Math.round(at.y) };
  const hit = await describeAt(page, spot);
  page.contents.sendInputEvent({ type: 'mouseMove', ...spot, modifiers: [] });
  await pause(HOVER_REACTS);
  return done(`Hovering ${hit} at ${spot.x}, ${spot.y}.`);
}

/**
 * Presses at one place, moves to another and lets go, as the reader dragging would.
 * Both mouse-driven drags, such as sliders and sortable lists, and HTML drag and drop, which the engine takes over, are carried out.
 * @param {Page} page The page.
 * @param {Json} args `from` and `to`, each a `ref` or `selector`, or `x` and `y`.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function drag(page, args) {
  const ends = [args.from, args.to];
  if (!ends.every((end) => end && typeof end === 'object')) return failed('give from and to, each a ref or selector, or x and y');
  const start = await pointOf(page, ends[0]);
  if ('ok' in start) return failed(`from: ${start.text}`);
  const finish = await pointOf(page, ends[1]);
  if ('ok' in finish) return failed(`to: ${finish.text}`);
  const cdp = protocol(page);
  const fromHit = await describeAt(page, start);
  const toHit = await describeAt(page, finish);
  /** @type {Json | null} */
  let data = null;
  /** @type {import('./cdp').EventListener} */
  const hear = (method, params) => {
    if (method === 'Input.dragIntercepted') data = params.data;
  };
  cdp.listen(hear);
  /** @type {(type: string, at: { x: number, y: number }, held: boolean) => Promise<Json>} */
  const mouse = (type, at, held) =>
    cdp.call('Input.dispatchMouseEvent', {
      type,
      x: at.x,
      y: at.y,
      button: held || type === 'mouseReleased' ? 'left' : 'none',
      buttons: held ? 1 : 0,
      clickCount: type === 'mouseMoved' ? 0 : 1,
    });
  try {
    await cdp.call('Input.setInterceptDrags', { enabled: true });
    await mouse('mouseMoved', start, false);
    await mouse('mousePressed', start, true);
    for (let step = 1; step <= DRAG_STEPS; step += 1) {
      const along = step / DRAG_STEPS;
      await mouse('mouseMoved', { x: start.x + (finish.x - start.x) * along, y: start.y + (finish.y - start.y) * along }, true);
      await pause(16);
    }
    if (data) {
      for (const type of ['dragEnter', 'dragOver', 'drop']) {
        await cdp.call('Input.dispatchDragEvent', { type, x: finish.x, y: finish.y, data });
      }
    }
    await mouse('mouseReleased', finish, false);
  } finally {
    cdp.unlisten(hear);
    await cdp.call('Input.setInterceptDrags', { enabled: false }).catch(() => {});
  }
  await settled(page, CLICK_STILL, CLICK_SETTLING);
  return done(`Dragged ${fromHit} to ${toHit}${data ? ' as an HTML drag and drop' : ''}. The page is at ${page.contents.getURL()}.`);
}

/**
 * Types text into a text field.
 * @param {Page} page The page.
 * @param {Json} args `ref` or `selector`, `text`, and `submit` to press Enter afterwards.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function typeInto(page, args) {
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

/**
 * Chooses an option of a list.
 * @param {Page} page The page.
 * @param {Json} args `ref` or `selector` for the list, and the option's `value` or text.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function selectOption(page, args) {
  const element = target(args);
  if (!element || typeof args.value !== 'string') return failed('give the option as value, and the list by ref or selector');
  const found = await inWorld(
    page,
    `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: 'no such element; take a new browser_snapshot' }); if (el.tagName !== 'SELECT') return JSON.stringify({ error: 'the element is not a list' }); const want = ${JSON.stringify(args.value)}; const option = [...el.options].find((o) => o.value === want) || [...el.options].find((o) => o.text.trim() === want); if (!option) return JSON.stringify({ error: 'the list has no such option; it has: ' + [...el.options].map((o) => (o.text.trim() && o.text.trim() !== o.value ? o.value + ' (' + o.text.trim() + ')' : o.value)).join(', ') }); el.value = option.value; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); return JSON.stringify({ chosen: option.text.trim() || option.value }); })()`,
  );
  if (found.error) return failed(found.error);
  return done(`Chose ${found.chosen}.`);
}

/**
 * Ticks or clears a checkbox, or chooses a radio button, by clicking it only when it is not as wanted.
 * @param {Page} page The page.
 * @param {Json} args `ref` or `selector` for the box.
 * @param {boolean} checked Whether it should end up ticked.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function setBox(page, args, checked) {
  const element = target(args);
  if (!element) return failed('name the box by ref or selector');
  const found = await inWorld(
    page,
    `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: ${JSON.stringify(NO_ELEMENT)} }); if (el.type !== 'checkbox' && el.type !== 'radio') return JSON.stringify({ error: 'the element is not a checkbox or a radio button' }); const want = ${checked}; if (el.checked !== want) { if (el.type === 'radio' && !want) return JSON.stringify({ error: 'a radio button cannot be cleared; choose another of its group' }); el.scrollIntoView({ block: 'center' }); el.click(); } return JSON.stringify({ checked: el.checked }); })()`,
  );
  if (found.error) return failed(found.error);
  return done(found.checked ? 'Ticked.' : 'Cleared.');
}

/** The most fields `browser_fill_form` fills in one call. */
const MOST_FIELDS = 50;

/**
 * Fills in several fields of a form in order, each as `browser_type`, `browser_select_option` or a click on a box would, and stops at the first that fails.
 * @param {Page} page The page.
 * @param {Json} args `fields`: each with `ref` or `selector` and a `value`, text for a text field or a list's option, true or false for a box.
 * @returns {Promise<ToolOutcome>} The answer, saying what each field came to.
 */
async function fillForm(page, args) {
  const fields = args.fields;
  if (!Array.isArray(fields) || fields.length === 0) return failed('give fields: a list of { ref or selector, value }');
  if (fields.length > MOST_FIELDS) return failed(`give at most ${MOST_FIELDS} fields in one call`);
  /** @type {string[]} */
  const lines = [];
  for (const [at, field] of fields.entries()) {
    const label = `field ${at + 1}`;
    const before = lines.length ? `The ${lines.length} before it were filled: ${lines.join(' ')}` : 'None before it was filled.';
    if (!field || typeof field !== 'object') return failed(`${label} is not an object. ${before}`);
    const element = target(field);
    if (!element) return failed(`${label}: give the field by ref or selector. ${before}`);
    const found = await inWorld(
      page,
      `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: ${JSON.stringify(NO_ELEMENT)} }); return JSON.stringify({ kind: el.tagName === 'SELECT' ? 'list' : el.type === 'checkbox' || el.type === 'radio' ? 'box' : 'text' }); })()`,
    );
    /** @type {ToolOutcome} */
    let outcome;
    if (found.error) {
      outcome = failed(found.error);
    } else if (found.kind === 'box') {
      outcome = typeof field.value === 'boolean' ? await setBox(page, field, field.value) : failed('A box takes true or false as its value.');
    } else if (typeof field.value === 'boolean') {
      outcome = failed(`A ${found.kind === 'list' ? 'list' : 'text field'} takes a string as its value, not true or false.`);
    } else if (found.kind === 'list') {
      outcome = await selectOption(page, { ...field, value: String(field.value) });
    } else {
      outcome = await typeInto(page, { ...field, text: String(field.value ?? ''), submit: false });
    }
    if (!outcome.ok) return failed(`${label} failed: ${outcome.text} ${before}`);
    lines.push(`${label}: ${outcome.text}`);
  }
  return done(`Filled ${lines.length} fields. ${lines.join(' ')}`);
}

/** The default time a highlight stays, in milliseconds. */
const HIGHLIGHT_FOR = 2000;

/**
 * Draws an outline round an element for a moment, so that the reader sees which one is meant.
 * @param {Page} page The page.
 * @param {Json} args `ref` or `selector`, `duration_ms` and an optional `label`.
 * @returns {Promise<ToolOutcome>} The answer, with the element's box.
 */
async function highlight(page, args) {
  const element = target(args);
  if (!element) return failed('name an element with ref or selector');
  const wanted = Number.isFinite(args.duration_ms) ? Number(args.duration_ms) : HIGHLIGHT_FOR;
  const lasts = Math.min(Math.max(wanted, 200), 10000);
  const label = typeof args.label === 'string' ? args.label.slice(0, 80) : '';
  const found = await inWorld(
    page,
    `(() => { const el = ${element}; if (!el) return JSON.stringify({ error: ${JSON.stringify(NO_ELEMENT)} }); el.scrollIntoView({ block: 'center', inline: 'center' }); const r = el.getBoundingClientRect(); const old = document.getElementById('__pm_highlight'); if (old) old.remove(); const box = document.createElement('div'); box.id = '__pm_highlight'; box.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;box-sizing:border-box;border:3px solid #ff2d95;border-radius:4px;background:rgba(255,45,149,0.12);left:' + r.left + 'px;top:' + r.top + 'px;width:' + r.width + 'px;height:' + r.height + 'px'; const text = ${JSON.stringify(label)}; if (text) { const tag = document.createElement('span'); tag.textContent = text; tag.style.cssText = 'position:absolute;left:-3px;bottom:100%;background:#ff2d95;color:#fff;font:12px sans-serif;padding:2px 6px;white-space:nowrap'; box.appendChild(tag); } document.documentElement.appendChild(box); setTimeout(() => box.remove(), ${lasts}); return JSON.stringify({ x: r.x, y: r.y, width: r.width, height: r.height }); })()`,
  );
  if (found.error) return failed(found.error);
  return done(`Highlighted the element for ${lasts / 1000} seconds: ${found.width} by ${found.height} at ${Math.round(found.x)}, ${Math.round(found.y)}.`);
}

/**
 * Says where an element is, without moving the page.
 * @param {Page} page The page.
 * @param {Json} args `ref` or `selector`.
 * @returns {Promise<ToolOutcome>} The box as JSON: `x` and `y` from the top left of the view, `pageX` and `pageY` from the top left of the page, `width`, `height` and whether it is `inViewport`.
 */
async function boundingBox(page, args) {
  const element = target(args);
  if (!element) return failed('name an element with ref or selector');
  const found = await inWorld(page, measure(element, false));
  if (found.error) return failed(found.error);
  const round = (/** @type {number} */ value) => Math.round(value * 100) / 100;
  const seen = found.x < found.viewportWidth && found.y < found.viewportHeight && found.x + found.width > 0 && found.y + found.height > 0;
  return done(
    JSON.stringify({
      x: round(found.x),
      y: round(found.y),
      width: round(found.width),
      height: round(found.height),
      pageX: round(found.x + found.scrollX),
      pageY: round(found.y + found.scrollY),
      inViewport: seen && found.width > 0 && found.height > 0,
    }),
  );
}

/** The tallest a full-page picture is, in pixels; the engine cannot make taller ones. */
const MOST_PICTURE_HEIGHT = 16000;

/**
 * Takes a picture of the view, of the whole page, or of one element.
 * @param {Page} page The page.
 * @param {Json} args `full_page`, or `ref` or `selector`; not both.
 * @returns {Promise<ToolOutcome>} The answer, with the picture.
 */
async function screenshot(page, args) {
  const element = target(args);
  if (args.full_page === true && element) return failed('full_page and an element (ref or selector) cannot be combined; give one.');
  await settled(page, 0, SETTLING / 2);
  /** @type {Json} */
  const params = { format: 'png' };
  let said = 'Screenshot taken.';
  if (element) {
    const found = await inWorld(page, measure(element, false));
    if (found.error) return failed(found.error);
    if (!(found.width > 0 && found.height > 0)) return failed('the element has no size on the page, so there is nothing to take a picture of');
    params.clip = { x: found.x + found.scrollX, y: found.y + found.scrollY, width: found.width, height: found.height, scale: 1 };
    params.captureBeyondViewport = true;
    said = 'Screenshot of the element taken.';
  } else if (args.full_page === true) {
    const metrics = await protocol(page).call('Page.getLayoutMetrics').catch(() => null);
    const size = metrics && (metrics.cssContentSize || metrics.contentSize);
    if (!size) return failed('the size of the page could not be read');
    const tall = Math.ceil(size.height);
    const height = Math.min(tall, MOST_PICTURE_HEIGHT);
    params.clip = { x: 0, y: 0, width: Math.ceil(size.width), height, scale: 1 };
    params.captureBeyondViewport = true;
    said = height < tall ? `Screenshot of the page taken, cut at ${height} of ${tall} pixels.` : 'Screenshot of the whole page taken.';
  }
  const reply = await protocol(page).call('Page.captureScreenshot', params).catch(() => null);
  if (!reply || !reply.data) return failed('no picture could be taken');
  return done(said, page.surface.savePicture(Buffer.from(reply.data, 'base64')));
}

/** The smallest and the largest a viewport side may be, in pixels. */
const VIEWPORT_LIMITS = [100, 4000];

/**
 * Sets the size of the page's viewport by emulation, or gives it back to the pane.
 * The emulated size stays through navigation and through the pane's own resizing; the pane shows the page at that size at its top left.
 * @param {Page} page The page.
 * @param {Json} args `width` and `height` in CSS pixels, or `reset: true`.
 * @returns {Promise<ToolOutcome>} The answer, with the size the page reports.
 */
async function resize(page, args) {
  const cdp = protocol(page);
  if (args.reset === true) {
    await cdp.call('Emulation.clearDeviceMetricsOverride');
    page.viewport = null;
    return done('The viewport follows the pane again.');
  }
  const [low, high] = VIEWPORT_LIMITS;
  const [width, height] = [args.width, args.height];
  if (![width, height].every((side) => Number.isInteger(side) && side >= low && side <= high)) {
    return failed(`give width and height as whole numbers from ${low} to ${high}, or reset as true`);
  }
  await cdp.call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 0, mobile: false });
  page.viewport = { width, height };
  const found = await inWorld(page, `JSON.stringify({ width: window.innerWidth, height: window.innerHeight })`);
  return done(`The viewport is ${width} by ${height}; the page reports ${found.width} by ${found.height}. The pane shows it at its top left.`);
}

/** The tools that only read what the helper recorded, so they answer while a dialog stops the page. */
const WORKS_WITH_DIALOG = new Set(['browser_console', 'browser_network']);

/**
 * Answers the dialog that is open.
 * @param {Page} page The page.
 * @param {import('./dialogs').Dialogs} dialogs The page's dialogs.
 * @param {Json} args What the agent gave the tool: `accept`, and `prompt_text` for a prompt.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function answerDialog(page, dialogs, args) {
  if (typeof args.accept !== 'boolean') return failed('say whether to accept the dialog: give accept as true or false');
  const shown = dialogs.pending;
  if (!shown) return failed(`No dialog is open.${dialogs.report()}`);
  const text = typeof args.prompt_text === 'string' ? args.prompt_text : '';
  await dialogs.answer(args.accept, text);
  await settled(page, CLICK_STILL, CLICK_SETTLING);
  return done(`${args.accept ? 'Accepted' : 'Dismissed'} the ${shown.type} "${shown.message}".${dialogs.report()}`);
}

/**
 * Runs a tool against a page, without letting a dialog the page opens hold it up.
 * While a dialog is open the page is stopped, so a tool that talks to the page would never answer: it is refused with the dialog described instead, and a tool that opens one ends at once saying so. What happened to dialogs since the agent was last told is added to every answer.
 * @param {Page} page The page.
 * @param {string} tool The tool's name, such as `browser_click`.
 * @param {Json} args What the agent gave it.
 * @returns {Promise<ToolOutcome>} What it came to.
 */
async function run(page, tool, args) {
  const dialogs = page.dialogs;
  if (!dialogs) return execute(page, tool, args);
  if (tool === 'browser_handle_dialog') return answerDialog(page, dialogs, args);
  if (dialogs.pending && !WORKS_WITH_DIALOG.has(tool)) {
    return failed(`${tool} cannot run while a dialog stops the page.${dialogs.report()}`);
  }
  let stop = () => {};
  /** @type {Promise<ToolOutcome>} */
  const opened = new Promise((resolve) => {
    stop = dialogs.watch(() => resolve(done('The page opened a dialog, so the tool may not have finished.')));
  });
  const work = execute(page, tool, args);
  work.catch(() => {});
  dialogs.busy += 1;
  try {
    const outcome = await Promise.race([work, opened]);
    return { ...outcome, text: outcome.text + dialogs.report() };
  } finally {
    stop();
    dialogs.busy -= 1;
  }
}

/**
 * Runs a tool against a page, with no care for dialogs.
 * @param {Page} page The page.
 * @param {string} tool The tool's name, such as `browser_click`.
 * @param {Json} args What the agent gave it.
 * @returns {Promise<ToolOutcome>} What it came to.
 */
async function execute(page, tool, args) {
  switch (tool) {
    case 'browser_navigate':
      return typeof args.url === 'string' ? navigate(page, args.url) : failed('no url was given');
    case 'browser_snapshot': {
      const found = await inWorld(page, SNAPSHOT);
      const lines = Array.isArray(found.lines) ? found.lines : [];
      const text = `URL: ${found.url || ''}\nTitle: ${found.title || ''}\n\nElements:\n${lines.join('\n')}\n\nText:\n${found.text || ''}`;
      return done(cut(text, MOST_TEXT));
    }
    case 'browser_screenshot':
      return screenshot(page, args);
    case 'browser_hover':
      return hover(page, args);
    case 'browser_drag':
      return drag(page, args);
    case 'browser_fill_form':
      return fillForm(page, args);
    case 'browser_highlight':
      return highlight(page, args);
    case 'browser_get_bounding_box':
      return boundingBox(page, args);
    case 'browser_resize':
      return resize(page, args);
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
    case 'browser_type':
      return typeInto(page, args);
    case 'browser_select_option':
      return selectOption(page, args);
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
      const refusal = mayVisit(page, -1);
      if (refusal) return failed(refusal);
      return loading(page, () => page.contents.navigationHistory.goBack());
    }
    case 'browser_forward': {
      if (!page.contents.navigationHistory.canGoForward()) return failed('There is no later page.');
      const refusal = mayVisit(page, 1);
      if (refusal) return failed(refusal);
      return loading(page, () => page.contents.navigationHistory.goForward());
    }
    case 'browser_reload': {
      const refusal = mayVisit(page, 0);
      if (refusal) return failed(refusal);
      return loading(page, () => page.contents.reload());
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
