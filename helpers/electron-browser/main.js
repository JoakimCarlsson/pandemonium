'use strict';

/**
 * The browser helper of the window: it speaks the line protocol over the control socket, paints
 * pages offscreen into pairs of shared files the window maps, and runs the tools agents call.
 *
 * Started by the window as `main.js --control <socket> --surfaces <folder> --profile <folder>`.
 */

const { app, dialog } = require('electron');
const net = require('net');
const path = require('path');
const { holdNativeBoxes } = require('./src/dialogs');
const origins = require('./src/origins');
const { Page } = require('./src/page');
const safety = require('./src/safety');

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

/** The control socket the window listens on. */
const controlPath = argument('--control') || '';

/** The folder the helper paints its shared files into. */
const surfacesFolder = argument('--surfaces') || '';

/** The folder the engine keeps its cookies and caches in. */
const profileFolder = argument('--profile');

if (profileFolder) {
  app.setPath('userData', path.join(profileFolder, 'electron'));
}
app.disableHardwareAcceleration();

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

/**
 * The fields every request about a page has.
 * @param {Exclude<Request, 'Shutdown'>} request The request.
 * @returns {PageBody} Its body, which names the page.
 */
function bodyOf(request) {
  return /** @type {PageBody} */ (Object.values(request)[0]);
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
  if ('AgentOrigins' in request) {
    origins.setPolicy(request.AgentOrigins.restrict, request.AgentOrigins.origins);
  } else if ('PermissionAnswer' in request) {
    safety.answerPermission(request.PermissionAnswer.id, request.PermissionAnswer.allow);
  } else if ('CertificateChoice' in request) {
    const page = pages.get(request.CertificateChoice.page);
    if (page) safety.chooseCertificate(page, request.CertificateChoice.proceed);
  } else if ('Open' in request) {
    const body = request.Open;
    const page = new Page({ id: body.page, width: body.width, height: body.height, folder: surfacesFolder, send, inspectable: true });
    pages.set(body.page, page);
    safety.watch(page);
    page.load(body.url);
  } else if ('OpenTools' in request) {
    const body = request.OpenTools;
    const inspected = pages.get(body.of);
    if (!inspected || inspected.win.isDestroyed()) return;
    const tools = new Page({ id: body.page, width: body.width, height: body.height, folder: surfacesFolder, send, inspectable: false });
    pages.set(body.page, tools);
    inspected.contents.setDevToolsWebContents(tools.contents);
    inspected.contents.openDevTools({ mode: 'detach' });
  } else if ('Close' in request) {
    const page = pages.get(request.Close.page);
    if (page) {
      safety.forget(request.Close.page);
      page.close();
      pages.delete(request.Close.page);
    }
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
  const connection = net.createConnection(controlPath);
  socket = connection;
  let pending = '';
  connection.on('connect', () => send('Ready'));
  connection.on('data', (chunk) => {
    pending += chunk.toString('utf8');
    let end = pending.indexOf('\n');
    while (end >= 0) {
      const text = pending.slice(0, end);
      pending = pending.slice(end + 1);
      if (text.trim()) {
        try {
          handle(JSON.parse(text));
        } catch (error) {
          console.error('request failed:', error);
        }
      }
      end = pending.indexOf('\n');
    }
  });
  connection.on('close', () => app.quit());
  connection.on('error', () => app.quit());
});

app.on('window-all-closed', () => {
  /* the helper lives until the window hangs up */
});
