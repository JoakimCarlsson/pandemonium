'use strict';

/**
 * One page: an offscreen window, the files it paints into, and what is said about it to the window
 * and asked of it.
 */

const { BrowserWindow } = require('electron');
const path = require('path');
const agent = require('./agent');
const capture = require('./capture');
const chrome = require('./chrome');
const { Cdp } = require('./cdp');
const { Dialogs } = require('./dialogs');
const emulation = require('./emulation');
const lifecycle = require('./lifecycle');
const { FileChooser } = require('./filechooser');
const { cursorOf, deliver } = require('./input');
const origins = require('./origins');
const pick = require('./pick');
const routing = require('./routing');
const { Recorder } = require('./recorder');
const { Restriction } = require('./restriction');
const sealed = require('./sealed');
const sealedPolicy = require('./sealed-policy');
const { Surface, displayScale, deviceSide, ENGINE_SCALE } = require('./surface');

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
 * A document shown sealed in a page: an agent wrote it, its scripts run and the network is denied to it.
 * @typedef {object} SealedDocument
 * @property {string} html The document.
 * @property {string} title What the page is called while the document names nothing.
 */

/**
 * Options for a new page.
 * @typedef {object} PageOptions
 * @property {number} id The number the window chose for the page.
 * @property {number} width Its width in logical pixels.
 * @property {number} height Its height in logical pixels.
 * @property {number} [scale] The display scale of the pane it is shown in, device pixels per logical pixel; 1 when not given.
 * @property {string} folder The folder it paints its files into.
 * @property {Send} send Tells the window what the page reports.
 * @property {boolean} inspectable Whether it records its console and answers tools; the developer tools and sealed pages do not.
 * @property {SealedDocument} [sealed] The document the page shows sealed from the network, in place of an address; such a page is never inspectable.
 * @property {import('electron').BrowserWindow} [window] The offscreen window another page opened with `window.open`, which the page takes over in place of making one.
 */

/**
 * A page of the web, or the developer tools of one.
 */
class Page {
  /**
   * Makes the page and its offscreen window.
   * @param {PageOptions} options What the page is, and where it reports.
   */
  constructor({ id, width, height, scale, folder, send, inspectable, sealed: document, window: opened }) {
    /** @type {number} The number the window chose for the page. */
    this.id = id;
    /** @type {Send} Tells the window what the page reports. */
    this.send = send;
    /** @type {number | null} The scripting context the picker runs in, while pick mode is on. */
    this.pickContext = null;
    /** @type {boolean} Whether the page already listens for what the picker reports. */
    this.pickListening = false;
    /** @type {SealedDocument | null} The document the page shows sealed from the network, when it shows one. */
    this.sealed = document || null;
    /** @type {number} The display scale of the pane the page is shown in, as the window last gave it. */
    this.paneScale = displayScale(scale);
    /** @type {{ width: number, height: number }} The size of the pane, in logical pixels, as the window last gave it. */
    this.size = { width, height };
    /** @type {number} The part of the contents' zoom that carries the pane's display scale: the scale, or 1 while an emulated pixel ratio carries it or before the page set it. */
    this.zoomBase = 1;
    /** @type {number} The zoom the reader chose, on top of {@link Page#zoomBase}. */
    this.zoom = 1;
    /** @type {Cdp | null} The protocol a page that is not inspectable is reached through to render at the pane's scale, once one is above 1. */
    this.scaler = null;
    /** @type {Surface} The files the page paints into. */
    this.surface = new Surface(folder, id, (w, h, paths) => send({ Surface: { page: id, width: w, height: h, paths } }));
    /** @type {import('electron').BrowserWindow} The offscreen window that holds the page. */
    this.win = opened || new BrowserWindow({
      show: false,
      width: deviceSide(width, this.paneScale),
      height: deviceSide(height, this.paneScale),
      useContentSize: true,
      webPreferences: this.sealed
        ? sealed.preferences(path.join(__dirname, 'popups-preload.js'), ENGINE_SCALE)
        : {
            sandbox: true,
            contextIsolation: true,
            nodeIntegration: false,
            offscreen: { deviceScaleFactor: ENGINE_SCALE },
            preload: path.join(__dirname, 'popups-preload.js'),
            nodeIntegrationInSubFrames: true,
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
    if (this.sealed) {
      const { title: named } = this.sealed;
      sealed.guard(this.contents);
      send({ Title: { page: id, title: named } });
      this.contents.on('page-title-updated', (_event, title) =>
        send({ Title: { page: id, title: title.startsWith('data:') || !title.trim() ? named : title } }),
      );
      this.contents.on('did-navigate', () => this.reportZoom());
    } else {
      this.contents.setWindowOpenHandler(({ url, disposition }) => {
        const refused = webAddress(url) ? this.guard(url) : null;
        if (!refused && this.popupAllowed(url, disposition)) {
          return { action: 'allow', outlivesOpener: true, overrideBrowserWindowOptions: lifecycle.popupOptions(ENGINE_SCALE) };
        }
        if (webAddress(url) && !refused) send({ OpenRequested: { page: id, url, agent: this.agentBusy() } });
        return { action: 'deny' };
      });
      this.contents.on('did-create-window', (child, details) => lifecycle.hold(this, child, details.url, this.agentBusy(), this.paneScale));
      lifecycle.watch(this);
      this.contents.on('will-navigate', (event, url) => {
        if (!webAddress(url) || this.guard(url)) event.preventDefault();
      });
      this.contents.on('will-frame-navigate', (event) => {
        if (event.isMainFrame ? this.guard(event.url) : this.guard(event.url, true)) event.preventDefault();
      });
      this.contents.on('will-redirect', (event) => {
        if (this.guard(event.url, !event.isMainFrame)) event.preventDefault();
      });
      this.contents.on('page-title-updated', (_event, title) => send({ Title: { page: id, title } }));
      this.contents.on('did-navigate', (_event, url) => {
        this.holdZoom();
        send({ Address: { page: id, url } });
        this.reportZoom();
      });
      this.contents.on('did-navigate-in-page', (_event, url) => send({ Address: { page: id, url } }));
    }
    this.contents.on('paint', (_event, dirty, image) => this.paint(image, dirty));
    this.contents.on('found-in-page', (_event, result) =>
      send({ Found: { page: id, active: result.activeMatchOrdinal, total: result.matches } }),
    );
    this.contents.on('cursor-changed', (_event, type) => send({ Cursor: { page: id, cursor: cursorOf(type) } }));
    if (inspectable && !this.sealed) {
      this.cdp = new Cdp(this.contents);
      this.contents.setZoomMode('isolated');
      routing.install(this.cdp, this.contents, () => this.paneScale * this.readerZoom());
      this.recorder = new Recorder(this.cdp);
      this.dialogs = new Dialogs(this);
      this.fileChooser = new FileChooser(this);
    }
    if (inspectable || this.sealed) chrome.watch(this);
    this.contents.on('did-start-navigation', (details) => {
      if (details.isMainFrame && !details.isSameDocument) this.applyScale();
    });
    if (opened) this.win.setContentSize(deviceSide(width, this.paneScale), deviceSide(height, this.paneScale));
    this.applyScale();
  }

  /**
   * Whether a tool of an agent runs against the page or ended a moment ago, so that what the page starts may be the agent's doing.
   * @returns {boolean} Whether one does.
   */
  agentBusy() {
    return this.restriction.recent();
  }

  /**
   * Whether a window the page asks for may open as a real window that keeps `window.opener`, as a sign-in or payment
   * flow needs: only for `window.open` with features, to a web address or, when the reader asked, a blank one the
   * script fills in; a link with `target=_blank` opens as a plain tab. The agent restriction is checked by the caller.
   * @param {string} url Where the window goes.
   * @param {string} disposition How the page asked for it; `new-window` is `window.open` with features.
   * @returns {boolean} Whether it may.
   */
  popupAllowed(url, disposition) {
    if (disposition !== 'new-window') return false;
    if (url === '' || url === 'about:blank') return !this.agentBusy();
    return webAddress(url);
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
   * @param {import('electron').Rectangle} [dirty] The part that changed; the whole picture when missing.
   * @returns {void}
   */
  paint(image, dirty) {
    const written = this.surface.write(image, dirty);
    if (written) {
      this.send({ Painted: { page: this.id, buffer: written.buffer, generation: written.generation } });
    }
  }

  /**
   * Opens an address. A sealed page stays on its document: it is not sent anywhere.
   * @param {string} url Where to go.
   * @returns {void}
   */
  load(url) {
    if (this.sealed) return;
    this.contents.loadURL(url).catch(() => {});
  }

  /**
   * Loads the document of a sealed page; a document that may not be shown sealed is replaced by a line that says why.
   * @returns {void}
   */
  showSealed() {
    if (!this.sealed) return;
    const refused = sealedPolicy.refusal(this.sealed.html);
    const html = refused ? `<!doctype html><title>Not shown</title><p style="font: 14px sans-serif">${refused}</p>` : this.sealed.html;
    this.contents.loadURL(sealedPolicy.address(html)).catch(() => {});
  }

  /**
   * Carries out a request of the window that concerns this page and is not about opening or closing it.
   * @param {{ [name: string]: unknown }} request The request, as the window sent it.
   * @returns {void}
   */
  handle(request) {
    if (this.win.isDestroyed()) return;
    if (deliver(this.contents, request, (event) => this.send(event), this.paneScale)) return;
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
    } else if ('Stop' in asked) {
      this.contents.stop();
    } else if ('Find' in asked) {
      this.find(asked.Find.text, asked.Find.forward, asked.Find.again);
    } else if ('StopFind' in asked) {
      this.contents.stopFindInPage('keepSelection');
    } else if ('Zoom' in asked) {
      this.setReaderZoom(zoomFrom(asked.Zoom.action, this.zoom));
      this.reportZoom();
    } else if ('ClearData' in asked) {
      if (!this.sealed) this.clearData();
    } else if ('Resize' in asked) {
      this.resize(asked.Resize.width, asked.Resize.height, asked.Resize.scale);
    } else if ('Focus' in asked) {
      this.focused(asked.Focus.focused);
    } else if ('Tool' in asked) {
      this.runTool(asked.Tool);
    } else if ('Pick' in asked) {
      this.pick(asked.Pick.on);
    } else if ('Capture' in asked) {
      capture.run(this, asked.Capture.kind);
    } else if ('PickArea' in asked) {
      capture.pickArea(this, asked.PickArea.on);
    } else if ('Chrome' in asked) {
      chrome.act(this, asked.Chrome.action);
    }
  }

  /**
   * Gives the page the keyboard or takes it away. The hidden window itself is focused, not only the contents, because
   * Chromium treats a page in an unfocused window as inactive: it draws no caret or focus ring and paints a selection grey.
   * @param {boolean} focused Whether the page has the keyboard.
   * @returns {void}
   */
  focused(focused) {
    if (focused) {
      this.win.focus();
      this.contents.focus();
    } else {
      this.win.blur();
    }
  }

  /**
   * Sizes the window to what the page shows, in device pixels: the viewport an agent set with `browser_resize` or
   * `browser_emulate`, or else the pane, times the pane's display scale. A viewport is emulated by this size and not by
   * the engine's fixed-size override, which would paint it at a device scale of 1.
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @returns {void}
   */
  fitWindow(viewport) {
    const shown = viewport || this.size;
    this.win.setContentSize(deviceSide(shown.width, this.paneScale), deviceSide(shown.height, this.paneScale));
  }

  /**
   * Sets the viewport an agent asked for, or with nothing gives it back to the pane: sizes the window for it and has the
   * engine render it at the pane's display scale. The caller keeps it in {@link Page#viewport}.
   * @param {import('./emulation').Emulated | null} viewport The viewport, or nothing.
   * @returns {Promise<void>} Resolves when the engine has taken it; rejects when the page has no protocol.
   */
  async emulateViewport(viewport) {
    if (!this.cdp) throw new Error('the page has no protocol');
    this.fitWindow(viewport);
    this.carryScale(viewport);
    await emulation.applyViewport(this.cdp, viewport, this.paneScale);
  }

  /**
   * Follows the pane's size and display scale. The window is sized in device pixels, the logical size times the scale
   * ({@link Page#fitWindow}), and the engine is told to lay the page out at the logical size and render it at that scale
   * ({@link Page#applyScale}), so a frame is always `width * scale` by `height * scale` and sharp, also after the window
   * moves to a display of another scale. A viewport an agent set keeps its size.
   * @param {number} width The pane's width, in logical pixels.
   * @param {number} height The pane's height, in logical pixels.
   * @param {unknown} scale The scale the window gave, device pixels per logical pixel.
   * @returns {void}
   */
  resize(width, height, scale) {
    const wanted = displayScale(scale);
    const changed = wanted !== this.paneScale;
    this.paneScale = wanted;
    this.size = { width, height };
    this.fitWindow(this.viewport);
    if (changed) this.applyScale();
  }

  /**
   * Tells the engine to render the page at the pane's display scale in its window of device pixels: by the page's zoom
   * ({@link Page#carryScale}) and its own protocol, with the viewport an agent set, or, for the developer tools and
   * sealed pages, through a debugger attached for this once the scale is above 1. It is sent again as each page starts loading, since a new document can
   * come up before an override reaches it.
   * @returns {void}
   */
  applyScale() {
    if (this.win.isDestroyed()) return;
    if (this.cdp) {
      this.carryScale(this.viewport);
      emulation.applyViewport(this.cdp, this.viewport, this.paneScale).catch(() => {});
      return;
    }
    if (!this.scaler && this.paneScale === 1) return;
    if (!this.scaler) {
      try {
        this.scaler = new Cdp(this.contents);
      } catch (_) {
        return;
      }
    }
    this.scaler.call(...emulation.paneMetrics(this.paneScale)).catch(() => {});
  }

  /**
   * Sets the part of the contents' zoom that carries the pane's display scale, keeping the reader's zoom: the scale when
   * {@link emulation.scaledByZoom} holds for the viewport, else 1. The contents' zoom mode is isolated, so it stays
   * through navigation and touches no other page of the site.
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @returns {void}
   */
  carryScale(viewport) {
    this.zoomBase = emulation.scaledByZoom(viewport) ? this.paneScale : 1;
    emulation.setZoomBase(this.contents, this.zoomBase);
    this.holdZoom();
  }

  /**
   * Gives the contents the zoom they should have, the reader's on top of the part that carries the pane's display
   * scale, when they lost it: a zoom set before the first document came up does not stay.
   * @returns {void}
   */
  holdZoom() {
    const wanted = this.zoom * this.zoomBase;
    if (Math.abs(this.contents.getZoomFactor() - wanted) > 0.001) this.contents.setZoomFactor(wanted);
  }

  /**
   * The zoom the reader sees: the contents' zoom without the part that carries the pane's display scale, which is the
   * zoom the reader chose unless the engine held it to its limits.
   * @returns {number} The zoom factor.
   */
  readerZoom() {
    return this.contents.getZoomFactor() / this.zoomBase;
  }

  /**
   * Sets the zoom the reader chose, on top of the part that carries the pane's display scale.
   * @param {number} factor The zoom factor.
   * @returns {void}
   */
  setReaderZoom(factor) {
    this.zoom = factor;
    this.holdZoom();
  }

  /**
   * Tells the window the zoom the reader chose.
   * @returns {void}
   */
  reportZoom() {
    this.send({ Zoomed: { page: this.id, factor: this.readerZoom() } });
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
      const why = this.sealed
        ? 'This tab shows a document an agent drew for the reader, sealed from the network; the browser tools do not work on it. Open a tab of your own with browser_tabs.'
        : 'These tools do not work on developer tools.';
      outcome = { ok: false, text: why, image: null };
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
    if (this.scaler) this.scaler.detach();
    this.restriction.dispose();
    if (!this.win.isDestroyed()) this.win.destroy();
  }
}

exports.Page = Page;
