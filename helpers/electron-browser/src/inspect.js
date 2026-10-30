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

/**
 * `browser_console`: the console's messages, filtered by `level`, and forgotten with `clear`.
 * @param {Page} page The page.
 * @param {Json} args `level` and `clear`.
 * @returns {ToolOutcome} The answer.
 */
function consoleTool(page, args) {
  const levels = netview.parseLevels(args.level);
  if (levels && 'error' in levels) return failed(levels.error);
  const text = recorder(page).consoleText('The console is empty.', /** @type {Set<string> | null} */ (levels), args.clear === true);
  return done(args.clear === true ? `${text}\n[the console was cleared]` : text);
}

/**
 * The body of a request, as text an agent can read.
 * @param {Page} page The page.
 * @param {import('./netview').NetEntry} entry The request.
 * @returns {Promise<string>} The body, cut, or why there is none.
 */
async function bodyOf(page, entry) {
  if (entry.failure !== null) return '(the request failed, so there is no body)';
  if (entry.finished === null) return '(the request has not finished)';
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
 * `browser_network`: the requests, filtered by `filter`, forgotten with `clear`, or one in full with `request`.
 * @param {Page} page The page.
 * @param {Json} args `filter`, `clear` and `request`.
 * @returns {Promise<ToolOutcome>} The answer.
 */
async function networkTool(page, args) {
  const record = recorder(page);
  const clear = args.clear === true;
  if (args.filter !== undefined && typeof args.filter !== 'string') return failed('give filter as text, such as "api", "4xx" or "failed"');
  if (args.request !== undefined) {
    if (!Number.isInteger(args.request)) return failed('give request as the number after # in the list');
    const entry = record.request(args.request);
    if (!entry) return failed(`no request #${args.request} is kept; list the requests again (only the latest ${200} are kept, and clear forgets them)`);
    const text = netview.detail(entry, await bodyOf(page, entry));
    if (clear) record.forgetNetwork();
    return done(text);
  }
  const text = record.networkText('No requests were made.', args.filter || '', clear);
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
  const cdp = protocol(page);
  if (args.reset === true) {
    await emulation.applyViewport(cdp, null);
    await emulation.applyUserAgent(cdp, null);
    page.viewport = null;
    page.userAgent = null;
    return done('Emulation cleared: the viewport follows the pane and the user agent is the browser\'s own.');
  }
  const asked = ['device', 'width', 'height', 'device_scale_factor', 'mobile', 'user_agent'].some((name) => args[name] !== undefined);
  if (!asked) return done(`Emulating: ${emulation.describe({ viewport: page.viewport, userAgent: page.userAgent })}.`);
  const needsPane = page.viewport === null && args.device === undefined;
  const wanted = emulation.resolve(args, { viewport: page.viewport, userAgent: page.userAgent }, needsPane ? await paneSize(page) : { width: 0, height: 0 });
  if ('error' in wanted) return failed(wanted.error);
  await emulation.applyViewport(cdp, wanted.viewport);
  await emulation.applyUserAgent(cdp, wanted.userAgent);
  page.viewport = wanted.viewport;
  page.userAgent = wanted.userAgent;
  const found = await cdp
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
