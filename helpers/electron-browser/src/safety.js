'use strict';

/**
 * What keeps the reader safe from the pages: permissions pages ask for, certificates that are not
 * trusted, and files pages start downloading. Everything is refused unless the reader says
 * otherwise, and what the reader says is remembered for the origin until the helper ends.
 */

const { app, session } = require('electron');
const fs = require('fs');
const path = require('path');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Send} Send */

/** How long a question waits for the reader before the answer is no, in milliseconds. */
const WAIT = 120000;

/** The one permission pages are given without asking: putting text on the clipboard in answer to a click. */
const UNASKED = 'clipboard-sanitized-write';

/**
 * What the reader is asked about, by the engine's name for it and the words the question uses.
 * Camera and microphone come as `media` and are named from what the page asked for.
 * @type {Record<string, string>}
 */
const ASKABLE = {
  geolocation: 'location',
  notifications: 'notifications',
  'clipboard-read': 'clipboard',
  midi: 'MIDI devices',
  midiSysex: 'MIDI devices',
  pointerLock: 'mouse pointer lock',
  'idle-detection': 'idle detection',
  'window-management': 'window management',
};

/**
 * What each question is about and how it ends, by the number the window knows it by.
 * @typedef {object} Question
 * @property {number} page The page that asked.
 * @property {string[]} keys What the reader's answer is remembered under, such as `https://example.com|camera`.
 * @property {string} words What the question says it is about.
 * @property {((allow: boolean) => void)[]} callbacks The engine's callbacks waiting for the answer.
 * @property {NodeJS.Timeout} timer Ends the question with a no when it is not answered.
 */

/**
 * The questions the reader has not answered.
 * @type {Map<number, Question>}
 */
const open = new Map();

/**
 * What the reader answered, by origin and thing, for as long as the helper runs.
 * @type {Map<string, boolean>}
 */
const remembered = new Map();

/**
 * The certificates the reader chose to load anyway, by origin and fingerprint.
 * @type {Set<string>}
 */
const trusted = new Set();

/** The number the next question gets. */
let nextQuestion = 1;

/**
 * Where the helper is told what to do and what to report.
 * @type {{ send: Send, pageOf: (contents: import('electron').WebContents | null) => Page | undefined }}
 */
let context = { send: () => {}, pageOf: () => undefined };

/**
 * The origin of an address.
 * @param {string} url The address.
 * @returns {string | null} Its origin, or nothing when it is not a web address.
 */
function originOf(url) {
  try {
    const address = new URL(url);
    return address.protocol === 'http:' || address.protocol === 'https:' ? address.origin : null;
  } catch (_) {
    return null;
  }
}

/**
 * What a permission is, as things the reader decides one by one.
 * @param {string} permission The engine's name for it.
 * @param {Record<string, any>} details What the engine says about the request.
 * @returns {string[]} The things, such as `camera` and `microphone`; none when the permission is not one that is asked.
 */
function thingsOf(permission, details) {
  if (permission === 'media') {
    const types = Array.isArray(details.mediaTypes) ? details.mediaTypes : [];
    const things = [];
    if (types.includes('video')) things.push('camera');
    if (types.includes('audio')) things.push('microphone');
    return things;
  }
  const word = Object.prototype.hasOwnProperty.call(ASKABLE, permission) ? ASKABLE[permission] : '';
  return word ? [word] : [];
}

/**
 * Ends a question.
 * @param {number} id The question.
 * @param {boolean} allow The answer.
 * @param {boolean} answered Whether the reader gave the answer, which is then remembered; when not, the page went away or the time ran out.
 * @returns {void}
 */
function settle(id, allow, answered) {
  const question = open.get(id);
  if (!question) return;
  open.delete(id);
  clearTimeout(question.timer);
  if (answered) {
    for (const key of question.keys) remembered.set(key, allow);
  } else {
    context.send({ PermissionCancelled: { page: question.page, id } });
  }
  for (const callback of question.callbacks) callback(allow && answered);
}

/**
 * Settles the questions of a page with a no, as when it went to another address or closed.
 * @param {number} pageId The page.
 * @returns {void}
 */
function cancelQuestions(pageId) {
  for (const [id, question] of [...open]) {
    if (question.page === pageId) settle(id, false, false);
  }
}

/**
 * Answers a permission request of a page: no when the reader refused it before, yes when they allowed it before and no agent is acting on the page, and otherwise by asking them.
 * @param {Page} page The page.
 * @param {string} origin The origin that asks.
 * @param {string[]} things What it asks for.
 * @param {(allow: boolean) => void} callback Gets the answer.
 * @returns {void}
 */
function decide(page, origin, things, callback) {
  const keys = things.map((thing) => `${origin}|${thing}`);
  if (keys.some((key) => remembered.get(key) === false)) return callback(false);
  if (keys.every((key) => remembered.get(key) === true) && !page.agentBusy()) return callback(true);
  const words = things.join(' and ');
  for (const question of open.values()) {
    if (question.page === page.id && question.words === words && question.keys.join() === keys.join()) {
      question.callbacks.push(callback);
      return;
    }
  }
  const id = nextQuestion;
  nextQuestion += 1;
  const timer = setTimeout(() => settle(id, false, false), WAIT);
  open.set(id, { page: page.id, keys, words, callbacks: [callback], timer });
  context.send({ PermissionAsked: { page: page.id, id, origin, permission: words } });
}

/**
 * Takes the reader's answer to a question.
 * @param {number} id The question.
 * @param {boolean} allow Whether the reader allowed it.
 * @returns {void}
 */
function answerPermission(id, allow) {
  settle(id, allow === true, true);
}

/**
 * Has pages refuse what they ask the browser for, such as the camera, the microphone, notifications and the screen, unless the reader allows it when asked.
 * @returns {void}
 */
function guardPermissions() {
  const pages = session.defaultSession;
  pages.setPermissionRequestHandler((contents, permission, callback, details) => {
    if (permission === UNASKED) return callback(true);
    const page = context.pageOf(contents);
    const info = /** @type {Record<string, any>} */ (details);
    const things = thingsOf(permission, info);
    const origin = originOf(String(info.requestingUrl || ''));
    if (!page || !page.cdp || !origin || info.isMainFrame === false || things.length === 0) return callback(false);
    decide(page, origin, things, callback);
  });
  pages.setPermissionCheckHandler((contents, permission, requestingOrigin, details) => {
    if (permission === UNASKED) return true;
    const page = context.pageOf(contents);
    const info = /** @type {Record<string, any>} */ (details);
    const kind = info.mediaType === 'video' ? ['camera'] : info.mediaType === 'audio' ? ['microphone'] : [];
    const things = permission === 'media' ? kind : thingsOf(permission, info);
    const origin = originOf(requestingOrigin);
    if (!page || !origin || things.length === 0 || page.agentBusy()) return false;
    return things.every((thing) => remembered.get(`${origin}|${thing}`) === true);
  });
  pages.setDevicePermissionHandler(() => false);
  pages.setDisplayMediaRequestHandler((_request, callback) => callback({}));
}

/**
 * Has a site whose certificate is not trusted fail to load, and tells the window so the reader can choose; a site the reader chose to load anyway loads, only for that origin and certificate.
 * @returns {void}
 */
function guardCertificates() {
  app.on('certificate-error', (event, contents, url, error, certificate, callback, isMainFrame) => {
    event.preventDefault();
    const page = context.pageOf(contents);
    const origin = originOf(url);
    if (!page || !origin) return callback(false);
    if (trusted.has(`${origin}|${certificate.fingerprint}`)) return callback(true);
    callback(false);
    if (!isMainFrame) return;
    page.certificate = { url, origin, fingerprint: certificate.fingerprint };
    const expires = Number.isFinite(certificate.validExpiry) ? new Date(certificate.validExpiry * 1000).toISOString().slice(0, 10) : '';
    context.send({
      CertificateError: {
        page: page.id,
        info: {
          url,
          error,
          subject: certificate.subject.commonName || certificate.subjectName,
          issuer: certificate.issuer.commonName || certificate.issuerName,
          fingerprint: certificate.fingerprint,
          expires,
        },
      },
    });
  });
}

/**
 * Takes the reader's choice on the certificate error of a page: to load it anyway, which holds for that origin and certificate until the helper ends, or to go back.
 * @param {Page} page The page.
 * @param {boolean} proceed Whether to load it anyway.
 * @returns {void}
 */
function chooseCertificate(page, proceed) {
  const pending = page.certificate;
  if (!pending || page.win.isDestroyed()) return;
  page.certificate = null;
  context.send({ CertificateCleared: { page: page.id } });
  if (proceed === true) {
    trusted.add(`${pending.origin}|${pending.fingerprint}`);
    page.contents.loadURL(pending.url).catch(() => {});
  } else if (page.contents.navigationHistory.canGoBack()) {
    page.contents.navigationHistory.goBack();
  } else {
    page.contents.loadURL('about:blank').catch(() => {});
  }
}

/**
 * A file name that is safe to create in the reader's folder: no folders, no characters a file system treats specially, no leading dot, not empty.
 * @param {string} raw The name the page or the server proposed.
 * @returns {string} The name to use.
 */
function safeName(raw) {
  const base = String(raw).split(/[\\/]/).pop() || '';
  let name = base
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u001f\u007f<>:"|?*]/g, '_')
    .replace(/^[\s.]+/, '')
    .replace(/[\s.]+$/, '');
  if (/^(con|prn|aux|nul|com[0-9]|lpt[0-9])(\..*)?$/i.test(name)) name = `_${name}`;
  if (name.length > 120) {
    const extension = path.extname(name).slice(0, 20);
    name = name.slice(0, 120 - extension.length) + extension;
  }
  return name || 'download';
}

/**
 * Creates an empty file in `folder` with a name that nothing else has, so a download never overwrites a file: `name`, or `name (1)`, `name (2)` and so on before the extension.
 * @param {string} folder The reader's Downloads folder.
 * @param {string} name A safe file name.
 * @returns {string} The path of the file made.
 */
function reserve(folder, name) {
  fs.mkdirSync(folder, { recursive: true });
  const extension = path.extname(name);
  const stem = name.slice(0, name.length - extension.length);
  for (let n = 0; n < 1000; n += 1) {
    const candidate = path.join(folder, n === 0 ? name : `${stem} (${n})${extension}`);
    try {
      fs.closeSync(fs.openSync(candidate, 'wx', 0o600));
      return candidate;
    } catch (error) {
      if (!(error instanceof Error) || /** @type {NodeJS.ErrnoException} */ (error).code !== 'EEXIST') throw error;
    }
  }
  throw new Error('no free file name');
}

/**
 * Saves what pages download into the reader's Downloads folder without a dialog, under a name the helper makes safe and never over an existing file, tells the window, and opens nothing.
 * @returns {void}
 */
function guardDownloads() {
  session.defaultSession.on('will-download', (_event, item, contents) => {
    const page = context.pageOf(contents);
    if (!page || !page.cdp) {
      item.cancel();
      return;
    }
    let target = '';
    try {
      target = reserve(app.getPath('downloads'), safeName(item.getFilename()));
    } catch (_) {
      item.cancel();
      context.send({ Download: { page: page.id, name: safeName(item.getFilename()), state: 'Failed' } });
      return;
    }
    const name = path.basename(target);
    item.setSavePath(target);
    context.send({ Download: { page: page.id, name, state: 'Started' } });
    item.once('done', (_done, state) => {
      if (state !== 'completed') fs.rmSync(target, { force: true });
      context.send({ Download: { page: page.id, name, state: state === 'completed' ? 'Finished' : 'Failed' } });
    });
  });
}

/**
 * Watches a page: a question it asked, or a certificate error it had, no longer applies once it goes to another address.
 * @param {Page} page The page.
 * @returns {void}
 */
function watch(page) {
  page.contents.on('did-start-navigation', (details) => {
    if (!details.isMainFrame || details.isSameDocument || details.url.startsWith('chrome-error:')) return;
    cancelQuestions(page.id);
    if (page.certificate) {
      page.certificate = null;
      context.send({ CertificateCleared: { page: page.id } });
    }
  });
}

/**
 * Forgets what a page that closed was waiting for.
 * @param {number} pageId The page.
 * @returns {void}
 */
function forget(pageId) {
  cancelQuestions(pageId);
}

/**
 * Puts the helper's guards in place: permissions, certificates and downloads.
 * @param {typeof context} where Where to report, and how to find the page of a window's contents.
 * @returns {void}
 */
function install(where) {
  context = where;
  guardPermissions();
  guardCertificates();
  guardDownloads();
}

module.exports = { answerPermission, chooseCertificate, forget, install, reserve, safeName, watch };
