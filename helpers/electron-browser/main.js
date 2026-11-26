'use strict';

/**
 * The browser helper of the window: it speaks the line protocol over a loopback connection to the window, paints
 * pages offscreen into pairs of shared files the window maps, and runs the tools agents call.
 *
 * Started by the window as `main.js --control <host:port> --surfaces <folder> --profile <folder>`.
 */

const { app, dialog } = require('electron');
const net = require('net');
const path = require('path');
const blocker = require('./src/blocker');
const chrome = require('./src/chrome');
const { holdNativeBoxes } = require('./src/dialogs');
const origins = require('./src/origins');
const passwords = require('./src/passwords');
const { Page } = require('./src/page');
const safety = require('./src/safety');
const sealed = require('./src/sealed');
const features = require('./src/features');
const lifecycle = require('./src/lifecycle');
const partitions = require('./src/partitions');
const recording = require('./src/recording');
const runner = require('./src/runner');
const { LineReader } = require('./src/lines');

/** @typedef {import('./src/types').Event} Event */
/** @typedef {import('./src/types').PageBody} PageBody */
/** @typedef {import('./src/types').Request} Request */

/**
 * The value after a flag on the command line.
 * @param {string} name The flag, such as `--control`.
 * @returns {string | undefined} Its value, or nothing when the flag was not given.
 */
function argument(name) {
  const at = process.argv.indexOf(name);
  return at >= 0 ? process.argv[at + 1] : undefined;
}

/** The loopback address, `host:port`, the window listens on. */
const controlPath = argument('--control') || '';

/** The secret that shows the window this is the helper it started, sent as the first line. */
const controlToken = process.env.PANDEMONIUM_BROWSER_TOKEN || '';

/** The folder the helper paints its shared files into. */
const surfacesFolder = argument('--surfaces') || '';

/** The folder the engine keeps its cookies and caches in. */
const profileFolder = argument('--profile');

if (profileFolder) {
  app.setPath('userData', path.join(profileFolder, 'electron'));
}
app.commandLine.appendSwitch('enable-unsafe-swiftshader');
app.commandLine.appendSwitch('disable-features', 'WaylandFractionalScaleV1');

/**
 * The socket to the window, once connected.
 * @type {net.Socket | null}
 */
let socket = null;

/**
 * Sends a report to the window as one line of JSON.
 * @param {Event} event The report.
 * @returns {void}
 */
function send(event) {
  if (socket && !socket.destroyed) {
    socket.write(`${JSON.stringify(event)}\n`);
  }
}

/**
 * The open pages, by the number the window chose.
 * @type {Map<number, Page>}
 */
const pages = new Map();

/** The number the next page no tab shows gets: below zero, so it never meets a number the window chose. */
let nextHeadless = -1;

runner.configure({
  makePage(partition, width, height) {
    const page = new Page({ id: nextHeadless, width, height, scale: 1, folder: surfacesFolder, send: () => {}, inspectable: true, partition });
    nextHeadless -= 1;
    watchWeb(page);
    return page;
  },
  recipes: () => path.join(profileFolder || app.getPath('userData'), 'recipes'),
});

/**
 * The fields every request about a page has.
 * @param {Exclude<Request, 'Shutdown'>} request The request.
 * @returns {PageBody} Its body, which names the page.
 */
function bodyOf(request) {
  return /** @type {PageBody} */ (Object.values(request)[0]);
}

/**
 * Renders a document sealed from the network to a picture and tells the window how it went, as `HtmlRendered` with the ticket of the request.
 * @param {import('./src/types').RenderHtmlBody} body The document, the viewport and where the picture goes.
 * @returns {void}
 */
function renderHtml(body) {
  sealed.render(body.html, body.width, body.height, body.path, body.scale).then(
    () => send({ HtmlRendered: { ticket: body.ticket, path: body.path, error: '' } }),
    (error) =>
      send({ HtmlRendered: { ticket: body.ticket, path: '', error: String(error instanceof Error ? error.message : error) } }),
  );
}

/**
 * Makes a window a page opened with `window.open` a page of its own, which keeps its `window.opener`, and tells the
 * window what it shows so far, since it may have loaded before it was taken.
 * @param {import('./src/types').AdoptPopupBody} body The new page's number, the helper's number for the window, and its size.
 * @returns {void}
 */
function adoptPopup(body) {
  const taken = lifecycle.take(body.popup);
  if (!taken) {
    send({ PageClosed: { page: body.page } });
    return;
  }
  const page = new Page({
    id: body.page,
    width: body.width,
    height: body.height,
    scale: taken.scale,
    folder: surfacesFolder,
    send,
    inspectable: true,
    window: taken.win,
    partition: partitions.partitionOf(taken.win.webContents.session),
  });
  pages.set(body.page, page);
  watchWeb(page);
  page.handle({ Resize: { page: body.page, width: body.width, height: body.height, scale: body.scale } });
  const url = page.contents.getURL();
  if (url) send({ Address: { page: body.page, url } });
  send({ Title: { page: body.page, title: page.contents.getTitle() } });
  send({ Loading: { page: body.page, loading: page.contents.isLoading(), progress: 0.5 } });
  lifecycle.reportHistory(page);
  page.contents.invalidate();
}

/**
 * Starts following a page of the web for what the reader is asked and told about it: permissions, certificates and
 * downloads, the ads and trackers it blocks, and the logins it sends.
 * @param {Page} page The page.
 * @returns {void}
 */
function watchWeb(page) {
  page.rewatch = watchWeb;
  safety.watch(page);
  blocker.watch(page);
  passwords.watch(page);
}

/**
 * Carries out one request of the window.
 * @param {Request} request The request.
 * @returns {void}
 */
function handle(request) {
  if (request === 'Shutdown') {
    app.quit();
    return;
  }
  if ('Privacy' in request) {
    blocker.setEnabled(request.Privacy.block_trackers === true, pages.values());
    passwords.setEnabled(request.Privacy.save_passwords === true, pages.values());
  } else if ('AgentOrigins' in request) {
    origins.setPolicy(request.AgentOrigins.restrict, request.AgentOrigins.origins);
  } else if ('PermissionAnswer' in request) {
    safety.answerPermission(request.PermissionAnswer.id, request.PermissionAnswer.allow);
  } else if ('CertificateChoice' in request) {
    const page = pages.get(request.CertificateChoice.page);
    if (page) safety.chooseCertificate(page, request.CertificateChoice.proceed);
  } else if ('Open' in request) {
    const body = request.Open;
    const page = new Page({
      id: body.page,
      width: body.width,
      height: body.height,
      scale: body.scale,
      folder: surfacesFolder,
      send,
      inspectable: true,
      partition: body.partition || '',
    });
    pages.set(body.page, page);
    watchWeb(page);
    page.load(body.url);
    runner.opened(page);
  } else if ('OpenHtml' in request) {
    const body = request.OpenHtml;
    const page = new Page({
      id: body.page,
      width: body.width,
      height: body.height,
      scale: body.scale,
      folder: surfacesFolder,
      send,
      inspectable: false,
      sealed: { html: body.html, title: body.title },
    });
    pages.set(body.page, page);
    page.showSealed();
  } else if ('RenderHtml' in request) {
    renderHtml(request.RenderHtml);
  } else if ('OpenTools' in request) {
    const body = request.OpenTools;
    const inspected = pages.get(body.of);
    if (!inspected || inspected.win.isDestroyed()) return;
    const tools = new Page({ id: body.page, width: body.width, height: body.height, scale: body.scale, folder: surfacesFolder, send, inspectable: false });
    pages.set(body.page, tools);
    inspected.contents.setDevToolsWebContents(tools.contents);
    inspected.contents.openDevTools({ mode: 'detach' });
  } else if ('AdoptPopup' in request) {
    adoptPopup(request.AdoptPopup);
  } else if ('DropPopup' in request) {
    lifecycle.drop(request.DropPopup.popup);
  } else if ('Close' in request) {
    const page = pages.get(request.Close.page);
    if (page) {
      safety.forget(request.Close.page);
      page.close();
      pages.delete(request.Close.page);
      partitions.release(page.partition, pages.values());
    }
  } else if ('RecordStep' in request && !pages.has(request.RecordStep.page)) {
    const body = request.RecordStep;
    const caller = { session: body.session || '', name: body.session_name || '' };
    recording.afterTool(null, body.tool, body.arguments, { ok: body.ok === true, text: String(body.text || ''), image: null }, caller);
  } else {
    const page = pages.get(bodyOf(request).page);
    if (page) page.handle(request);
  }
}

/**
 * The page that shows a window's contents.
 * @param {import('electron').WebContents | null} contents What a window shows.
 * @returns {Page | undefined} Its page, or nothing when it is not one of the open pages.
 */
function pageOf(contents) {
  if (!contents) return undefined;
  for (const page of pages.values()) {
    if (!page.win.isDestroyed() && page.contents === contents) return page;
  }
  return undefined;
}

app.whenReady().then(() => {
  holdNativeBoxes(dialog);
  safety.install({ send, pageOf });
  chrome.install(pageOf);
  const profile = profileFolder || app.getPath('userData');
  blocker.install({ profile, pageOf });
  passwords.install({ profile, pageOf });
  if (process.platform === 'darwin') app.dock?.hide();
  const separator = controlPath.lastIndexOf(':');
  const connection = net.createConnection({
    host: controlPath.slice(0, separator),
    port: Number(controlPath.slice(separator + 1)),
  });
  socket = connection;
  const lines = new LineReader();
  connection.on('connect', () => {
    connection.write(`${controlToken}\n`);
    send(features.announcement());
    send('Ready');
  });
  connection.on('data', (chunk) => {
    for (const text of lines.push(typeof chunk === 'string' ? Buffer.from(chunk, 'utf8') : chunk)) {
      if (!text.trim()) continue;
      try {
        handle(JSON.parse(text));
      } catch (error) {
        console.error('request failed:', error);
      }
    }
  });
  connection.on('close', () => app.quit());
  connection.on('error', () => app.quit());
});

app.on('window-all-closed', () => {
  /* the helper lives until the window hangs up */
});
