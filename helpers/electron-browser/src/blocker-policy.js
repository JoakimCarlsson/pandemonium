'use strict';

/**
 * The decisions of the blocker of ads and trackers that need no engine: which filter lists it uses, when they are old
 * enough to fetch again, which site a page is on and on which sites it blocks nothing. `src/blocker.js` applies them.
 */

/** The filter lists the blocker is built from: EasyList for ads and EasyPrivacy for trackers. */
const LISTS = Object.freeze(['https://easylist.to/easylist/easylist.txt', 'https://easylist.to/easylist/easyprivacy.txt']);

/** How old the kept lists may get before they are fetched again, in milliseconds: a day. */
const MAX_AGE = 24 * 60 * 60 * 1000;

/** How often the helper looks whether the kept lists got too old, in milliseconds: an hour. */
const CHECK_EVERY = 60 * 60 * 1000;

/** How long to wait after a fetch failed before trying again, in milliseconds: ten minutes. */
const RETRY_AFTER = 10 * 60 * 1000;

/**
 * The host of a web address, which is the site the reader lets through or not.
 * @param {string} url The address.
 * @returns {string} Its host in lower case, without the brackets of an IPv6 address, or an empty string when it is not an `http` or `https` address.
 */
function siteOf(url) {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return '';
    return parsed.hostname.toLowerCase().replace(/^\[|\]$/g, '');
  } catch (_) {
    return '';
  }
}

/**
 * Whether a host is this machine, where a reader runs the sites being built, which the blocker leaves alone.
 * @param {string} host The host, as {@link siteOf} gives it.
 * @returns {boolean} Whether it is `localhost`, a name under `.localhost`, or a loopback address.
 */
function isLoopback(host) {
  return (
    host === 'localhost' ||
    host.endsWith('.localhost') ||
    host === '::1' ||
    host === '0.0.0.0' ||
    /^127(\.\d{1,3}){3}$/.test(host)
  );
}

/**
 * Whether the blocker works on the page at an address.
 * @param {boolean} enabled Whether blocking is on in the settings.
 * @param {string} url The address of the page, the top frame's.
 * @param {ReadonlySet<string>} allowed The sites the reader let through.
 * @returns {boolean} Whether it blocks there: on, on a web address that is not this machine and not let through.
 */
function blocksOn(enabled, url, allowed) {
  const site = siteOf(url);
  return enabled && site !== '' && !isLoopback(site) && !allowed.has(site);
}

/**
 * Whether lists fetched at a time are too old to keep using without fetching them again.
 * @param {number | null} fetched When they were fetched, in milliseconds since the epoch, or nothing when there are none.
 * @param {number} now The time now, in the same units.
 * @returns {boolean} Whether to fetch them again.
 */
function stale(fetched, now) {
  return fetched === null || !Number.isFinite(fetched) || now - fetched >= MAX_AGE || fetched > now + MAX_AGE;
}

/**
 * The sites the reader let through, read from what was kept on disk.
 * @param {unknown} kept The parsed file: a list of hosts.
 * @returns {Set<string>} The hosts that are strings and not empty, in lower case.
 */
function allowedFrom(kept) {
  if (!Array.isArray(kept)) return new Set();
  return new Set(kept.filter((site) => typeof site === 'string' && site.trim() !== '').map((site) => site.trim().toLowerCase()));
}

module.exports = { LISTS, MAX_AGE, CHECK_EVERY, RETRY_AFTER, siteOf, isLoopback, blocksOn, stale, allowedFrom };
