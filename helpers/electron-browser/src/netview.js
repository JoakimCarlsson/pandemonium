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
 */

/**
 * One line the console kept.
 * @typedef {object} ConsoleLine
 * @property {string} level `error`, `warning`, `info`, `log` or `debug`.
 * @property {string} text What it said.
 */

/** The most characters of a response body that are given back. */
const MOST_BODY = 8000;

/** The names of headers whose values are replaced. */
const SECRET_HEADERS = new Set(['authorization', 'proxy-authorization', 'cookie', 'set-cookie', 'set-cookie2', 'x-api-key', 'x-auth-token']);

/** The console levels an agent may ask for. */
const LEVELS = ['error', 'warning', 'info', 'log', 'debug'];

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
  return `#${entry.id} ${entry.method} ${entry.url} -> ${outcome}`;
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
    `Response body:\n${body}`,
  ];
  return sections.join('\n');
}

module.exports = { MOST_BODY, LEVELS, levelOf, parseLevels, matches, line, redactHeaders, textLike, timingText, detail };
