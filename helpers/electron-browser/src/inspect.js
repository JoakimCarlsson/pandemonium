'use strict';

/**
 * The tools that look into the page's engine rather than its content: the console, the network, raw
 * protocol commands and device emulation.
 */

const { checkMethod } = require('./cdp-policy');
const emulation = require('./emulation');
const netview = require('./netview');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */

/** The most characters of a raw protocol result that are given back. */
const MOST_RESULT = 20000;

/** How long a raw protocol command may take, in milliseconds. */
const COMMAND_TIME = 30000;

/**
 * A tool's answer: it worked.
 * @param {string} text What it says.
 * @param {string | null} [image] Where a picture it took was saved.
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
  return text.length > most ? `${text.slice(0, most)}\n[cut: ${text.length - most} more characters]` : text;
}

/**
 * Sends a command, giving up after `COMMAND_TIME`.
 * @param {import('./cdp').Cdp} cdp The page's protocol.
 * @param {string} method The command.
 * @param {Json} params What it takes.
 * @returns {Promise<Json>} The result; rejects with the protocol's error or when it takes too long.
 */
function timed(cdp, method, params) {
  /** @type {NodeJS.Timeout | undefined} */
  let timer;
  const late = new Promise((_resolve, reject) => {
    timer = setTimeout(() => reject(new Error(`${method} did not answer in ${COMMAND_TIME / 1000} seconds`)), COMMAND_TIME);
  });
  return Promise.race([cdp.call(method, params), late]).finally(() => clearTimeout(timer));
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
 * The page's recorder.
 * @param {Page} page The page.
 * @returns {import('./recorder').Recorder} Its recorder; throws when it has none.
 */
function recorder(page) {
  if (!page.recorder) throw new Error('the page has no recorder');
  return page.recorder;
}

/** How long `wait_for` waits when no `timeout_ms` is given, in milliseconds. */
const WAIT_DEFAULT = 10000;

/** The longest `wait_for` may wait, in milliseconds, well within how long the window waits for a tool. */
const WAIT_MOST = 60000;

/**
 * `browser_console`: the console's messages, filtered by `level`, newer than `since`, and forgotten with `clear`; the answer ends with the cursor to pass as `since` next time.
 * @param {Page} page The page.
 * @param {Json} args `level`, `since` and `clear`.
 * @returns {ToolOutcome} The answer.
 */
function consoleTool(page, args) {
  const levels = netview.parseLevels(args.level);
  if (levels && 'error' in levels) return failed(levels.error);
  if (args.since !== undefined && !(Number.isInteger(args.since) && args.since >= 0)) return failed('give since as the cursor a browser_console answer ended with, a whole number');
  const record = recorder(page);
  const since = args.since === undefined ? 0 : args.since;
  const cursor = record.consoleSequence;
  const none = since > 0 ? 'No new messages.' : 'The console is empty.';
  const text = record.consoleText(none, /** @type {Set<string> | null} */ (levels), args.clear === true, since);
  const tail = `[cursor: ${cursor}; pass since: ${cursor} to see only newer messages]`;
  return done(args.clear === true ? `${text}\n[the console was cleared]\n${tail}` : `${text}\n${tail}`);
}

/**
 * The body of a request, as text an agent can read: the one recorded when it finished, or else what the engine still holds.
 * @param {Page} page The page.
 * @param {import('./netview').NetEntry} entry The request.
 * @returns {Promise<string>} The body, cut, or why there is none.
 */
async function bodyOf(page, entry) {
  if (entry.failure !== null) return '(the request failed, so there is no body)';
  if (entry.finished === null) return '(the request has not finished)';
  if (entry.body !== null) return entry.body;
  if (entry.bodyNote) return `(${entry.bodyNote})`;
  if (!netview.textLike(entry.mimeType)) return `(not shown: ${entry.mimeType || 'unknown type'} is not text)`;
  try {
    const reply = await timed(protocol(page), 'Network.getResponseBody', { requestId: entry.requestId });
    const text = reply.base64Encoded ? Buffer.from(reply.body, 'base64').toString('utf8') : String(reply.body);
    return cut(text, netview.MOST_BODY);
  } catch (error) {
    return `(the engine no longer has the body: ${error instanceof Error ? error.message : error})`;
  }
}

/**
 * `browser_network` with `wait_for`: waits for a request whose address matches to finish and gives it in full.
 * @param {Page} page The page.
 * @param {Json} args `wait_for`, `type` and `timeout_ms`.
 * @param {Set<string> | null} types The resource types wanted, or nothing for any.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function waitForRequest(page, args, types) {
  if (typeof args.wait_for !== 'string' || args.wait_for.trim() === '') return failed('give wait_for as part of an address, or a pattern with *, such as "/api/items" or "*://*/api/*"');
  if (args.timeout_ms !== undefined && !(Number.isInteger(args.timeout_ms) && args.timeout_ms > 0)) return failed('give timeout_ms as a whole number of milliseconds');
  const timeout = Math.min(args.timeout_ms === undefined ? WAIT_DEFAULT : args.timeout_ms, WAIT_MOST);
  const entry = await recorder(page).waitFor(args.wait_for, types, timeout);
  if (!entry) return failed(`No request matching "${args.wait_for}" finished in ${timeout} ms.`);
  return done(`A request matching "${args.wait_for}" finished.\n${netview.detail(entry, await bodyOf(page, entry))}`);
}

/**
 * `browser_network`: the requests, filtered by `filter` and `type`, forgotten with `clear`, one in full with `request`, or the next to finish matching `wait_for`.
 * @param {Page} page The page.
 * @param {Json} args `filter`, `type`, `clear`, `request`, `wait_for` and `timeout_ms`.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function networkTool(page, args) {
  const record = recorder(page);
  const clear = args.clear === true;
  if (args.filter !== undefined && typeof args.filter !== 'string') return failed('give filter as text, such as "api", "4xx" or "failed"');
  const types = netview.parseTypes(args.type);
  if (types && 'error' in types) return failed(types.error);
  const wanted = /** @type {Set<string> | null} */ (types);
  if (args.wait_for !== undefined) return waitForRequest(page, args, wanted);
  if (args.request !== undefined) {
    if (!Number.isInteger(args.request)) return failed('give request as the number after # in the list');
    const entry = record.request(args.request);
    if (!entry) return failed(`no request #${args.request} is kept; list the requests again (only the latest ${200} are kept, and clear forgets them)`);
    const text = netview.detail(entry, await bodyOf(page, entry));
    if (clear) record.forgetNetwork();
    return done(text);
  }
  const text = record.networkText('No requests were made.', args.filter || '', clear, wanted);
  return done(clear ? `${text}\n[the requests were cleared]` : text);
}


/**
 * `browser_cdp`: one DevTools command from the allowlist, with its result cut to a size.
 * @param {Page} page The page.
 * @param {Json} args `method` and `params`.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function cdpTool(page, args) {
  const verdict = checkMethod(args.method);
  if (!verdict.allowed) return failed(verdict.reason);
  if (args.params !== undefined && (args.params === null || typeof args.params !== 'object' || Array.isArray(args.params))) {
    return failed('give params as an object');
  }
  /** @type {Json} */
  const params = { ...(args.params || {}) };
  const shot = args.method === 'Page.captureScreenshot';
  if (shot) params.format = 'png';
  try {
    const result = await timed(protocol(page), args.method, params);
    if (shot && typeof result.data === 'string') {
      return done('Screenshot taken.', page.surface.savePicture(Buffer.from(result.data, 'base64')));
    }
    const text = JSON.stringify(result === undefined ? null : result) || 'null';
    return done(cut(text, MOST_RESULT));
  } catch (error) {
    return failed(`${args.method} failed: ${error instanceof Error ? error.message : error}`);
  }
}

/**
 * The size the page has now.
 * @param {Page} page The page.
 * @returns {Promise<{ width: number, height: number }>} Its inner size.
 */
async function paneSize(page) {
  const reply = await protocol(page).call('Runtime.evaluate', {
    expression: 'JSON.stringify({ width: window.innerWidth, height: window.innerHeight })',
    returnByValue: true,
  });
  try {
    const found = JSON.parse(reply.result.value);
    return { width: Math.max(100, Math.min(4000, Math.round(found.width))), height: Math.max(100, Math.min(4000, Math.round(found.height))) };
  } catch (_) {
    return { width: 1280, height: 800 };
  }
}

/**
 * `browser_emulate`: a device preset, or a viewport, pixel ratio, mobile mode and user agent; `reset` clears all of them.
 * @param {Page} page The page.
 * @param {Json} args `device`, `width`, `height`, `device_scale_factor`, `mobile`, `user_agent` and `reset`.
 * @returns {Promise<ToolOutcome>} The answer, with what the page reports.
 */
async function emulateTool(page, args) {
  protocol(page);
  if (args.reset === true) {
    await page.emulateViewport(null);
    await emulation.applyUserAgent(protocol(page), null);
    page.userAgent = null;
    return done('Emulation cleared: the viewport follows the pane and the user agent is the browser\'s own.');
  }
  const asked = ['device', 'width', 'height', 'device_scale_factor', 'mobile', 'user_agent'].some((name) => args[name] !== undefined);
  if (!asked) return done(`Emulating: ${emulation.describe({ viewport: page.viewport, userAgent: page.userAgent })}.`);
  const needsPane = page.viewport === null && args.device === undefined;
  const wanted = emulation.resolve(args, { viewport: page.viewport, userAgent: page.userAgent }, needsPane ? await paneSize(page) : { width: 0, height: 0 });
  if ('error' in wanted) return failed(wanted.error);
  await page.emulateViewport(wanted.viewport);
  const moved = protocol(page);
  await emulation.applyUserAgent(moved, wanted.userAgent);
  page.userAgent = wanted.userAgent;
  const found = await moved
    .call('Runtime.evaluate', {
      expression: 'JSON.stringify({ width: window.innerWidth, height: window.innerHeight, ratio: window.devicePixelRatio, touch: navigator.maxTouchPoints, agent: navigator.userAgent })',
      returnByValue: true,
    })
    .then((reply) => JSON.parse(reply.result.value))
    .catch(() => null);
  const seen = found ? ` The page reports ${found.width} by ${found.height}, pixel ratio ${found.ratio}, ${found.touch} touch points, user agent "${found.agent}".` : '';
  return done(`Emulating: ${emulation.describe(wanted)}. It stays through navigation and pane resizes; the pane shows the page at its top left.${seen} Only the page's view changes: the engine is still Chromium, so layout, features and client hints stay Chromium's.`);
}

module.exports = { consoleTool, networkTool, cdpTool, emulateTool };
