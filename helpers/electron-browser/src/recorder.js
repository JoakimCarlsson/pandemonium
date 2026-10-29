'use strict';

/**
 * What a page wrote to its console and what it asked of the network, kept for the tools that
 * report them.
 */

/** @typedef {import('./cdp').Cdp} Cdp */
/** @typedef {import('./types').Json} Json */

/** The most lines the console and the network keep. */
const KEPT = 200;

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
    /** @type {string[]} The console's lines, oldest first. */
    this.console = [];
    /** @type {string[]} The requests' lines, oldest first. */
    this.network = [];
    /** @type {Map<string, [string, string]>} The method and address of each request, until it is answered. */
    this.requests = new Map();
    /** @type {Set<string>} The requests that were sent and are not finished. */
    this.pending = new Set();
    /** @type {number} When the network last did something, in milliseconds. */
    this.active = Date.now();
    /** @type {{ url: string, status: number, at: number } | null} The page's own document as last answered, and when. */
    this.document = null;
    cdp.listen((method, params) => this.hear(method, params));
    for (const method of ['Runtime.enable', 'Network.enable', 'Page.enable', 'Log.enable']) {
      cdp.call(method).catch(() => {});
    }
  }

  /**
   * Adds `line` to `lines`, forgetting the oldest when there are too many.
   * @param {string[]} lines Where it goes.
   * @param {string} line What to keep.
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
        this.keep(this.console, `[${params.type || 'log'}] ${(params.args || []).map(said).join(' ')}`);
        break;
      case 'Runtime.exceptionThrown': {
        const details = params.exceptionDetails || {};
        const text = (details.exception && details.exception.description) || details.text || 'an exception';
        this.keep(this.console, `[error] ${text}`);
        break;
      }
      case 'Log.entryAdded': {
        const entry = params.entry || {};
        this.keep(this.console, `[${entry.level || 'info'}] ${entry.text || ''} ${entry.url || ''}`);
        break;
      }
      case 'Network.requestWillBeSent':
        if (params.requestId && params.request) {
          if (!OPEN_ENDED.has(params.type)) this.pending.add(params.requestId);
          this.requests.set(params.requestId, [params.request.method || 'GET', params.request.url]);
        }
        break;
      case 'Network.loadingFinished':
        this.pending.delete(params.requestId);
        break;
      case 'Network.responseReceived': {
        if (params.type === 'Document' && params.response) {
          this.document = { url: params.response.url, status: params.response.status, at: Date.now() };
        }
        const known = this.requests.get(params.requestId);
        if (known) {
          this.requests.delete(params.requestId);
          this.keep(this.network, `${known[0]} ${known[1]} -> ${params.response.status}`);
        }
        break;
      }
      case 'Network.loadingFailed': {
        this.pending.delete(params.requestId);
        const known = this.requests.get(params.requestId);
        if (known) {
          this.requests.delete(params.requestId);
          this.keep(this.network, `${known[0]} ${known[1]} -> failed: ${params.errorText || 'unknown'}`);
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
   * The console's lines.
   * @param {string} none What to say when there are no lines.
   * @returns {string} The lines, oldest first, or `none`.
   */
  consoleText(none) {
    return this.console.length ? this.console.join('\n') : none;
  }

  /**
   * The requests' lines.
   * @param {string} none What to say when there are no lines.
   * @returns {string} The lines, oldest first, or `none`.
   */
  networkText(none) {
    return this.network.length ? this.network.join('\n') : none;
  }
}

exports.Recorder = Recorder;
