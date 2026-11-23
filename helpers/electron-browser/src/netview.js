'use strict';

/**
 * How the recorded requests and console messages are filtered and told to an agent: pure functions
 * that need no page.
 */

/** @typedef {import('./types').Json} Json */

/**
 * One request the network recorder kept.
 * @typedef {object} NetEntry
 * @property {number} id The number the agent uses to ask for it.
 * @property {string} requestId The engine's id for the request.
 * @property {string} method The HTTP method.
 * @property {string} url The address.
 * @property {string} type The resource type, such as `Document` or `XHR`.
 * @property {number | null} status The HTTP status, once answered.
 * @property {string | null} failure Why it failed, when it did.
 * @property {Json} requestHeaders The headers sent.
 * @property {Json} responseHeaders The headers received.
 * @property {string} mimeType The response's media type.
 * @property {number | null} size The bytes that came over the network, once finished.
 * @property {number} started When it was sent, in the engine's seconds.
 * @property {number | null} finished When it finished, in the engine's seconds.
 * @property {Json | null} timing The engine's timing of the response, in milliseconds from its request time.
 * @property {boolean} cached Whether the cache answered.
 * @property {number | null} endedAt When it finished or failed, in milliseconds since the epoch.
 * @property {string | null} body The response body as text, kept when the request finished with a text type, cut to `MOST_BODY` characters; nothing until then, for other types, or once dropped to keep memory bounded.
 * @property {string} bodyNote Why there is no body when there is none and the reason is known, such as that it was dropped; empty otherwise.
 * @property {string | null} requestBody What was sent with the request, such as a form or JSON, cut to `MOST_REQUEST_BODY` characters; nothing when nothing was sent.
 */

/**
 * One line the console kept.
 * @typedef {object} ConsoleLine
 * @property {number} seq Its number, counting up from 1 for the page, which `since` is compared with.
 * @property {number} at When it was written, in milliseconds since the epoch.
 * @property {string} level `error`, `warning`, `info`, `log` or `debug`.
 * @property {string} text What it said.
 * @property {string} source Where in the page's code it was written, as `address:line:column`, or empty when unknown.
 * @property {string} stack The call stack, one frame to a line, for uncaught exceptions, errors and traces; empty otherwise.
 * @property {boolean} uncaught Whether it is an exception nothing caught.
 */

/** The most characters of a response body that are given back. */
const MOST_BODY = 8000;

/** The most characters of a request body that are given back. */
const MOST_REQUEST_BODY = 4000;

/** The most frames of a call stack that are kept. */
const MOST_FRAMES = 12;

/** The names of headers whose values are replaced. */
const SECRET_HEADERS = new Set(['authorization', 'proxy-authorization', 'cookie', 'set-cookie', 'set-cookie2', 'x-api-key', 'x-auth-token']);

/** The console levels an agent may ask for. */
const LEVELS = ['error', 'warning', 'info', 'log', 'debug'];

/** The resource types an agent may filter requests by, as the engine names them, in lower case. */
const TYPES = ['document', 'stylesheet', 'image', 'media', 'font', 'script', 'xhr', 'fetch', 'websocket', 'eventsource', 'manifest', 'ping', 'preflight', 'other'];

/**
 * The console level a protocol type or log level stands for.
 * @param {string} type The type, such as `warn`, `verbose` or `assert`.
 * @returns {string} One of `LEVELS`.
 */
function levelOf(type) {
  switch (type) {
    case 'error':
    case 'assert':
      return 'error';
    case 'warning':
    case 'warn':
      return 'warning';
    case 'info':
      return 'info';
    case 'debug':
    case 'verbose':
    case 'trace':
      return 'debug';
    default:
      return 'log';
  }
}

/**
 * Parses what an agent gave as `level`.
 * @param {unknown} level A level, or several separated by commas.
 * @returns {Set<string> | null | { error: string }} The levels wanted, nothing for all, or why not.
 */
function parseLevels(level) {
  if (level === undefined || level === '') return null;
  if (typeof level !== 'string') return { error: `give level as text: some of ${LEVELS.join(', ')}` };
  const wanted = new Set();
  for (const part of level.split(',')) {
    const name = levelOf(part.trim().toLowerCase());
    if (!LEVELS.includes(part.trim().toLowerCase()) && part.trim().toLowerCase() !== 'warn') {
      return { error: `unknown level "${part.trim()}"; use some of ${LEVELS.join(', ')}` };
    }
    wanted.add(name);
  }
  return wanted;
}

/**
 * Whether a request is what a `filter` asks for: a status class such as `4xx`, an exact status, `failed`, or text in the address or method.
 * @param {NetEntry} entry The request.
 * @param {string} filter What the agent gave.
 * @returns {boolean} Whether it is wanted.
 */
function matches(entry, filter) {
  const wanted = filter.trim().toLowerCase();
  if (wanted === '') return true;
  if (/^[1-5]xx$/.test(wanted)) return entry.status !== null && String(entry.status)[0] === wanted[0];
  if (wanted === 'failed') return entry.failure !== null;
  if (/^[1-5]\d\d$/.test(wanted) && entry.status !== null && String(entry.status) === wanted) return true;
  return entry.url.toLowerCase().includes(wanted) || entry.method.toLowerCase() === wanted;
}

/**
 * One request as a line.
 * @param {NetEntry} entry The request.
 * @returns {string} The line, starting with the number to ask for it by.
 */
function line(entry) {
  let outcome = 'pending';
  if (entry.failure !== null) outcome = `failed: ${entry.failure}`;
  else if (entry.status !== null) outcome = String(entry.status);
  return `#${entry.id} ${entry.method} ${entry.url} -> ${outcome}${entry.type ? ` (${entry.type.toLowerCase()})` : ''}`;
}

/**
 * Headers with the values of secret ones replaced.
 * @param {Json} headers The headers.
 * @returns {Json} A copy that holds no credentials.
 */
function redactHeaders(headers) {
  /** @type {Json} */
  const safe = {};
  for (const [name, value] of Object.entries(headers || {})) {
    safe[name] = SECRET_HEADERS.has(name.toLowerCase()) ? '[redacted]' : value;
  }
  return safe;
}

/**
 * Whether a media type is text a body can be shown for.
 * @param {string} mime The media type.
 * @returns {boolean} Whether it is.
 */
function textLike(mime) {
  const type = (mime || '').toLowerCase().split(';')[0].trim();
  return type.startsWith('text/') || /(json|xml|javascript|ecmascript|x-www-form-urlencoded|graphql|svg)/.test(type);
}

/**
 * How long a request took and where the time went.
 * @param {NetEntry} entry The request.
 * @returns {string} Milliseconds, or `unknown` while it has not finished.
 */
function timingText(entry) {
  const parts = [];
  if (entry.finished !== null) parts.push(`total ${Math.round((entry.finished - entry.started) * 1000)} ms`);
  const timing = entry.timing;
  if (timing && typeof timing.sendEnd === 'number' && typeof timing.receiveHeadersEnd === 'number' && timing.receiveHeadersEnd >= 0) {
    parts.push(`waiting for the first byte ${Math.round(timing.receiveHeadersEnd - timing.sendEnd)} ms`);
    if (timing.connectStart >= 0 && timing.connectEnd >= 0) parts.push(`connecting ${Math.round(timing.connectEnd - timing.connectStart)} ms`);
    if (timing.dnsStart >= 0 && timing.dnsEnd >= 0) parts.push(`dns ${Math.round(timing.dnsEnd - timing.dnsStart)} ms`);
  }
  return parts.length ? parts.join(', ') : 'unknown';
}

/**
 * One request in full.
 * @param {NetEntry} entry The request.
 * @param {string} body What to say of the body: its text, or why there is none.
 * @returns {string} The text.
 */
function detail(entry, body) {
  const sections = [
    line(entry),
    `Type: ${entry.type || 'unknown'}${entry.mimeType ? `, ${entry.mimeType}` : ''}${entry.cached ? ' (from cache)' : ''}`,
    `Size on the wire: ${entry.size === null ? 'unknown' : `${entry.size} bytes`}`,
    `Timing: ${timingText(entry)}`,
    `Request headers (secrets redacted; the engine may add some it does not report):\n${JSON.stringify(redactHeaders(entry.requestHeaders), null, 1)}`,
    `Response headers (secrets redacted):\n${JSON.stringify(redactHeaders(entry.responseHeaders), null, 1)}`,
    `Request body:\n${entry.requestBody === null || entry.requestBody === undefined ? '(none)' : entry.requestBody}`,
    `Response body:\n${body}`,
  ];
  return sections.join('\n');
}

/**
 * Parses what an agent gave as `type`.
 * @param {unknown} type A resource type, such as `xhr` or `fetch`, or several separated by commas.
 * @returns {Set<string> | null | { error: string }} The types wanted, in lower case, nothing for all, or why not.
 */
function parseTypes(type) {
  if (type === undefined || type === '') return null;
  if (typeof type !== 'string') return { error: `give type as text: some of ${TYPES.join(', ')}` };
  const wanted = new Set();
  for (const part of type.split(',')) {
    const name = part.trim().toLowerCase();
    if (!TYPES.includes(name)) return { error: `unknown type "${part.trim()}"; use some of ${TYPES.join(', ')}` };
    wanted.add(name);
  }
  return wanted;
}

/**
 * Whether a request is of one of the types wanted.
 * @param {NetEntry} entry The request.
 * @param {Set<string> | null} types The types wanted, in lower case, or nothing for all.
 * @returns {boolean} Whether it is.
 */
function ofType(entry, types) {
  return !types || types.has((entry.type || 'other').toLowerCase());
}

/**
 * Whether an address matches a pattern: with `*` in it, a wildcard pattern over the whole address, where `*` stands for any run of characters; otherwise, text the address contains. Case does not matter.
 * @param {string} url The address.
 * @param {string} pattern The pattern.
 * @returns {boolean} Whether it matches.
 */
function urlMatches(url, pattern) {
  const wanted = pattern.trim().toLowerCase();
  const address = url.toLowerCase();
  if (!wanted.includes('*')) return address.includes(wanted);
  const expression = wanted.split('*').map((part) => part.replace(/[.+?^${}()|[\]\\]/g, '\\$&')).join('.*');
  return new RegExp(`^${expression}$`).test(address);
}

/**
 * A call stack of the protocol as lines.
 * @param {Json | undefined} trace The protocol's `StackTrace`.
 * @returns {string} One `at function (address:line:column)` to a line, at most `MOST_FRAMES`, or empty.
 */
function stackText(trace) {
  const frames = (trace && Array.isArray(trace.callFrames) ? trace.callFrames : []).slice(0, MOST_FRAMES);
  return frames.map((frame) => `at ${frame.functionName || '(anonymous)'} (${frame.url || '<anonymous>'}:${(frame.lineNumber ?? 0) + 1}:${(frame.columnNumber ?? 0) + 1})`).join('\n');
}

/**
 * Where a protocol call stack or location points, as `address:line:column`.
 * @param {string | undefined} url The address of the script.
 * @param {number | undefined} lineNumber The line, counted from 0.
 * @param {number | undefined} columnNumber The column, counted from 0.
 * @returns {string} The place, or empty when there is no address.
 */
function sourceText(url, lineNumber, columnNumber) {
  if (!url) return '';
  if (typeof lineNumber !== 'number') return url;
  return `${url}:${lineNumber + 1}:${(columnNumber ?? 0) + 1}`;
}

/**
 * A clock time with milliseconds, in UTC.
 * @param {number} at Milliseconds since the epoch.
 * @returns {string} Such as `12:04:05.120`.
 */
function clock(at) {
  return new Date(at).toISOString().slice(11, 23);
}

/**
 * One console line as an agent reads it: its number, time, level, text and source, then its stack indented.
 * @param {ConsoleLine} entry The line.
 * @returns {string} The text.
 */
function consoleLine(entry) {
  const head = `#${entry.seq} ${clock(entry.at)} [${entry.level}] ${entry.uncaught ? 'Uncaught ' : ''}${entry.text}${entry.source ? ` (${entry.source})` : ''}`;
  if (!entry.stack) return head;
  return `${head}\n${entry.stack.split('\n').map((frame) => `    ${frame}`).join('\n')}`;
}

module.exports = {
  MOST_BODY,
  MOST_REQUEST_BODY,
  LEVELS,
  TYPES,
  levelOf,
  parseLevels,
  parseTypes,
  ofType,
  urlMatches,
  stackText,
  sourceText,
  clock,
  consoleLine,
  matches,
  line,
  redactHeaders,
  textLike,
  timingText,
  detail,
};
