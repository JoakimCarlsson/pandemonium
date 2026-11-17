'use strict';

/**
 * Sealed pages: documents an agent wrote, shown with their scripts running and the network denied.
 *
 * Every sealed page lives in one session of its own that is kept in memory only, so it shares no cookies, storage or
 * cache with the pages of the web the reader browses. That session cancels every request that is not for a `data:`,
 * `blob:` or `about:` address, refuses every permission, cancels every download and sends what would still leave
 * through a proxy that does not exist. This file makes that session, the options of a window in it, the guards of
 * such a window, and renders a document to a picture.
 */

const { BrowserWindow, session } = require('electron');
const fs = require('fs');
const path = require('path');
const policy = require('./sealed-policy');

/** The name of the session sealed pages share; without `persist:` in front, the engine keeps it in memory only. */
const PARTITION = 'sealed-documents';

/** How long a render may take before it is given up, in milliseconds. */
const RENDER_TIMEOUT = 10000;

/** How long a document is left to run its scripts and lay itself out after it loaded, in milliseconds. */
const SETTLE = 250;

/** How many documents are rendered at once. */
const RENDERS_AT_ONCE = 2;

/** How many renders may wait for their turn before new ones are refused. */
const RENDERS_WAITING = 64;

/**
 * The session of sealed pages, once it was made.
 * @type {import('electron').Session | null}
 */
let held = null;

/**
 * The session every sealed page lives in, made and locked down the first time it is asked for.
 * @returns {import('electron').Session} The session.
 */
function sealedSession() {
  if (held) return held;
  const made = session.fromPartition(PARTITION, { cache: false });
  made.webRequest.onBeforeRequest((details, callback) => callback({ cancel: !policy.allowed(details.url) }));
  made.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  made.setPermissionCheckHandler(() => false);
  made.setDevicePermissionHandler(() => false);
  made.on('will-download', (event) => event.preventDefault());
  made.setProxy({ proxyRules: 'socks5://127.0.0.1:1', proxyBypassRules: '<-loopback>' }).catch(() => {});
  held = made;
  return made;
}

/**
 * The options of a window that shows a sealed page.
 * @param {string | null} preload A script to run in the page's isolated world, which the page cannot reach, or nothing.
 * @returns {import('electron').WebPreferences} The options.
 */
function preferences(preload) {
  /** @type {import('electron').WebPreferences} */
  const options = {
    session: sealedSession(),
    sandbox: true,
    contextIsolation: true,
    nodeIntegration: false,
    nodeIntegrationInSubFrames: false,
    nodeIntegrationInWorker: false,
    webSecurity: true,
    allowRunningInsecureContent: false,
    webviewTag: false,
    navigateOnDragDrop: false,
    spellcheck: false,
    offscreen: true,
    backgroundThrottling: false,
  };
  if (preload) options.preload = preload;
  return options;
}

/**
 * Keeps a sealed page on its document: it opens no window, its top frame goes nowhere else, and a frame inside it goes
 * only to what the session would let it ask for.
 * @param {import('electron').WebContents} contents What the window shows.
 * @returns {void}
 */
function guard(contents) {
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
  contents.on('will-navigate', (event) => event.preventDefault());
  contents.on('will-redirect', (event) => event.preventDefault());
  contents.on('will-frame-navigate', (event) => {
    if (event.isMainFrame || !policy.allowed(event.url)) event.preventDefault();
  });
  contents.on('will-attach-webview', (event) => event.preventDefault());
  contents.setWebRTCIPHandlingPolicy('disable_non_proxied_udp');
}

/**
 * Waits.
 * @param {number} milliseconds How long.
 * @returns {Promise<void>} Resolves once the time has passed.
 */
function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

/**
 * Loads a document into a sealed offscreen window and gives what it painted once it settled.
 * @param {string} html The document.
 * @param {number} width The viewport's width in CSS pixels.
 * @param {number} height The viewport's height in CSS pixels.
 * @returns {Promise<Buffer>} The picture as a PNG file's contents; rejects when the document does not load or paints nothing.
 */
async function paintOnce(html, width, height) {
  const win = new BrowserWindow({ show: false, width, height, useContentSize: true, webPreferences: preferences(null) });
  const contents = win.webContents;
  /** @type {import('electron').NativeImage | null} */
  let latest = null;
  /** @type {NodeJS.Timeout | null} */
  let timer = null;
  try {
    guard(contents);
    contents.setAudioMuted(true);
    contents.setFrameRate(30);
    contents.on('paint', (_event, _dirty, image) => {
      latest = image;
    });
    const expired = new Promise((_resolve, reject) => {
      timer = setTimeout(() => reject(new Error('The document took too long to render.')), RENDER_TIMEOUT);
    });
    const painted = (async () => {
      await contents.loadURL(policy.address(html));
      await delay(SETTLE);
      if (!latest && !win.isDestroyed()) {
        contents.invalidate();
        await delay(SETTLE);
      }
    })();
    await Promise.race([painted, expired]);
    const image = /** @type {import('electron').NativeImage | null} */ (latest);
    if (!image || image.isEmpty()) throw new Error('The document painted nothing.');
    return image.toPNG();
  } finally {
    if (timer) clearTimeout(timer);
    if (!win.isDestroyed()) win.destroy();
  }
}

/**
 * How many renders are running.
 * @type {number}
 */
let running = 0;

/**
 * The renders waiting for their turn: each is called when one that runs has ended.
 * @type {(() => void)[]}
 */
const waiting = [];

/**
 * The renders under way, by the file they write, so that the same picture asked for twice is rendered once.
 * @type {Map<string, Promise<void>>}
 */
const underWay = new Map();

/**
 * Waits for a turn to render, so that no more than a few documents are rendered at once.
 * @returns {Promise<void>} Resolves when the caller may render; rejects when too many already wait.
 */
function turn() {
  if (running < RENDERS_AT_ONCE) {
    running += 1;
    return Promise.resolve();
  }
  if (waiting.length >= RENDERS_WAITING) return Promise.reject(new Error('Too many documents are waiting to be rendered.'));
  return new Promise((resolve) => waiting.push(resolve));
}

/**
 * Gives the turn of a render that ended to the next one waiting, if any.
 * @returns {void}
 */
function turnOver() {
  const next = waiting.shift();
  if (next) {
    next();
  } else {
    running -= 1;
  }
}

/**
 * Renders a document and saves the picture, written whole or not at all.
 * @param {string} html The document.
 * @param {number} width The viewport's width in CSS pixels.
 * @param {number} height The viewport's height in CSS pixels.
 * @param {string} file Where the PNG goes.
 * @returns {Promise<void>} Resolves once the file is there.
 */
async function renderInTurn(html, width, height, file) {
  await turn();
  try {
    const bytes = await paintOnce(html, width, height);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const partial = `${file}.${process.pid}.part`;
    fs.writeFileSync(partial, bytes);
    fs.renameSync(partial, file);
  } finally {
    turnOver();
  }
}

/**
 * Renders a document to a PNG file at a viewport, sealed from the network. A file that is already there is left as it
 * is, since the window names the file after what is rendered; the same file asked for while it is being rendered waits
 * for that render.
 * @param {unknown} html The document, at most 256 KiB.
 * @param {unknown} width The viewport's width in CSS pixels.
 * @param {unknown} height The viewport's height in CSS pixels.
 * @param {unknown} file Where the PNG goes, an absolute path.
 * @returns {Promise<void>} Resolves once the file is there; rejects with why it could not be rendered.
 */
function render(html, width, height, file) {
  const refused = policy.refusal(html);
  if (refused) return Promise.reject(new Error(refused));
  if (!policy.usableSide(width) || !policy.usableSide(height)) {
    return Promise.reject(new Error('The viewport is not a usable size.'));
  }
  if (typeof file !== 'string' || !path.isAbsolute(file) || !file.endsWith('.png')) {
    return Promise.reject(new Error('The picture has nowhere to go.'));
  }
  if (fs.existsSync(file)) return Promise.resolve();
  const already = underWay.get(file);
  if (already) return already;
  const started = renderInTurn(/** @type {string} */ (html), /** @type {number} */ (width), /** @type {number} */ (height), file).finally(() =>
    underWay.delete(file),
  );
  underWay.set(file, started);
  return started;
}

module.exports = { sealedSession, preferences, guard, render };
