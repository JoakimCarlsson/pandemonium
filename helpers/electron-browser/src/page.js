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
const recording = require('./recording');
const { Restriction } = require('./restriction');
const { Screencast } = require('./screencast');
const sealed = require('./sealed');
const sealedPolicy = require('./sealed-policy');
const { Surface, displayScale, deviceSide, ENGINE_SCALE } = require('./surface');
const partitions = require('./partitions');

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
 * @property {string} [partition] The profile it lives in, by the engine's name for it, or the one a taken window already lives in; the shared one when not given.
 */

/**
 * A page of the web, or the developer tools of one.
 */
class Page {
  /**
   * Makes the page and its offscreen window.
   * @param {PageOptions} options What the page is, and where it reports.
   */
  constructor({ id, width, height, scale, folder, send, inspectable, sealed: document, window: opened, partition }) {
    /** @type {number} The number the window chose for the page. */
    this.id = id;
    /** @type {Send} Tells the window what the page reports. */
    this.send = send;
    /** @type {boolean} Whether the page records its console and answers tools. */
    this.inspectable = Boolean(inspectable);
    /** @type {SealedDocument | null} The document the page shows sealed from the network, when it shows one. */
    this.sealed = document || null;
    /** @type {string} The profile the page lives in, by the engine's name for it; empty for the shared one, and always so for a sealed page. */
    this.partition = this.sealed ? '' : partition || '';
    /** @type {number} The display scale of the pane the page is shown in, as the window last gave it. */
    this.paneScale = displayScale(scale);
    /** @type {{ width: number, height: number }} The size of the pane, in logical pixels, as the window last gave it. */
    this.size = { width, height };
    /** @type {number} The device scale the engine paints the page's window at: {@link ENGINE_SCALE}, or the pixel ratio of a mobile viewport an agent set ({@link emulation.engineScale}). */
    this.engineScale = ENGINE_SCALE;
    /** @type {number} The zoom the contents should have: the share of the pane's scale and the reader's zoom that {@link emulation.rendering} gives the zoom, or the reader's zoom alone for a page without a protocol of its own. */
    this.engineZoom = 1;
    /** @type {number} The zoom the reader chose. */
    this.zoom = 1;
    /** @type {Cdp | null} The protocol a page that is not inspectable is reached through to render at the pane's scale, once one is above 1. */
    this.scaler = null;
    /** @type {Surface} The files the page paints into. */
    this.surface = new Surface(folder, id, (w, h, paths) => send({ Surface: { page: id, width: w, height: h, paths } }));
    /** @type {import('./emulation').Emulated | null} The viewport an agent set with `browser_resize` or `browser_emulate`, until it is reset. */
    this.viewport = null;
    /** @type {string | null} The user agent an agent set with `browser_emulate`, until it is reset. */
    this.userAgent = null;
    /** @type {InstanceType<typeof Restriction>} When the restriction on agent browsing applies to the page, and what it refused. */
    this.restriction = new Restriction();
    /** @type {boolean} Whether the window asked for the page to close, so the page's closing is not reported back to it. */
    this.closedByWindow = false;
    /** @type {{ url: string, origin: string, fingerprint: string } | null} The site whose untrusted certificate kept the page from loading, while the reader has not chosen. */
    this.certificate = null;
    /** @type {((page: Page) => void) | null} Starts following the page's contents for what is watched from outside it, again for each window it moves into ({@link Page#rebuild}). */
    this.rewatch = null;
    this.adopt(opened || this.makeWindow(), Boolean(opened));
    this.applyScale();
  }

  /**
   * Makes an offscreen window for the page, painted at {@link Page#engineScale} and sized for what it shows
   * ({@link Page#windowSize}).
   * @returns {import('electron').BrowserWindow} The window, hidden.
   */
  makeWindow() {
    const size = this.windowSize(this.viewport);
    return new BrowserWindow({
      show: false,
      ...size,
      useContentSize: true,
      webPreferences: this.sealed
        ? sealed.preferences(path.join(__dirname, 'popups-preload.js'), ENGINE_SCALE)
        : {
            sandbox: true,
            contextIsolation: true,
            nodeIntegration: false,
            offscreen: { deviceScaleFactor: this.engineScale },
            preload: path.join(__dirname, 'popups-preload.js'),
            nodeIntegrationInSubFrames: true,
            backgroundThrottling: false,
            session: partitions.sessionOf(this.partition),
          },
    });
  }

  /**
   * Takes `win` as the page's window and starts following what it shows: its painting, navigation, title and cursor,
   * and for an inspectable page its protocol, cross-origin input, console, dialogs and file pickers.
   * @param {import('electron').BrowserWindow} win The window.
   * @param {boolean} opened Whether another page opened it with `window.open`, so the engine gives it no offscreen view
   *   and it is painted through a screencast.
   * @returns {void}
   */
  adopt(win, opened) {
    const { id, send } = this;
    /** @type {import('electron').BrowserWindow} The offscreen window that holds the page. */
    this.win = win;
    /** @type {import('electron').WebContents} What the window shows. */
    this.contents = this.win.webContents;
    /** @type {number | null} The scripting context the picker runs in, while pick mode is on. */
    this.pickContext = null;
    /** @type {boolean} Whether the page already listens for what the picker reports. */
    this.pickListening = false;
    /** @type {InstanceType<typeof Screencast> | null} How a window another page opened is sized and painted, since the engine gives it no offscreen view. */
    this.screencast = null;
    /** @type {Cdp | null} The page's DevTools protocol, for pages that are inspectable. */
    this.cdp = null;
    /** @type {Recorder | null} What the page wrote to its console and asked of the network, for pages that are inspectable. */
    this.recorder = null;
    /** @type {Dialogs | null} The page's dialogs, for pages that are inspectable. */
    this.dialogs = null;
    /** @type {FileChooser | null} The page's file pickers, for pages that are inspectable. */
    this.fileChooser = null;
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
          return {
            action: 'allow',
            outlivesOpener: true,
            overrideBrowserWindowOptions: lifecycle.popupOptions(ENGINE_SCALE, partitions.sessionOf(this.partition)),
          };
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
    if (this.inspectable && !this.sealed) {
      this.cdp = new Cdp(this.contents);
      this.contents.setZoomMode('isolated');
      routing.install(this.cdp, this.contents, () => this.paneScale * this.readerZoom(), () => this.topUnits());
      this.recorder = new Recorder(this.cdp);
      this.dialogs = new Dialogs(this);
      this.fileChooser = new FileChooser(this);
      if (opened) this.screencast = new Screencast(this.cdp, (image) => this.paint(image));
    }
    if (this.inspectable || this.sealed) chrome.watch(this);
    this.contents.on('did-start-navigation', (details) => {
      if (details.isMainFrame && !details.isSameDocument) this.applyScale();
    });
    if (!opened) {
      const size = this.windowSize(this.viewport);
      this.win.setContentSize(size.width, size.height);
    }
  }

  /**
   * Moves the page into a new offscreen window painted at the device scale `scale`, since the engine fixes a window's
   * scale when it makes it, and only that scale reaches cross-origin frames while a mobile viewport is emulated. The
   * old window is closed without telling the window the page closed, and the new one takes over the page's history
   * and what it emulates; the page's document is loaded again from its history once `resume` is called. A window
   * another page opened, a sealed page and a page without a protocol are never moved, nor one already at `scale`.
   * @param {number} scale The device scale for the new window.
   * @returns {(() => Promise<void>) | null} Loads the page's history into the new window, to call once its rendering
   *   is set; nothing when the page was not moved.
   */
  rebuild(scale) {
    if (this.screencast || this.sealed || !this.cdp || this.win.isDestroyed() || scale === this.engineScale) return null;
    const history = this.contents.navigationHistory;
    const entries = history.getAllEntries();
    const index = history.getActiveIndex();
    const old = this.win;
    if (this.dialogs) this.dialogs.dispose();
    this.cdp.detach();
    this.engineScale = scale;
    this.adopt(this.makeWindow(), false);
    old.destroy();
    emulation.setReaderZoom(this.contents, this.zoom);
    if (this.rewatch) this.rewatch(this);
    const { cdp, contents, userAgent } = this;
    return async () => {
      const agent = userAgent ? emulation.applyUserAgent(cdp, userAgent).catch(() => {}) : null;
      if (entries.length > 0 && index >= 0) await contents.navigationHistory.restore({ entries, index }).catch(() => {});
      if (agent) await agent;
    };
  }

  /**
   * The size of the page's window, in the engine's pixels of it: the viewport an agent set, or else the pane, times
   * the pane's display scale, over the window's {@link Page#engineScale}, rounded up so the window paints at least
   * that many device pixels ({@link Page#paint} cuts what lies past them).
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @returns {{ width: number, height: number }} The window's content size.
   */
  windowSize(viewport) {
    const shown = viewport || this.size;
    return {
      width: Math.ceil(deviceSide(shown.width, this.paneScale) / this.engineScale),
      height: Math.ceil(deviceSide(shown.height, this.paneScale) / this.engineScale),
    };
  }

  /**
   * The window's own pixels per logical pixel of the pane, which Electron's input events and menus are given in: the
   * pane's display scale over the window's {@link Page#engineScale}.
   * @returns {number} The ratio.
   */
  inputScale() {
    return this.paneScale / this.engineScale;
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
   * Writes a painted image into the shared files and tells the window. A window painted at an engine scale above 1 is
   * rounded up to whole pixels of its own ({@link Page#windowSize}), so what it paints past the page's device size is
   * cut off, and the whole picture is written, since the part that changed is then given in other units.
   * @param {import('electron').NativeImage} image What the page painted.
   * @param {import('electron').Rectangle} [dirty] The part that changed; the whole picture when missing.
   * @returns {void}
   */
  paint(image, dirty) {
    let picture = image;
    let changed = dirty;
    if (this.engineScale !== ENGINE_SCALE && !this.screencast) {
      const shown = this.viewport || this.size;
      const painted = image.getSize();
      const width = Math.min(painted.width, deviceSide(shown.width, this.paneScale));
      const height = Math.min(painted.height, deviceSide(shown.height, this.paneScale));
      if (width !== painted.width || height !== painted.height) picture = image.crop({ x: 0, y: 0, width, height });
      changed = undefined;
    }
    const written = this.surface.write(picture, changed);
    if (written) {
      this.send({
        Painted: { page: this.id, buffer: written.buffer, generation: written.generation, dirty: written.dirty },
      });
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
    if (deliver(this.contents, request, (event) => this.send(event), this.paneScale, this.inputScale())) return;
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
    } else if ('RecordStep' in asked) {
      this.recordStep(asked.RecordStep);
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
   * `browser_emulate`, or else the pane, times the pane's display scale, over the window's engine scale
   * ({@link Page#windowSize}). A viewport is emulated by this size and not by the engine's fixed-size override, which
   * would paint it at the window's engine scale. A window another page opened has no view to size, so its page is sized
   * through its screencast instead ({@link Page#fitFrame}).
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @returns {void}
   */
  fitWindow(viewport) {
    if (this.screencast) {
      this.fitFrame(viewport).catch(() => {});
      return;
    }
    const size = this.windowSize(viewport);
    this.win.setContentSize(size.width, size.height);
  }

  /**
   * Sizes the page of a window another page opened through its screencast: to the viewport an agent set, or else the
   * pane, in device pixels, and has it render at the pane's display scale. Nothing without a screencast.
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @returns {Promise<void>} Resolves when the engine has taken it; rejects with the protocol's error.
   */
  async fitFrame(viewport) {
    if (!this.screencast) return;
    const shown = viewport || this.size;
    const device = { width: deviceSide(shown.width, this.paneScale), height: deviceSide(shown.height, this.paneScale) };
    await this.screencast.fit(device, viewport, this.paneScale, this.zoom);
  }

  /**
   * Sets the viewport an agent asked for, or with nothing gives it back to the pane: moves the page into a window of
   * the engine scale the viewport needs ({@link emulation.engineScale}, {@link Page#rebuild}), sizes the window for it,
   * loads the page again when it moved, since the engine answers a new window's protocol only once it shows a
   * document, and has the engine render it at the pane's display scale. It is kept in
   * {@link Page#viewport} before anything is sent, so a rendering sent meanwhile, for a zoom, a resize or a navigation,
   * is already the new one's.
   * @param {import('./emulation').Emulated | null} viewport The viewport, or nothing.
   * @returns {Promise<void>} Resolves when the engine has taken it and a moved page has loaded again; rejects when the
   *   page has no protocol.
   */
  async emulateViewport(viewport) {
    if (!this.cdp) throw new Error('the page has no protocol');
    this.viewport = viewport;
    const resume = this.rebuild(emulation.engineScale(viewport, this.paneScale));
    this.carryScale(viewport);
    if (this.screencast) {
      await this.fitFrame(viewport);
      return;
    }
    this.fitWindow(viewport);
    const restoring = resume ? resume() : null;
    await emulation.applyViewport(this.cdp, viewport, this.paneScale, this.zoom, this.engineScale);
    if (restoring) await restoring;
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
    if (changed && this.cdp && emulation.engineScale(this.viewport, wanted) !== this.engineScale) {
      this.emulateViewport(this.viewport).catch(() => {});
      return;
    }
    this.fitWindow(this.viewport);
    if (changed) this.applyScale();
  }

  /**
   * Tells the engine to render the page at the pane's display scale and the reader's zoom in its window of device
   * pixels: by the page's zoom ({@link Page#carryScale}) and its own protocol, with the viewport an agent set, or, for the developer tools and
   * sealed pages, through a debugger attached for this once the scale is above 1. It is sent again as each page starts loading, since a new document can
   * come up before an override reaches it.
   * @returns {void}
   */
  applyScale() {
    if (this.win.isDestroyed()) return;
    if (this.cdp) {
      this.carryScale(this.viewport);
      if (this.screencast) this.fitFrame(this.viewport).catch(() => {});
      else emulation.applyViewport(this.cdp, this.viewport, this.paneScale, this.zoom, this.engineScale).catch(() => {});
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
   * Sets the contents' zoom to its share of the pane's display scale and the reader's zoom for the viewport
   * ({@link emulation.rendering}); the override carries the rest. The contents' zoom mode is isolated, so it stays
   * through navigation and touches no other page of the site.
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @returns {void}
   */
  carryScale(viewport) {
    this.engineZoom = emulation.rendering(viewport, this.paneScale, this.zoom, null, this.engineScale).zoom;
    this.holdZoom();
  }

  /**
   * Gives the contents the zoom they should have, {@link Page#engineZoom}, when they lost it: a zoom set before the
   * first document came up does not stay.
   * @returns {void}
   */
  holdZoom() {
    if (Math.abs(this.contents.getZoomFactor() - this.engineZoom) > 0.001) this.contents.setZoomFactor(this.engineZoom);
  }

  /**
   * The window's pixels per unit of a point the page's own protocol target takes, for input and drops. The engine takes
   * points in CSS pixels times the override's `scale`: in a window of its own, that is the window's pixels over the
   * contents' zoom and the window's engine scale; in a window drawn by its screencast, whose override has no `scale`,
   * plain CSS pixels, so the pane's scale times the reader's zoom.
   * @returns {number} The window's pixels per unit.
   */
  topUnits() {
    if (this.screencast) return this.paneScale * this.zoom;
    return (this.contents.getZoomFactor() || 1) * this.engineScale;
  }

  /**
   * The zoom the reader chose, which the page shows whole: what the contents' zoom cannot carry, the override does.
   * @returns {number} The zoom factor.
   */
  readerZoom() {
    return this.zoom;
  }

  /**
   * Sets the zoom the reader chose and renders the page at it: a page with a protocol of its own through
   * {@link Page#applyScale}, any other by its contents' zoom alone.
   * @param {number} factor The zoom factor.
   * @returns {void}
   */
  setReaderZoom(factor) {
    this.zoom = factor;
    emulation.setReaderZoom(this.contents, factor);
    if (this.cdp) {
      this.applyScale();
      return;
    }
    this.engineZoom = factor;
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
    const caller = { session: body.session || '', name: body.session_name || '' };
    /** @type {boolean[]} */
    let secret = [];
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
      const given = args && typeof args === 'object' ? /** @type {Record<string, any>} */ (args) : {};
      secret = recording.typesSecrets(body.tool, caller) ? await agent.secretFields(this, body.tool, given) : [];
      const carried = this.restriction.takeRefusal();
      this.restriction.start(body.tool);
      try {
        outcome = await agent.run(this, body.tool, given, caller);
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
    const step = this.cdp ? await recording.afterTool(this, body.tool, body.arguments, outcome, caller, secret) : null;
    this.send({ ToolDone: { page: this.id, result: step ? { call: body.call, ...outcome, step } : { call: body.call, ...outcome } } });
  }

  /**
   * Keeps a tool call the window answered itself, such as `browser_tabs`, in the recording of the session that made it,
   * with a screenshot of this page after it, and tells the window of the step; nothing is told when the session is not
   * recorded.
   * @param {import('./types').RecordStepBody} body The call, what it came to, and the session that made it.
   * @returns {Promise<void>} Resolves once the step is kept.
   */
  async recordStep(body) {
    const caller = { session: body.session || '', name: body.session_name || '' };
    const outcome = { ok: body.ok === true, text: String(body.text || ''), image: null };
    const step = await recording.afterTool(this.cdp ? this : null, body.tool, body.arguments, outcome, caller);
    if (step) this.send({ ToolDone: { page: this.id, result: { call: body.call, ...outcome, step } } });
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
    this.closedByWindow = true;
    this.surface.close();
    if (this.dialogs) this.dialogs.dispose();
    if (this.cdp) this.cdp.detach();
    if (this.screencast) this.screencast.dispose();
    if (this.scaler) this.scaler.detach();
    this.restriction.dispose();
    if (!this.win.isDestroyed()) this.win.destroy();
  }
}

exports.Page = Page;
