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

/** The most bytes of response bodies the engine is asked to keep, so that bodies not yet read can still be. */
const BUFFER_TOTAL = 10 * 1024 * 1024;

/** The most bytes of one response body the engine is asked to keep. */
const BUFFER_ONE = 5 * 1024 * 1024;

/** The most characters of response bodies the recorder holds at once; the oldest are dropped past it. */
const BODIES_TOTAL = 1024 * 1024;

/** The kinds of request that stay open for as long as the page does, which say nothing of whether it is still loading. */
const OPEN_ENDED = new Set(['EventSource', 'WebSocket', 'Ping']);

/**
 * A request that `browser_network` waits for.
 * @typedef {object} Waiter
 * @property {string} pattern The address pattern it must match.
 * @property {Set<string> | null} types The resource types it must be of, or nothing for any.
 * @property {(entry: NetEntry) => void} resolve Called with the first request that finishes and matches.
 */

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
 * Text cut to at most `most` characters.
 * @param {string} text The text.
 * @param {number} most The most characters to keep.
 * @returns {string} The text, with a mark where it was cut.
 */
function cut(text, most) {
  return text.length > most ? `${text.slice(0, most)}\n[cut: ${text.length - most} more characters]` : text;
}

/**
 * Where a call stack starts, as `address:line:column`.
 * @param {Json | undefined} trace The protocol's `StackTrace`.
 * @returns {string} The place of its first frame, or empty.
 */
function firstFrame(trace) {
  const frame = trace && Array.isArray(trace.callFrames) ? trace.callFrames[0] : null;
  return frame ? netview.sourceText(frame.url, frame.lineNumber, frame.columnNumber) : '';
}

/**
 * The console line an uncaught exception makes.
 * @param {Json} details The protocol's `ExceptionDetails`.
 * @returns {{ level: string, text: string, source: string, stack: string, uncaught: boolean }} The line, without its number and time.
 */
function exceptionLine(details) {
  const description = (details.exception && details.exception.description) || details.text || 'an exception';
  const [head, ...rest] = String(description).split('\n');
  const traced = netview.stackText(details.stackTrace);
  const stack = traced || rest.map((frame) => frame.trim()).filter(Boolean).join('\n');
  const source = netview.sourceText(details.url, details.lineNumber, details.columnNumber) || firstFrame(details.stackTrace);
  return { level: 'error', text: head.replace(/^Uncaught\s+/, ''), source, stack, uncaught: true };
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
    /** @type {number} The number the last console line got. */
    this.consoleSequence = 0;
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
    /** @type {NetEntry[]} The requests whose bodies are held, oldest first. */
    this.held = [];
    /** @type {number} How many characters of bodies are held. */
    this.heldSize = 0;
    /** @type {WeakMap<NetEntry, Promise<void>>} The reading of each body still under way. */
    this.reading = new WeakMap();
    /** @type {Waiter[]} The requests `browser_network` waits for. */
    this.waiters = [];
    /** @type {number} When the tool before the one running started, in milliseconds. */
    this.previousTool = Date.now();
    /** @type {number} When the tool running started, in milliseconds. */
    this.currentTool = Date.now();
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
   * Keeps a console line, numbered and timed.
   * @param {{ level: string, text: string, source?: string, stack?: string, uncaught?: boolean }} line What it says.
   * @returns {void}
   */
  write(line) {
    this.consoleSequence += 1;
    this.keep(this.console, {
      seq: this.consoleSequence,
      at: Date.now(),
      level: line.level,
      text: line.text,
      source: line.source || '',
      stack: line.stack || '',
      uncaught: line.uncaught === true,
    });
  }

  /**
   * Notes that a tool starts, so that `browser_network`'s `wait_for` also finds requests the tool before it caused.
   * @returns {void}
   */
  toolStarted() {
    this.previousTool = this.currentTool;
    this.currentTool = Date.now();
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
      case 'Runtime.consoleAPICalled': {
        const level = netview.levelOf(params.type || 'log');
        const traced = level === 'error' || params.type === 'trace';
        this.write({
          level,
          text: (params.args || []).map(said).join(' '),
          source: firstFrame(params.stackTrace),
          stack: traced ? netview.stackText(params.stackTrace) : '',
        });
        break;
      }
      case 'Runtime.exceptionThrown':
        this.write(exceptionLine(params.exceptionDetails || {}));
        break;
      case 'Log.entryAdded': {
        const entry = params.entry || {};
        this.write({
          level: netview.levelOf(entry.level || 'info'),
          text: entry.text || '',
          source: netview.sourceText(entry.url, entry.lineNumber, undefined) || firstFrame(entry.stackTrace),
          stack: netview.stackText(entry.stackTrace),
        });
        break;
      }
      case 'Network.requestWillBeSent':
        if (params.requestId && params.request) this.sent(params);
        break;
      case 'Network.loadingFinished': {
        this.pending.delete(params.requestId);
        const known = this.requests.get(params.requestId);
        if (known) {
          known.size = params.encodedDataLength ?? null;
          known.finished = params.timestamp ?? null;
          known.endedAt = Date.now();
          this.requests.delete(params.requestId);
          const reading = this.readBody(known);
          this.reading.set(known, reading);
          reading.then(() => this.ended(known));
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
          known.endedAt = Date.now();
          this.requests.delete(params.requestId);
          this.ended(known);
        }
        break;
      }
      default:
        break;
    }
  }

  /**
   * Records a request that was sent, and ends the one it redirects.
   * @param {Json} params What `Network.requestWillBeSent` carries.
   * @returns {void}
   */
  sent(params) {
    const before = this.requests.get(params.requestId);
    if (before && params.redirectResponse) {
      this.answer(before, params.redirectResponse);
      before.finished = params.timestamp ?? null;
      before.endedAt = Date.now();
      before.bodyNote = 'it was redirected, so it has no body';
      this.ended(before);
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
      endedAt: null,
      timing: null,
      cached: false,
      body: null,
      bodyNote: '',
      requestBody: typeof params.request.postData === 'string' ? cut(params.request.postData, netview.MOST_REQUEST_BODY) : null,
    };
    if (entry.requestBody === null && params.request.hasPostData === true) this.readRequestBody(entry);
    this.requests.set(params.requestId, entry);
    this.keep(this.network, entry);
  }

  /**
   * Reads what was sent with a request whose event did not carry it.
   * @param {NetEntry} entry The request.
   * @returns {void}
   */
  readRequestBody(entry) {
    this.cdp
      .call('Network.getRequestPostData', { requestId: entry.requestId })
      .then((reply) => {
        if (reply && typeof reply.postData === 'string') entry.requestBody = cut(reply.postData, netview.MOST_REQUEST_BODY);
      })
      .catch(() => {});
  }

  /**
   * Reads the body of a request that finished, while the engine still has it, when it is text.
   * @param {NetEntry} entry The request.
   * @returns {Promise<void>} Resolves once the body is held or known to be missing; never rejects.
   */
  async readBody(entry) {
    if (!netview.textLike(entry.mimeType)) {
      entry.bodyNote = `not shown: ${entry.mimeType || 'unknown type'} is not text`;
      return;
    }
    try {
      const reply = await this.cdp.call('Network.getResponseBody', { requestId: entry.requestId });
      const text = reply.base64Encoded ? Buffer.from(reply.body, 'base64').toString('utf8') : String(reply.body);
      this.hold(entry, cut(text, netview.MOST_BODY));
    } catch (error) {
      entry.bodyNote = `the engine gave no body: ${error instanceof Error ? error.message : error}`;
    }
  }

  /**
   * Holds the body of a request, dropping the oldest held bodies while all of them are more than `BODIES_TOTAL` characters.
   * @param {NetEntry} entry The request.
   * @param {string} body Its body, already cut.
   * @returns {void}
   */
  hold(entry, body) {
    entry.body = body;
    this.held.push(entry);
    this.heldSize += body.length;
    while (this.heldSize > BODIES_TOTAL && this.held.length > 1) {
      const oldest = /** @type {NetEntry} */ (this.held.shift());
      this.heldSize -= (oldest.body || '').length;
      oldest.body = null;
      oldest.bodyNote = 'dropped to keep the memory the recorded bodies take bounded; newer requests keep theirs';
    }
  }

  /**
   * Hands a request that ended to the waiters it matches.
   * @param {NetEntry} entry The request.
   * @returns {void}
   */
  ended(entry) {
    const matched = this.waiters.filter((waiter) => netview.ofType(entry, waiter.types) && netview.urlMatches(entry.url, waiter.pattern));
    if (!matched.length) return;
    this.waiters = this.waiters.filter((waiter) => !matched.includes(waiter));
    for (const waiter of matched) waiter.resolve(entry);
  }

  /**
   * Waits for a request whose address matches `pattern` to finish: one that already finished since the tool before this one started, or else the next one.
   * @param {string} pattern The address pattern, as `netview.urlMatches` reads it.
   * @param {Set<string> | null} types The resource types it must be of, or nothing for any.
   * @param {number} timeout How long to wait, in milliseconds.
   * @returns {Promise<NetEntry | null>} The request once its body is read, or nothing when none finished in time.
   */
  async waitFor(pattern, types, timeout) {
    const since = this.previousTool;
    const done = this.network.find(
      (entry) => entry.endedAt !== null && entry.endedAt >= since && netview.ofType(entry, types) && netview.urlMatches(entry.url, pattern),
    );
    /** @type {NodeJS.Timeout | undefined} */
    let timer;
    /** @type {Waiter | null} */
    let waiter = null;
    const found = done
      ? done
      : await new Promise((resolve) => {
          waiter = { pattern, types, resolve };
          this.waiters.push(waiter);
          timer = setTimeout(() => resolve(null), timeout);
        });
    clearTimeout(timer);
    if (waiter) this.waiters = this.waiters.filter((held) => held !== waiter);
    if (found) await (this.reading.get(found) || Promise.resolve());
    return found;
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
   * @param {number} [since] Only the lines numbered above this are given.
   * @returns {string} The lines, oldest first, as `netview.consoleLine` writes them, or `none`.
   */
  consoleText(none, levels = null, clear = false, since = 0) {
    const shown = this.console.filter((entry) => entry.seq > since && (!levels || levels.has(entry.level)));
    if (clear) this.console = [];
    return shown.length ? shown.map(netview.consoleLine).join('\n') : none;
  }

  /**
   * The requests' lines.
   * @param {string} none What to say when there are no lines.
   * @param {string} [filter] A status class, status or text of the address that the requests must match.
   * @param {boolean} [clear] Whether to forget every request after reading; those still on their way are forgotten too and are not recorded when they finish.
   * @param {Set<string> | null} [types] The resource types wanted, or nothing for all.
   * @returns {string} The lines, oldest first, or `none`.
   */
  networkText(none, filter = '', clear = false, types = null) {
    const shown = this.network.filter((entry) => netview.ofType(entry, types) && netview.matches(entry, filter));
    if (clear) this.forgetNetwork();
    return shown.length ? shown.map(netview.line).join('\n') : none;
  }

  /**
   * Forgets the requests that were recorded, and the bodies held, but not that some are still on their way.
   * @returns {void}
   */
  forgetNetwork() {
    this.network = [];
    this.requests.clear();
    this.held = [];
    this.heldSize = 0;
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
