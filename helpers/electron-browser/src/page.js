'use strict';

/**
 * One page: an offscreen window, the files it paints into, and what is said about it to the window
 * and asked of it.
 */

const { BrowserWindow } = require('electron');
const agent = require('./agent');
const { Cdp } = require('./cdp');
const { cursorOf, deliver } = require('./input');
const pick = require('./pick');
const { Recorder } = require('./recorder');
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
    this.contents.setFrameRate(60);
    this.contents.setWindowOpenHandler(({ url }) => {
      if (webAddress(url)) this.contents.loadURL(url).catch(() => {});
      return { action: 'deny' };
    });
    this.contents.on('will-navigate', (event, url) => {
      if (!webAddress(url)) event.preventDefault();
    });
    this.contents.on('paint', (_event, _dirty, image) => this.paint(image));
    this.contents.on('page-title-updated', (_event, title) => send({ Title: { page: id, title } }));
    this.contents.on('did-navigate', (_event, url) => send({ Address: { page: id, url } }));
    this.contents.on('did-navigate-in-page', (_event, url) => send({ Address: { page: id, url } }));
    this.contents.on('cursor-changed', (_event, type) => send({ Cursor: { page: id, cursor: cursorOf(type) } }));
    if (inspectable) {
      this.cdp = new Cdp(this.contents);
      this.recorder = new Recorder(this.cdp);
    }
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
    } else if ('Resize' in asked) {
      this.win.setContentSize(asked.Resize.width, asked.Resize.height);
    } else if ('Focus' in asked) {
      if (asked.Focus.focused) this.contents.focus();
    } else if ('Tool' in asked) {
      this.runTool(asked.Tool);
    } else if ('Pick' in asked) {
      this.pick(asked.Pick.on);
    }
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
      try {
        outcome = await agent.run(this, body.tool, args && typeof args === 'object' ? /** @type {Record<string, any>} */ (args) : {});
      } catch (error) {
        outcome = { ok: false, text: String(error instanceof Error ? error.message : error), image: null };
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
    if (this.cdp) this.cdp.detach();
    if (!this.win.isDestroyed()) this.win.destroy();
  }
}

exports.Page = Page;
