'use strict';

/**
 * What keeps the reader safe from the pages: permissions pages ask for, certificates that are not
 * trusted, and files pages start downloading. Everything is refused unless the reader says
 * otherwise, and what the reader says is remembered for the origin in that profile until the helper ends, or until
 * the profile closes when it is kept in memory only, such as the private one.
 */

const { app, desktopCapturer, shell, webContents } = require('electron');
const fs = require('fs');
const path = require('path');
const partitions = require('./partitions');

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
 * @property {string[]} keys What the reader's answer is remembered under, such as `persist:project-00ff\nhttps://example.com|camera`: the profile, the origin and the thing.
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
 * What the reader answered, by profile, origin and thing, as {@link answerKey} writes them, for as long as the helper
 * runs or, for a profile kept in memory only, until its last page closes.
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
 * What the reader's answer about one thing an origin asked for is remembered under: the same answer in another profile
 * is another question, so that what the reader allowed in one profile is not allowed in another.
 * @param {string | undefined} partition The profile of the page that asked; nothing or empty for the shared one.
 * @param {string} origin The origin that asked.
 * @param {string} thing What it asked for, such as `camera`.
 * @returns {string} The key.
 */
function answerKey(partition, origin, thing) {
  return `${partition || ''}\n${origin}|${thing}`;
}

/**
 * Forgets every answer the reader gave in the profile `partition`, as when the last page of a profile kept in memory
 * closed.
 * @param {string} partition The profile.
 * @returns {void}
 */
function forgetProfile(partition) {
  const prefix = `${partition}\n`;
  for (const key of [...remembered.keys()]) {
    if (key.startsWith(prefix)) remembered.delete(key);
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
  const keys = things.map((thing) => answerKey(page.partition, origin, thing));
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
 * Has the pages of a profile refuse what they ask the browser for, such as the camera, the microphone, notifications and the screen, unless the reader allows it when asked.
 * @param {import('electron').Session} pages The profile.
 * @returns {void}
 */
function guardPermissions(pages) {
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
    return things.every((thing) => remembered.get(answerKey(page.partition, origin, thing)) === true);
  });
  pages.setDevicePermissionHandler(() => false);
  pages.setDisplayMediaRequestHandler((request, callback) => askScreen(request, callback));
}

/**
 * A question about sharing a screen or a window, by the number the window knows it by.
 * @typedef {object} ScreenQuestion
 * @property {number} page The page that asked.
 * @property {Map<string, import('electron').DesktopCapturerSource>} sources What it may share, by the engine's name.
 * @property {(streams: import('electron').Streams) => void} callback The engine's callback waiting for the answer.
 * @property {NodeJS.Timeout} timer Ends the question with a no when it is not answered.
 */

/**
 * The questions about sharing the screen the reader has not answered.
 * @type {Map<number, ScreenQuestion>}
 */
const screens = new Map();

/**
 * Asks the reader which screen or window a page may share, by a `ScreenShare` report the window answers with `ChooseScreen`; a page an agent acts on, a frame inside a page, and a page of no web origin are refused at once, as is the request when the system lists nothing to share.
 * @param {import('electron').DisplayMediaRequestHandlerHandlerRequest} request What the engine says of the request.
 * @param {(streams: import('electron').Streams) => void} callback Gets the answer; an empty object refuses.
 * @returns {void}
 */
function askScreen(request, callback) {
  const frame = request.frame;
  const contents = frame ? webContents.fromFrame(frame) : null;
  const page = context.pageOf(contents || null);
  const origin = originOf(String(request.securityOrigin || (frame ? frame.url : '') || ''));
  if (!page || !page.cdp || !origin || page.agentBusy() || (frame && frame.parent)) {
    callback({});
    return;
  }
  desktopCapturer.getSources({ types: ['screen', 'window'], thumbnailSize: { width: 0, height: 0 } }).then(
    (found) => {
      if (found.length === 0) return callback({});
      const id = nextQuestion;
      nextQuestion += 1;
      const timer = setTimeout(() => settleScreen(id, null, false), WAIT);
      screens.set(id, { page: page.id, sources: new Map(found.map((source) => [source.id, source])), callback, timer });
      context.send({
        Chrome: {
          page: page.id,
          report: { ScreenShare: { id, origin, sources: found.map((source) => ({ id: source.id, name: source.name })) } },
        },
      });
    },
    () => callback({}),
  );
}

/**
 * Ends a question about sharing the screen.
 * @param {number} id The question.
 * @param {string | null} source The engine's name of what the reader chose, or nothing to refuse.
 * @param {boolean} answered Whether the reader answered; when not, the window is told the question is gone.
 * @returns {void}
 */
function settleScreen(id, source, answered) {
  const question = screens.get(id);
  if (!question) return;
  screens.delete(id);
  clearTimeout(question.timer);
  if (!answered) context.send({ Chrome: { page: question.page, report: { ScreenShareCancelled: { id } } } });
  const chosen = source ? question.sources.get(source) : undefined;
  question.callback(chosen ? { video: chosen } : {});
}

/**
 * Takes the reader's answer about sharing the screen.
 * @param {number} id The question.
 * @param {string | null} source The engine's name of the screen or window chosen, or nothing to refuse.
 * @returns {void}
 */
function chooseScreen(id, source) {
  settleScreen(id, typeof source === 'string' ? source : null, true);
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
 * A download the helper saves, by the number the window knows it by.
 * @typedef {object} Saving
 * @property {number} page The page that started it.
 * @property {Transfer} item The engine's download, or the saving of a whole page that stands in for one.
 * @property {string} target Where it is saved.
 * @property {string} name The name it is saved under.
 */

/**
 * What the list of downloads reads of a download: the engine's own, or a saved page that behaves like one.
 * @typedef {object} Transfer
 * @property {() => number} getReceivedBytes How many bytes have been written.
 * @property {() => number} getTotalBytes How many bytes there are in all, or 0 when not known.
 * @property {() => string} getState `progressing`, `completed`, `cancelled` or `interrupted`.
 * @property {() => void} cancel Stops it.
 */

/**
 * The downloads of this run of the helper, by number.
 * @type {Map<number, Saving>}
 */
const downloads = new Map();

/** The number the next download gets. */
let nextDownload = 1;

/** How often a running download tells the window how far it got, at most, in milliseconds. */
const PROGRESS_EVERY = 250;

/**
 * Tells the window how a download stands.
 * @param {number} id The download.
 * @param {Saving} saving The download.
 * @param {'Running' | 'Finished' | 'Failed' | 'Cancelled'} progress How it stands.
 * @returns {void}
 */
function reportDownload(id, saving, progress) {
  const { item } = saving;
  context.send({
    Chrome: {
      page: saving.page,
      report: {
        Download: { id, name: saving.name, received: item.getReceivedBytes(), total: item.getTotalBytes(), progress },
      },
    },
  });
}

/**
 * Saves what the pages of a profile and their developer tools download into the reader's Downloads folder without a dialog, under a name the helper makes safe and never over an existing file, tells the window how far it got as it goes, and opens nothing. Sealed pages download nothing.
 * @param {import('electron').Session} pages The profile.
 * @returns {void}
 */
function guardDownloads(pages) {
  pages.on('will-download', (_event, item, contents) => {
    const page = context.pageOf(contents);
    if (!page || page.sealed) {
      item.cancel();
      return;
    }
    const id = nextDownload;
    nextDownload += 1;
    let target = '';
    try {
      target = reserve(app.getPath('downloads'), safeName(item.getFilename()));
    } catch (_) {
      item.cancel();
      context.send({
        Chrome: {
          page: page.id,
          report: { Download: { id, name: safeName(item.getFilename()), received: 0, total: 0, progress: 'Failed' } },
        },
      });
      return;
    }
    item.setSavePath(target);
    /** @type {Saving} */
    const saving = { page: page.id, item, target, name: path.basename(target) };
    downloads.set(id, saving);
    reportDownload(id, saving, 'Running');
    let told = Date.now();
    item.on('updated', () => {
      const now = Date.now();
      if (now - told < PROGRESS_EVERY) return;
      told = now;
      reportDownload(id, saving, 'Running');
    });
    item.once('done', (_done, state) => {
      if (state !== 'completed') fs.rmSync(target, { force: true });
      reportDownload(id, saving, state === 'completed' ? 'Finished' : state === 'cancelled' ? 'Cancelled' : 'Failed');
    });
  });
}

/**
 * Saves a page as a complete web page into the reader's Downloads folder, under a name nothing else has: its document,
 * named from its title, with the pictures, styles and scripts it uses in a folder beside it. The window is told as it
 * is of a download, so the save shows in the list of downloads and can be shown in its folder or opened. Sealed pages
 * are not saved.
 * @param {Page} page The page.
 * @returns {Promise<void>} Resolves once the window has been told how the save ended.
 */
async function savePage(page) {
  if (page.sealed || page.contents.isDestroyed()) return;
  const id = nextDownload;
  nextDownload += 1;
  const name = safeName(`${page.contents.getTitle() || 'page'}.html`);
  /** @type {string} */
  let state = 'progressing';
  /** @type {Transfer} */
  const item = {
    getReceivedBytes: () => (state === 'completed' && fs.existsSync(saving.target) ? fs.statSync(saving.target).size : 0),
    getTotalBytes: () => item.getReceivedBytes(),
    getState: () => state,
    cancel: () => {},
  };
  /** @type {Saving} */
  const saving = { page: page.id, item, target: '', name };
  try {
    saving.target = reserve(app.getPath('downloads'), name);
    saving.name = path.basename(saving.target);
    downloads.set(id, saving);
    reportDownload(id, saving, 'Running');
    await page.contents.savePage(saving.target, 'HTMLComplete');
    state = 'completed';
    reportDownload(id, saving, 'Finished');
  } catch {
    state = 'interrupted';
    if (saving.target) fs.rmSync(saving.target, { force: true });
    reportDownload(id, saving, 'Failed');
  }
}

/**
 * Stops a download that is still running; its file is removed.
 * @param {number} id The download.
 * @returns {void}
 */
function cancelDownload(id) {
  const saving = downloads.get(id);
  if (saving && saving.item.getState() === 'progressing') saving.item.cancel();
}

/**
 * Shows the file of a finished download in the system's file manager, or opens it with the program the system chooses.
 * @param {number} id The download.
 * @param {boolean} open Whether to open it rather than show it.
 * @returns {void}
 */
function showDownload(id, open) {
  const saving = downloads.get(id);
  if (!saving || saving.item.getState() !== 'completed' || !fs.existsSync(saving.target)) return;
  if (open) {
    shell.openPath(saving.target).catch(() => {});
  } else {
    shell.showItemInFolder(saving.target);
  }
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
  for (const [id, question] of [...screens]) {
    if (question.page === pageId) settleScreen(id, null, false);
  }
}

/**
 * Puts the helper's guards in place: permissions and downloads in every profile pages live in, now and as each is made, and certificates.
 * @param {typeof context} where Where to report, and how to find the page of a window's contents.
 * @returns {void}
 */
function install(where) {
  context = where;
  partitions.onSession((pages) => {
    guardPermissions(pages);
    guardDownloads(pages);
  });
  partitions.onRelease(forgetProfile);
  guardCertificates();
}

module.exports = {
  answerKey,
  answerPermission,
  cancelDownload,
  chooseCertificate,
  chooseScreen,
  forget,
  forgetProfile,
  install,
  remembered,
  reserve,
  safeName,
  savePage,
  showDownload,
  watch,
};
