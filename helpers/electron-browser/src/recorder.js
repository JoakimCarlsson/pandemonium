'use strict';

/**
 * What a page wrote to its console and what it asked of the network, kept for the tools that
 * report them.
 */

/** @typedef {import('./cdp').Cdp} Cdp */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./netview').NetEntry} NetEntry */
/** @typedef {import('./netview').ConsoleLine} ConsoleLine */

const netview = require('./netview');

/** The most lines the console and the network keep. */
const KEPT = 200;

/** The most bytes of response bodies the engine is asked to keep, so that `browser_network` can read them. */
const BUFFER_TOTAL = 10 * 1024 * 1024;

/** The most bytes of one response body the engine is asked to keep. */
const BUFFER_ONE = 5 * 1024 * 1024;

/** The kinds of request that stay open for as long as the page does, which say nothing of whether it is still loading. */
const OPEN_ENDED = new Set(['EventSource', 'WebSocket', 'Ping']);

/**
 * What a protocol value says, as text.
 * @param {Json} value A remote object of the protocol.
 * @returns {string} Its value, or its description.
 */
function said(value) {
  if (typeof value.value === 'string') return value.value;
  if (value.value !== undefined && value.value !== null) return JSON.stringify(value.value);
  return value.description || '';
}

/**
 * The console and the requests of one page.
 */
class Recorder {
  /**
   * Starts recording what `cdp` reports, and has the engine report it.
   * @param {Cdp} cdp The page's protocol.
   */
  constructor(cdp) {
    /** @type {ConsoleLine[]} The console's lines, oldest first. */
    this.console = [];
    /** @type {NetEntry[]} The requests, oldest first. */
    this.network = [];
    /** @type {Map<string, NetEntry>} The requests that are not finished, by the engine's id. */
    this.requests = new Map();
    /** @type {number} The number the next request gets. */
    this.sequence = 0;
    /** @type {Set<string>} The requests that were sent and are not finished. */
    this.pending = new Set();
    /** @type {number} When the network last did something, in milliseconds. */
    this.active = Date.now();
    /** @type {{ url: string, status: number, at: number, cached: boolean } | null} The page's own document as last answered, when, and whether the engine's cache answered. */
    this.document = null;
    cdp.listen((method, params) => this.hear(method, params));
    /** @type {import('./cdp').Cdp} The page's protocol, to ask for bodies. */
    this.cdp = cdp;
    for (const method of ['Runtime.enable', 'Page.enable', 'Log.enable']) {
      cdp.call(method).catch(() => {});
    }
    cdp.call('Network.enable', { maxTotalBufferSize: BUFFER_TOTAL, maxResourceBufferSize: BUFFER_ONE }).catch(() => {});
  }

  /**
   * Adds `line` to `lines`, forgetting the oldest when there are too many.
   * @param {any[]} lines Where it goes.
   * @param {any} line What to keep.
   * @returns {void}
   */
  keep(lines, line) {
    lines.push(line);
    while (lines.length > KEPT) lines.shift();
  }

  /**
   * Records one event.
   * @param {string} method The event's name.
   * @param {Json} params What it carries.
   * @returns {void}
   */
  hear(method, params) {
    if (method.startsWith('Network.')) this.active = Date.now();
    switch (method) {
      case 'Runtime.consoleAPICalled':
        this.keep(this.console, { level: netview.levelOf(params.type || 'log'), text: (params.args || []).map(said).join(' ') });
        break;
      case 'Runtime.exceptionThrown': {
        const details = params.exceptionDetails || {};
        const text = (details.exception && details.exception.description) || details.text || 'an exception';
        this.keep(this.console, { level: 'error', text });
        break;
      }
      case 'Log.entryAdded': {
        const entry = params.entry || {};
        this.keep(this.console, { level: netview.levelOf(entry.level || 'info'), text: `${entry.text || ''} ${entry.url || ''}`.trim() });
        break;
      }
      case 'Network.requestWillBeSent':
        if (params.requestId && params.request) {
          const before = this.requests.get(params.requestId);
          if (before && params.redirectResponse) {
            this.answer(before, params.redirectResponse);
            before.finished = params.timestamp ?? null;
          }
          if (!OPEN_ENDED.has(params.type)) this.pending.add(params.requestId);
          /** @type {NetEntry} */
          const entry = {
            id: ++this.sequence,
            requestId: params.requestId,
            method: params.request.method || 'GET',
            url: params.request.url,
            type: params.type || '',
            status: null,
            failure: null,
            requestHeaders: params.request.headers || {},
            responseHeaders: {},
            mimeType: '',
            size: null,
            started: params.timestamp || 0,
            finished: null,
            timing: null,
            cached: false,
          };
          this.requests.set(params.requestId, entry);
          this.keep(this.network, entry);
        }
        break;
      case 'Network.loadingFinished': {
        this.pending.delete(params.requestId);
        const known = this.requests.get(params.requestId);
        if (known) {
          known.size = params.encodedDataLength ?? null;
          known.finished = params.timestamp ?? null;
          this.requests.delete(params.requestId);
        }
        break;
      }
      case 'Network.responseReceived': {
        if (params.type === 'Document' && params.response) {
          this.document = {
            url: params.response.url,
            status: params.response.status,
            at: Date.now(),
            cached: params.response.fromDiskCache === true || params.response.fromPrefetchCache === true,
          };
        }
        const known = this.requests.get(params.requestId);
        if (known && params.response) this.answer(known, params.response);
        break;
      }
      case 'Network.loadingFailed': {
        this.pending.delete(params.requestId);
        const known = this.requests.get(params.requestId);
        if (known) {
          known.failure = params.errorText || 'unknown';
          known.finished = params.timestamp ?? null;
          this.requests.delete(params.requestId);
        }
        break;
      }
      default:
        break;
    }
  }

  /**
   * Whether the network has been still: nothing is on its way, and nothing happened for `quiet` milliseconds.
   * @param {number} quiet How long it must have been still.
   * @returns {boolean} Whether it has.
   */
  still(quiet) {
    return this.pending.size === 0 && Date.now() - this.active >= quiet;
  }

  /**
   * Notes what the engine answered a request with.
   * @param {NetEntry} entry The request.
   * @param {Json} response The protocol's response.
   * @returns {void}
   */
  answer(entry, response) {
    entry.status = response.status;
    entry.responseHeaders = response.headers || {};
    entry.mimeType = response.mimeType || '';
    entry.timing = response.timing || null;
    entry.cached = response.fromDiskCache === true || response.fromPrefetchCache === true;
  }

  /**
   * The console's lines.
   * @param {string} none What to say when there are no lines.
   * @param {Set<string> | null} [levels] The levels wanted, or nothing for all.
   * @param {boolean} [clear] Whether to forget every line after reading.
   * @returns {string} The lines, oldest first, as `[level] text`, or `none`.
   */
  consoleText(none, levels = null, clear = false) {
    const shown = this.console.filter((entry) => !levels || levels.has(entry.level));
    if (clear) this.console = [];
    return shown.length ? shown.map((entry) => `[${entry.level}] ${entry.text}`).join('\n') : none;
  }

  /**
   * The requests' lines.
   * @param {string} none What to say when there are no lines.
   * @param {string} [filter] A status class, status or text of the address that the requests must match.
   * @param {boolean} [clear] Whether to forget every request after reading; those still on their way are forgotten too and are not recorded when they finish.
   * @returns {string} The lines, oldest first, or `none`.
   */
  networkText(none, filter = '', clear = false) {
    const shown = this.network.filter((entry) => netview.matches(entry, filter));
    if (clear) this.forgetNetwork();
    return shown.length ? shown.map(netview.line).join('\n') : none;
  }

  /**
   * Forgets the requests that were recorded, but not that some are still on their way.
   * @returns {void}
   */
  forgetNetwork() {
    this.network = [];
    this.requests.clear();
  }

  /**
   * The request that was given a number.
   * @param {number} id The number from the list.
   * @returns {NetEntry | undefined} The request, unless it is no longer kept.
   */
  request(id) {
    return this.network.find((entry) => entry.id === id);
  }
}

exports.Recorder = Recorder;
