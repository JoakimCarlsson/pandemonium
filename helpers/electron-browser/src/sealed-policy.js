'use strict';

/**
 * What a sealed page may do, decided without the engine so that it can be tested alone: which requests it may make,
 * how large its document may be, and the document as it is handed to the engine, with a content security policy put
 * into it.
 *
 * A sealed page shows a document an agent wrote, a prototype or the mockup of an answer. Its scripts may run, and
 * nothing in it may reach the network.
 */

/** The most bytes of HTML a sealed page takes, as the harness allows an agent to write. */
const LARGEST_DOCUMENT = 256 * 1024;

/** The schemes a sealed page may ask for: what is already inside the document or made by it. */
const ALLOWED_SCHEMES = ['data', 'blob', 'about'];

/**
 * The content security policy a sealed document is given, as defence in depth behind the cancelled requests: nothing is
 * fetched from anywhere, inline scripts and styles run, forms go nowhere and WebRTC opens no connection.
 */
const CONTENT_POLICY = [
  "default-src 'none'",
  "script-src 'unsafe-inline' 'unsafe-eval' data: blob:",
  "style-src 'unsafe-inline' data: blob:",
  'img-src data: blob:',
  'font-src data: blob:',
  'media-src data: blob:',
  'connect-src data: blob:',
  'worker-src data: blob:',
  'child-src data: blob:',
  'frame-src data: blob: about:',
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
  "webrtc 'block'",
].join('; ');

/**
 * Whether a sealed page may make a request: only for a `data:`, `blob:` or `about:` address.
 * @param {unknown} url The address asked for.
 * @returns {boolean} Whether the request goes ahead; anything that is not such an address is cancelled.
 */
function allowed(url) {
  if (typeof url !== 'string') return false;
  const scheme = /^\s*([a-z][a-z0-9+.-]*):/i.exec(url);
  return scheme !== null && ALLOWED_SCHEMES.includes(scheme[1].toLowerCase());
}

/**
 * Why a document cannot be shown sealed, if it cannot.
 * @param {unknown} html The document.
 * @returns {string | null} The reason, or nothing when it can be shown.
 */
function refusal(html) {
  if (typeof html !== 'string' || html.length === 0) return 'There is no document to show.';
  if (Buffer.byteLength(html, 'utf8') > LARGEST_DOCUMENT) return 'The document is larger than 256 KiB.';
  return null;
}

/**
 * The document with the content security policy as its first element: after the doctype when it has one, so that the
 * page keeps the rendering mode it asked for, and before everything else, so that the policy holds for all of it.
 * @param {string} html The document as the agent wrote it.
 * @returns {string} The document to load.
 */
function sealedDocument(html) {
  const policy = `<meta http-equiv="Content-Security-Policy" content="${CONTENT_POLICY}">`;
  const doctype = /^(?:\ufeff)?\s*<!doctype[^>]*>/i.exec(html);
  if (!doctype) return policy + html;
  return html.slice(0, doctype[0].length) + policy + html.slice(doctype[0].length);
}

/**
 * The address a sealed document is loaded from: the document itself as a `data:` address, which has an origin of its
 * own with no cookies and no storage.
 * @param {string} html The document as the agent wrote it.
 * @returns {string} The address.
 */
function address(html) {
  return `data:text/html;charset=utf-8;base64,${Buffer.from(sealedDocument(html), 'utf8').toString('base64')}`;
}

/**
 * Whether a number is a usable side of a viewport, in CSS pixels.
 * @param {unknown} side The side.
 * @returns {boolean} Whether it is a whole number from 16 to 4096.
 */
function usableSide(side) {
  return typeof side === 'number' && Number.isInteger(side) && side >= 16 && side <= 4096;
}

module.exports = { LARGEST_DOCUMENT, CONTENT_POLICY, allowed, refusal, sealedDocument, address, usableSide };
