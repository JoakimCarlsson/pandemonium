'use strict';

/**
 * Which addresses the tools of agents may take a page to.
 *
 * With the restriction off, any web address is allowed. With it on, only local addresses
 * (`localhost`, `127.0.0.1` and `[::1]`, on any port) and the origins the reader added. The window
 * sends the setting with `AgentOrigins` when the helper starts and whenever it changes; until then
 * nothing is restricted, which is how the setting comes.
 */

/**
 * One origin the reader allowed.
 * @typedef {object} Allowed
 * @property {string} protocol Its scheme with a colon: `http:` or `https:`.
 * @property {string} hostname Its host, lower case, with brackets around an IPv6 address.
 * @property {string} port Its port, or an empty string for the scheme's usual one.
 */

/** The hosts that are always allowed, whatever the reader listed. */
const LOCAL = new Set(['localhost', '127.0.0.1', '[::1]']);

/** Whether agents are held to local addresses and the reader's list. */
let restricted = false;

/**
 * The origins the reader added.
 * @type {Allowed[]}
 */
let allowed = [];

/**
 * The origins one line of the reader's list stands for: with a scheme, that origin; without, the host and port under both `http` and `https`.
 * @param {string} line What the reader wrote, such as `https://example.com`, `example.com` or `staging.local:8080`.
 * @returns {Allowed[]} The origins, or none when the line is not a web address.
 */
function parseEntry(line) {
  const text = line.trim();
  if (!text) return [];
  const candidates = text.includes('://') ? [text] : [`http://${text}`, `https://${text}`];
  /** @type {Allowed[]} */
  const found = [];
  for (const candidate of candidates) {
    try {
      const { protocol, hostname, port, username, password } = new URL(candidate);
      if ((protocol === 'http:' || protocol === 'https:') && hostname && !username && !password) {
        found.push({ protocol, hostname, port });
      }
    } catch (_) {
      /* not an address: it allows nothing */
    }
  }
  return found;
}

/**
 * Sets the restriction and the reader's list.
 * @param {boolean} restrict Whether agents are held to local addresses and `origins`.
 * @param {string[]} origins The origins the reader added.
 * @returns {void}
 */
function setPolicy(restrict, origins) {
  restricted = restrict === true;
  allowed = (Array.isArray(origins) ? origins : []).filter((line) => typeof line === 'string').flatMap(parseEntry);
}

/**
 * Whether the restriction is on.
 * @returns {boolean} Whether it is.
 */
function isRestricted() {
  return restricted;
}

/**
 * Why an agent may not send a page to an address.
 * @param {string} url The address.
 * @returns {string | null} The reason, or nothing when the agent may: a web address or the blank page, and with the restriction on, one of a local host or the reader's list.
 */
function mayOpen(url) {
  if (url === 'about:blank') return null;
  /** @type {URL} */
  let address;
  try {
    address = new URL(url);
  } catch (_) {
    return 'Only http and https addresses can be opened.';
  }
  if (address.protocol !== 'http:' && address.protocol !== 'https:') {
    return 'Only http and https addresses can be opened.';
  }
  if (!restricted || LOCAL.has(address.hostname)) return null;
  const listed = allowed.some(
    (entry) => entry.protocol === address.protocol && entry.hostname === address.hostname && entry.port === address.port,
  );
  if (listed) return null;
  return `Blocked: ${address.origin} is not an origin agents may open. Agent browsing is restricted to localhost, 127.0.0.1, [::1] and the origins the reader added in Settings under Browser. Ask the reader to add the origin there, or to turn the restriction off.`;
}

module.exports = { isRestricted, mayOpen, setPolicy };
