'use strict';

/**
 * Where a page is in its life, told to the window: loading and how far it got, a main frame that did not load, a
 * content process that went away or stopped answering, how far its history goes; and the windows pages open with
 * `window.open`, held here until the window makes each a page of its own or drops it.
 */

const path = require('path');

/** @typedef {import('./types').Send} Send */

/** The engine's number for a load that was stopped or replaced by another, which is no failure to show. */
const ABORTED = -3;

/** The lowest and highest of the engine's numbers for certificate errors, which the certificate page shows instead. */
const CERTIFICATE_ERRORS = [-299, -200];

/** How long a window a page opened is held for the window to take it, in milliseconds, before it is closed. */
const HOLD_FOR = 30_000;

/**
 * What a page reports through `watch`: the parts of a page this file needs.
 * @typedef {object} Watched
 * @property {number} id The number the window chose for the page.
 * @property {Send} send Tells the window what the page reports.
 * @property {import('electron').BrowserWindow} win The offscreen window that holds the page.
 * @property {import('electron').WebContents} contents What the window shows.
 */

/**
 * Whether the engine's error `code` for a main frame is one the window shows its error view for.
 * @param {number} code The engine's number for the error.
 * @returns {boolean} Whether it is: every error but a stopped load and a certificate the reader is asked about.
 */
function shownFailure(code) {
  return code !== ABORTED && !(code >= CERTIFICATE_ERRORS[0] && code <= CERTIFICATE_ERRORS[1]);
}

/**
 * Tells the window whether the page's history goes back and forward from where it is.
 * @param {Watched} page The page.
 * @returns {void}
 */
function reportHistory(page) {
  if (page.contents.isDestroyed()) return;
  const history = page.contents.navigationHistory;
  page.send({ History: { page: page.id, back: history.canGoBack(), forward: history.canGoForward() } });
}

/**
 * Listens to a page for its loading, its failures, its content process and its history, and tells the window, and
 * for the page closing itself.
 * @param {Watched} page The page.
 * @returns {void}
 */
function watch(page) {
  const { id, send, contents, win } = page;
  /**
   * Tells the window the page is loading and how far it got.
   * @param {number} progress How far, from 0 to 1.
   * @returns {void}
   */
  const loading = (progress) => send({ Loading: { page: id, loading: true, progress } });
  contents.on('did-start-loading', () => loading(0.1));
  contents.on('did-start-navigation', (details) => {
    if (details.isMainFrame && !details.isSameDocument) loading(0.25);
  });
  contents.on('did-navigate', () => {
    if (contents.isLoading()) loading(0.5);
    reportHistory(page);
  });
  contents.on('dom-ready', () => {
    if (contents.isLoading()) loading(0.8);
  });
  contents.on('did-navigate-in-page', (_event, _url, isMainFrame) => {
    if (isMainFrame) reportHistory(page);
  });
  contents.on('did-stop-loading', () => {
    send({ Loading: { page: id, loading: false, progress: 1 } });
    reportHistory(page);
  });
  contents.on('did-fail-load', (_event, code, name, url, isMainFrame) => {
    if (isMainFrame && shownFailure(code)) send({ LoadFailed: { page: id, url, code, name } });
  });
  contents.on('render-process-gone', (_event, details) => {
    if (details.reason !== 'clean-exit') send({ Crashed: { page: id, reason: details.reason } });
  });
  contents.on('unresponsive', () => send({ Responsive: { page: id, responsive: false } }));
  contents.on('responsive', () => send({ Responsive: { page: id, responsive: true } }));
  win.on('close', () => send({ PageClosed: { page: id } }));
}

/**
 * The options of the offscreen window Electron makes for a page that opened one with `window.open`: hidden, painted
 * offscreen at the engine's scale, as every page is, and as confined as every other page. The page that takes it
 * sizes it in device pixels and renders it at its pane's scale.
 * @param {number} scale The device scale the engine paints the window at, `ENGINE_SCALE` of `surface.js`.
 * @returns {import('electron').BrowserWindowConstructorOptions} The options.
 */
function popupOptions(scale) {
  return {
    show: false,
    useContentSize: true,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      offscreen: { deviceScaleFactor: scale },
      preload: path.join(__dirname, 'popups-preload.js'),
      nodeIntegrationInSubFrames: true,
      backgroundThrottling: false,
    },
  };
}

/**
 * A window a page opened, held for the window to take.
 * @typedef {object} Held
 * @property {import('electron').BrowserWindow} win The window.
 * @property {number} scale The display scale of the opener's pane, which it is shown at until the window gives its own.
 * @property {NodeJS.Timeout} timer Closes it when the window does not take it in time.
 */

/**
 * The windows pages opened that the window has not taken yet, by the helper's number for them.
 * @type {Map<number, Held>}
 */
const held = new Map();

/** The number the next window a page opens gets. */
let nextPopup = 1;

/**
 * Holds a window a page opened and tells the window of it, so that it becomes a tab; it is closed when the window
 * does not take it in time.
 * @param {Watched} opener The page that opened it.
 * @param {import('electron').BrowserWindow} win The window Electron made.
 * @param {string} url Where it goes.
 * @param {boolean} agent Whether a tool of an agent was running on the opener.
 * @param {number} scale The display scale of the opener's pane, which it is shown at until the window gives its own.
 * @returns {void}
 */
function hold(opener, win, url, agent, scale) {
  const popup = nextPopup++;
  const timer = setTimeout(() => drop(popup), HOLD_FOR);
  held.set(popup, { win, scale, timer });
  win.once('closed', () => {
    const entry = held.get(popup);
    if (entry && entry.win === win) {
      clearTimeout(entry.timer);
      held.delete(popup);
    }
  });
  opener.send({ PopupOpened: { page: opener.id, popup, url, agent } });
}

/**
 * Hands over a window a page opened, for the window to make a page of; nothing when it is no longer held.
 * @param {number} popup The helper's number for it.
 * @returns {{ win: import('electron').BrowserWindow, scale: number } | null} The window and the display scale of its opener's pane.
 */
function take(popup) {
  const entry = held.get(popup);
  if (!entry) return null;
  clearTimeout(entry.timer);
  held.delete(popup);
  return entry.win.isDestroyed() ? null : { win: entry.win, scale: entry.scale };
}

/**
 * Closes a window a page opened that the window will not show.
 * @param {number} popup The helper's number for it.
 * @returns {void}
 */
function drop(popup) {
  const taken = take(popup);
  if (taken) taken.win.destroy();
}

module.exports = { watch, reportHistory, popupOptions, hold, take, drop, shownFailure };
