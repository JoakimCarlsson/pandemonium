'use strict';

/**
 * When the restriction on agent browsing applies to a page, and what it refuses there.
 *
 * It holds for navigation (a link, a redirect, a new window, a frame) while a tool of an agent runs
 * on the page and for a grace period after the last one ends, because a script or timer the tool
 * set off can navigate just after it returned. It holds for what a script fetches (`fetch`,
 * XMLHttpRequest, WebSocket, beacons, pings, EventSource) only while `browser_evaluate` runs and for
 * the same grace period after it. Page loads (scripts, images, fonts, stylesheets, media) are never
 * held. Nothing here depends on Electron, so it can be tested alone.
 */

const origins = require('./origins');

/** How long after a tool ends the guards still apply, in milliseconds. */
const GRACE = 5000;

/** The kinds of request a page's script makes itself, which are the ones held while a script an agent wrote may run. */
const SCRIPT_REQUESTS = new Set(['xhr', 'ping', 'webSocket', 'cspReport', 'other']);

/**
 * The restrictions of every page, so a request the engine cannot tie to a page is still held.
 * @type {Set<Restriction>}
 */
const all = new Set();

/**
 * The clock the grace periods use; tests replace it.
 * @type {() => number}
 */
let clock = () => Date.now();

/**
 * Replaces the clock the grace periods use.
 * @param {() => number} now Gives the time in milliseconds.
 * @returns {void}
 */
function setClock(now) {
  clock = now;
}

/** The tools that can make the page run script of the agent's choosing, which is why requests made by script are watched while they run. */
const SCRIPTING = new Set(['browser_evaluate', 'browser_cdp']);

/**
 * The restriction of one page.
 */
class Restriction {
  constructor() {
    /** @type {number} How many tools of agents run against the page. */
    this.tools = 0;
    /** @type {number} How many `browser_evaluate` calls run against the page. */
    this.evaluations = 0;
    /** @type {number} When the last tool ended, or minus infinity when none has. */
    this.toolEnded = -Infinity;
    /** @type {number} When the last `browser_evaluate` ended, or minus infinity when none has. */
    this.evaluationEnded = -Infinity;
    /** @type {string | null} What was refused and has not yet been told to the agent. */
    this.refused = null;
    all.add(this);
  }

  /**
   * Stops counting this page, as when it closes.
   * @returns {void}
   */
  dispose() {
    all.delete(this);
  }

  /**
   * Notes that a tool starts.
   * @param {string} tool The tool's name.
   * @returns {void}
   */
  start(tool) {
    this.tools += 1;
    if (SCRIPTING.has(tool)) this.evaluations += 1;
  }

  /**
   * Notes that a tool ended.
   * @param {string} tool The tool's name.
   * @returns {void}
   */
  end(tool) {
    this.tools -= 1;
    this.toolEnded = clock();
    if (SCRIPTING.has(tool)) {
      this.evaluations -= 1;
      this.evaluationEnded = clock();
    }
  }

  /**
   * Whether a tool runs or ended a moment ago, so what the page does now may be the agent's doing.
   * @returns {boolean} Whether it does.
   */
  recent() {
    return this.tools > 0 || clock() - this.toolEnded < GRACE;
  }

  /**
   * Whether a script an agent evaluated may still be running.
   * @returns {boolean} Whether it may.
   */
  scripted() {
    return this.evaluations > 0 || clock() - this.evaluationEnded < GRACE;
  }

  /**
   * Checks an address a page is about to navigate to against what agents may open.
   * @param {string} url Where the page is going.
   * @param {boolean} [frame] Whether a frame inside the page is going there, not the page itself.
   * @returns {string | null} Why it may not, which is kept for the agent's next answer, or nothing when it may.
   */
  guard(url, frame = false) {
    if (!origins.isRestricted() || !this.recent()) return null;
    if (frame && (url === 'about:srcdoc' || url === 'about:blank')) return null;
    const refusal = origins.mayOpen(url);
    if (refusal) this.refused = refusal;
    return refusal;
  }

  /**
   * Takes what was refused, so it is told once.
   * @returns {string | null} What was refused, or nothing.
   */
  takeRefusal() {
    const text = this.refused;
    this.refused = null;
    return text;
  }

  /**
   * Checks a request the engine is about to make for the page.
   * @param {string} url Where it goes.
   * @param {string} type The engine's name for the kind of request, such as `xhr` or `script`.
   * @returns {string | null} Why it is refused, which is kept for the agent's next answer, or nothing when it goes.
   */
  guardRequest(url, type) {
    if (!origins.isRestricted() || !this.scripted() || !SCRIPT_REQUESTS.has(type)) return null;
    const refusal = refuseRequest(url);
    if (refusal) this.refused = refusal;
    return refusal;
  }
}

/**
 * Why a request of a script may not go to an address.
 * @param {string} url The address; `ws` and `wss` count as `http` and `https`.
 * @returns {string | null} The reason, or nothing when it may.
 */
function refuseRequest(url) {
  /** @type {URL} */
  let address;
  try {
    address = new URL(url);
  } catch (_) {
    return 'Blocked: a script made a request to an address that could not be read.';
  }
  if (['data:', 'blob:', 'about:', 'chrome-extension:', 'devtools:'].includes(address.protocol)) return null;
  const web = address.protocol === 'ws:' ? 'http:' : address.protocol === 'wss:' ? 'https:' : address.protocol;
  const refusal = origins.mayOpen(`${web}//${address.host}/`);
  return refusal ? refusal.replace('is not an origin agents may open', 'is not an origin agents may reach from a script') : null;
}

/**
 * Checks a request the engine cannot tie to a page, against every page that may be running an agent's script.
 * @param {string} url Where it goes.
 * @param {string} type The engine's name for the kind of request.
 * @returns {string | null} Why it is refused, or nothing when it goes.
 */
function guardUnattributed(url, type) {
  for (const restriction of all) {
    const refusal = restriction.guardRequest(url, type);
    if (refusal) return refusal;
  }
  return null;
}

module.exports = { GRACE, Restriction, guardUnattributed, refuseRequest, setClock };
