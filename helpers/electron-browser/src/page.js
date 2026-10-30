'use strict';

/**
 * One page: an offscreen window, the files it paints into, and what is said about it to the window
 * and asked of it.
 */

const { BrowserWindow } = require('electron');
const agent = require('./agent');
const capture = require('./capture');
const { Cdp } = require('./cdp');
const { Dialogs } = require('./dialogs');
const emulation = require('./emulation');
const { FileChooser } = require('./filechooser');
const { cursorOf, deliver } = require('./input');
const origins = require('./origins');
const pick = require('./pick');
const { Recorder } = require('./recorder');
const { Restriction } = require('./restriction');
const { Surface } = require('./surface');

/** @typedef {import('./types').PageRequest} PageRequest */
/** @typedef {import('./types').Send} Send */
/** @typedef {import('./types').ToolBody} ToolBody */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */

/**
 * Whether a page may send the browser to an address by itself: only web addresses.
 * @param {string} url The address.
 * @returns {boolean} Whether it may.
 */
function webAddress(url) {
  try {
    const { protocol } = new URL(url);
    return protocol === 'http:' || protocol === 'https:';
  } catch (_) {
    return false;
  }
}

/** The zoom factors the zoom requests step through, as Chromium's own zoom menu does. */
const ZOOM_STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5];

/**
 * The zoom factor a zoom request leads to.
 * @param {import('./types').ZoomAction} action How the zoom changes.
 * @param {number} now The zoom factor now.
 * @returns {number} The factor to go to: the next step up or down from `now`, or 1 to reset.
 */
function zoomFrom(action, now) {
  if (action === 'Reset') return 1;
  if (action === 'In') {
    return ZOOM_STEPS.find((step) => step > now + 0.001) ?? ZOOM_STEPS[ZOOM_STEPS.length - 1];
  }
  return [...ZOOM_STEPS].reverse().find((step) => step < now - 0.001) ?? ZOOM_STEPS[0];
}

/**
 * Options for a new page.
 * @typedef {object} PageOptions
 * @property {number} id The number the window chose for the page.
 * @property {number} width Its width in logical pixels.
 * @property {number} height Its height in logical pixels.
 * @property {string} folder The folder it paints its files into.
 * @property {Send} send Tells the window what the page reports.
 * @property {boolean} inspectable Whether it records its console and answers tools; the developer tools do not.
 */

/**
 * A page of the web, or the developer tools of one.
 */
class Page {
  /**
   * Makes the page and its offscreen window.
   * @param {PageOptions} options What the page is, and where it reports.
   */
  constructor({ id, width, height, folder, send, inspectable }) {
    /** @type {number} The number the window chose for the page. */
    this.id = id;
    /** @type {Send} Tells the window what the page reports. */
    this.send = send;
    /** @type {number | null} The scripting context the picker runs in, while pick mode is on. */
    this.pickContext = null;
    /** @type {boolean} Whether the page already listens for what the picker reports. */
    this.pickListening = false;
    /** @type {Surface} The files the page paints into. */
    this.surface = new Surface(folder, id, (w, h, paths) => send({ Surface: { page: id, width: w, height: h, paths } }));
    /** @type {import('electron').BrowserWindow} The offscreen window that holds the page. */
    this.win = new BrowserWindow({
      show: false,
      width,
      height,
      useContentSize: true,
      webPreferences: {
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        offscreen: true,
        backgroundThrottling: false,
      },
    });
    /** @type {import('electron').WebContents} What the window shows. */
    this.contents = this.win.webContents;
    /** @type {Cdp | null} The page's DevTools protocol, for pages that are inspectable. */
    this.cdp = null;
    /** @type {Recorder | null} What the page wrote to its console and asked of the network, for pages that are inspectable. */
    this.recorder = null;
    /** @type {Dialogs | null} The page's dialogs, for pages that are inspectable. */
    this.dialogs = null;
    /** @type {FileChooser | null} The page's file pickers, for pages that are inspectable. */
    this.fileChooser = null;
    /** @type {import('./emulation').Emulated | null} The viewport an agent set with `browser_resize` or `browser_emulate`, until it is reset. */
    this.viewport = null;
    /** @type {string | null} The user agent an agent set with `browser_emulate`, until it is reset. */
    this.userAgent = null;
    /** @type {InstanceType<typeof Restriction>} When the restriction on agent browsing applies to the page, and what it refused. */
    this.restriction = new Restriction();
    /** @type {{ url: string, origin: string, fingerprint: string } | null} The site whose untrusted certificate kept the page from loading, while the reader has not chosen. */
    this.certificate = null;
    this.contents.setFrameRate(60);
    this.contents.setWindowOpenHandler(({ url }) => {
      if (webAddress(url) && !this.guard(url)) send({ OpenRequested: { page: id, url, agent: this.agentBusy() } });
      return { action: 'deny' };
    });
    this.contents.on('will-navigate', (event, url) => {
      if (!webAddress(url) || this.guard(url)) event.preventDefault();
    });
    this.contents.on('will-frame-navigate', (event) => {
      if (event.isMainFrame ? this.guard(event.url) : this.guard(event.url, true)) event.preventDefault();
    });
    this.contents.on('will-redirect', (event) => {
      if (this.guard(event.url, !event.isMainFrame)) event.preventDefault();
    });
    this.contents.on('paint', (_event, _dirty, image) => this.paint(image));
    this.contents.on('page-title-updated', (_event, title) => send({ Title: { page: id, title } }));
    this.contents.on('did-navigate', (_event, url) => {
      send({ Address: { page: id, url } });
      this.reportZoom();
    });
    this.contents.on('found-in-page', (_event, result) =>
      send({ Found: { page: id, active: result.activeMatchOrdinal, total: result.matches } }),
    );
    this.contents.on('did-navigate-in-page', (_event, url) => send({ Address: { page: id, url } }));
    this.contents.on('cursor-changed', (_event, type) => send({ Cursor: { page: id, cursor: cursorOf(type) } }));
    if (inspectable) {
      this.cdp = new Cdp(this.contents);
      this.recorder = new Recorder(this.cdp);
      this.dialogs = new Dialogs(this);
      this.fileChooser = new FileChooser(this);
    }
  }

  /**
   * Whether a tool of an agent runs against the page or ended a moment ago, so that what the page starts may be the agent's doing.
   * @returns {boolean} Whether one does.
   */
  agentBusy() {
    return this.restriction.recent();
  }

  /**
   * Checks an address the page is about to go to, by a link, a redirect, a new window or a frame, against what agents may open; it only holds while a tool of an agent runs and for a few seconds after, never otherwise.
   * @param {string} url Where the page is going.
   * @param {boolean} [frame] Whether a frame inside the page goes there.
   * @returns {string | null} Why it may not, which the next answer of a tool will say, or nothing when it may.
   */
  guard(url, frame = false) {
    return this.restriction.guard(url, frame);
  }

  /**
   * Writes a painted image into the shared files and tells the window.
   * @param {import('electron').NativeImage} image What the page painted.
   * @returns {void}
   */
  paint(image) {
    const written = this.surface.write(image);
    if (written) {
      this.send({ Painted: { page: this.id, buffer: written.buffer, generation: written.generation } });
    }
  }

  /**
   * Opens an address.
   * @param {string} url Where to go.
   * @returns {void}
   */
  load(url) {
    this.contents.loadURL(url).catch(() => {});
  }

  /**
   * Carries out a request of the window that concerns this page and is not about opening or closing it.
   * @param {{ [name: string]: unknown }} request The request, as the window sent it.
   * @returns {void}
   */
  handle(request) {
    if (this.win.isDestroyed()) return;
    if (deliver(this.contents, request)) return;
    const asked = /** @type {PageRequest} */ (request);
    if ('Load' in asked) {
      this.load(asked.Load.url);
    } else if ('Back' in asked) {
      this.contents.navigationHistory.goBack();
    } else if ('Forward' in asked) {
      this.contents.navigationHistory.goForward();
    } else if ('Reload' in asked) {
      this.contents.reload();
    } else if ('HardReload' in asked) {
      this.contents.reloadIgnoringCache();
    } else if ('Find' in asked) {
      this.find(asked.Find.text, asked.Find.forward, asked.Find.again);
    } else if ('StopFind' in asked) {
      this.contents.stopFindInPage('keepSelection');
    } else if ('Zoom' in asked) {
      this.contents.setZoomFactor(zoomFrom(asked.Zoom.action, this.contents.getZoomFactor()));
      this.reportZoom();
    } else if ('ClearData' in asked) {
      this.clearData();
    } else if ('Resize' in asked) {
      this.win.setContentSize(asked.Resize.width, asked.Resize.height);
      this.holdViewport();
    } else if ('Focus' in asked) {
      if (asked.Focus.focused) this.contents.focus();
    } else if ('Tool' in asked) {
      this.runTool(asked.Tool);
    } else if ('Pick' in asked) {
      this.pick(asked.Pick.on);
    } else if ('Capture' in asked) {
      capture.run(this, asked.Capture.kind);
    } else if ('PickArea' in asked) {
      capture.pickArea(this, asked.PickArea.on);
    }
  }

  /**
   * Applies the size an agent chose with `browser_resize` again, so that the pane's own resizing leaves the emulated viewport as the agent set it.
   * @returns {void}
   */
  holdViewport() {
    if (!this.viewport || !this.cdp) return;
    emulation.applyViewport(this.cdp, this.viewport).catch(() => {});
  }

  /**
   * Tells the window the page's zoom factor.
   * @returns {void}
   */
  reportZoom() {
    this.send({ Zoomed: { page: this.id, factor: this.contents.getZoomFactor() } });
  }

  /**
   * Looks for text in the page and marks the matches; the window hears the count as `Found`.
   * @param {string} text What to look for; nothing clears the marks.
   * @param {boolean} forward Whether to look forward from the current match.
   * @param {boolean} again Whether this steps to the next match of the text already searched for.
   * @returns {void}
   */
  find(text, forward, again) {
    if (!text) {
      this.contents.stopFindInPage('clearSelection');
      this.send({ Found: { page: this.id, active: 0, total: 0 } });
      return;
    }
    this.contents.findInPage(text, { forward, findNext: again });
  }

  /**
   * Throws away the cookies, site storage and cache of the session every page shares, then loads the page again.
   * @returns {Promise<void>} Resolves once the page has been asked to load again.
   */
  async clearData() {
    const { session } = this.contents;
    try {
      await Promise.all([session.clearStorageData(), session.clearCache(), session.clearAuthCache()]);
    } catch (_) {
      /* a failed clear still loads the page again, which shows what is left */
    }
    if (!this.win.isDestroyed()) this.contents.reload();
  }

  /**
   * Runs a tool and tells the window how it ended.
   * @param {ToolBody} body The tool, and what the agent gave it.
   * @returns {Promise<void>} Resolves once the window has been told.
   */
  async runTool(body) {
    /** @type {ToolOutcome} */
    let outcome;
    if (!this.cdp) {
      outcome = { ok: false, text: 'These tools do not work on developer tools.', image: null };
    } else {
      /** @type {unknown} */
      let args;
      try {
        args = JSON.parse(body.arguments);
      } catch (_) {
        args = {};
      }
      const carried = this.restriction.takeRefusal();
      this.restriction.start(body.tool);
      try {
        outcome = await agent.run(this, body.tool, args && typeof args === 'object' ? /** @type {Record<string, any>} */ (args) : {});
      } catch (error) {
        outcome = { ok: false, text: String(error instanceof Error ? error.message : error), image: null };
      } finally {
        this.restriction.end(body.tool);
      }
      const fresh = this.restriction.takeRefusal();
      const moving = ['browser_navigate', 'browser_back', 'browser_forward', 'browser_reload'].includes(body.tool);
      if (fresh && moving) {
        outcome = { ok: false, text: fresh, image: null };
      } else if (fresh || carried) {
        const notes = [carried && `Earlier, after the last tool ended: ${carried}`, fresh].filter(Boolean).join('\n');
        outcome = { ...outcome, text: `${outcome.text}\n${notes}` };
      }
    }
    this.send({ ToolDone: { page: this.id, result: { call: body.call, ...outcome } } });
  }

  /**
   * Turns the mode on or off in which a click picks the element under the pointer.
   * @param {boolean} on Whether the mode is on.
   * @returns {Promise<void>} Resolves once the mode has changed.
   */
  async pick(on) {
    if (!this.cdp) return;
    try {
      if (on) {
        await pick.start(this, (element) => this.send({ Picked: { page: this.id, element } }));
      } else {
        await pick.stop(this);
      }
    } catch (_) {
      if (on) this.send({ Picked: { page: this.id, element: null } });
    }
  }

  /**
   * Closes the page.
   * @returns {void}
   */
  close() {
    this.surface.close();
    if (this.dialogs) this.dialogs.dispose();
    if (this.cdp) this.cdp.detach();
    this.restriction.dispose();
    if (!this.win.isDestroyed()) this.win.destroy();
  }
}

exports.Page = Page;
