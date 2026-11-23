'use strict';

/**
 * The blocker of ads and trackers: it builds an engine from the filter lists of `src/blocker-policy.js`, keeps it
 * under the helper's profile and fetches the lists again once a day, and stops the requests of pages the lists name,
 * hides the elements they name, and tells the window how many requests of each page it stopped.
 *
 * It works in every profile pages of the web live in (`src/partitions.js`); sealed pages live in a session of their
 * own, which never sees it. A page on this machine, such as a dev server, and a site the reader let through are left alone. A request it
 * stops fails in the page as `net::ERR_BLOCKED_BY_CLIENT`, which the developer tools and the network tool of agents
 * show like any failed request.
 */

const { ipcMain, net, webContents } = require('electron');
const fs = require('fs');
const path = require('path');
const { ElectronBlocker } = require('@ghostery/adblocker-electron');
const partitions = require('./partitions');
const policy = require('./blocker-policy');

/** @typedef {import('./page').Page} Page */

/** The preload that asks for the hiding rules of each frame, run in the isolated world of every page of the web. */
const COSMETIC_PRELOAD = require.resolve('@ghostery/adblocker-electron-preload');

/** How long the count of a page waits before it is reported, so a burst of stopped requests is one report, in milliseconds. */
const REPORT_DELAY = 250;

/** Whether the reader has blocking on in the settings; off until the window says. */
let enabled = false;

/**
 * The engine built from the lists, once it is.
 * @type {ElectronBlocker | null}
 */
let engine = null;

/** The folder the engine and the sites let through are kept in, once installed. */
let folder = '';

/**
 * The sites the reader let through.
 * @type {Set<string>}
 */
let allowed = new Set();

/**
 * Finds the page of a window's contents.
 * @type {(contents: import('electron').WebContents | null) => Page | undefined}
 */
let pageOf = () => undefined;

/** Whether the lists are being fetched now. */
let fetching = false;

/** When fetching last failed, in milliseconds since the epoch, or 0. */
let failedAt = 0;

/**
 * The timer that looks whether the lists got too old.
 * @type {ReturnType<typeof setInterval> | null}
 */
let checker = null;

/**
 * How many requests each page had stopped since it last loaded, and the timer of its next report.
 * @type {WeakMap<Page, { blocked: number, timer: ReturnType<typeof setTimeout> | null }>}
 */
const COUNTS = new WeakMap();

/**
 * The file a kept thing is in.
 * @param {string} name Its name in the folder.
 * @returns {string} Its path.
 */
function file(name) {
  return path.join(folder, name);
}

/**
 * Writes a file whole or not at all, through a file beside it that is renamed over it.
 * @param {string} target Where it goes.
 * @param {string | Uint8Array} data What it holds.
 * @returns {void}
 * @throws {Error} When the folder cannot be written.
 */
function writeWhole(target, data) {
  fs.mkdirSync(path.dirname(target), { recursive: true });
  const part = `${target}.part`;
  fs.writeFileSync(part, data);
  fs.renameSync(part, target);
}

/**
 * When the kept engine's lists were fetched.
 * @returns {number | null} The time in milliseconds since the epoch, or nothing when no engine is kept.
 */
function fetchedAt() {
  try {
    const kept = JSON.parse(fs.readFileSync(file('lists.json'), 'utf8'));
    return Number.isFinite(kept.fetched) ? kept.fetched : null;
  } catch (_) {
    return null;
  }
}

/**
 * Builds the engine from the kept file, when there is one this version of the engine reads.
 * @returns {boolean} Whether it was built.
 */
function loadKept() {
  try {
    engine = ElectronBlocker.deserialize(new Uint8Array(fs.readFileSync(file('engine.bin'))));
    return true;
  } catch (_) {
    return false;
  }
}

/**
 * Fetches one filter list.
 * @param {string} url Where it is.
 * @returns {Promise<string>} Its text.
 * @throws {Error} When it cannot be fetched or the server refuses it.
 */
async function fetchList(url) {
  const response = await net.fetch(url, { cache: 'no-store' });
  if (!response.ok) throw new Error(`${url} answered ${response.status}`);
  return response.text();
}

/**
 * Fetches the lists, builds the engine from them, keeps it on disk and puts it to work. A failure leaves the engine
 * there was, and the next try waits {@link policy.RETRY_AFTER}.
 * @returns {Promise<void>} Resolves when it is done, whether it worked or not.
 */
async function refresh() {
  if (fetching) return;
  fetching = true;
  try {
    const texts = await Promise.all(policy.LISTS.map(fetchList));
    const built = ElectronBlocker.parse(texts.join('\n'), { loadCosmeticFilters: true, loadNetworkFilters: true });
    writeWhole(file('engine.bin'), built.serialize());
    writeWhole(file('lists.json'), JSON.stringify({ fetched: Date.now(), lists: policy.LISTS }));
    engine = built;
    failedAt = 0;
  } catch (error) {
    failedAt = Date.now();
    console.error('the filter lists could not be fetched:', error instanceof Error ? error.message : error);
  } finally {
    fetching = false;
  }
}

/**
 * Fetches the lists again when they are too old and the last failure is long enough ago; builds the engine from what
 * is kept first, when there is no engine yet.
 * @returns {void}
 */
function freshen() {
  if (!enabled) return;
  if (!engine) loadKept();
  const now = Date.now();
  if (policy.stale(engine ? fetchedAt() : null, now) && now - failedAt >= policy.RETRY_AFTER) {
    refresh().catch(() => {});
  }
}

/**
 * Whether the blocker works on a page now.
 * @param {Page | undefined} page The page, or nothing when the contents are not one of the open pages.
 * @returns {boolean} Whether it does: blocking on, the page of the web, and its site neither this machine nor let through.
 */
function activeOn(page) {
  return Boolean(page && !page.sealed && engine && policy.blocksOn(enabled, page.contents.getURL(), allowed));
}

/**
 * Tells the window how the blocker stands on a page.
 * @param {Page} page The page.
 * @returns {void}
 */
function report(page) {
  const count = COUNTS.get(page);
  const on = !page.sealed && policy.blocksOn(enabled, page.contents.getURL(), allowed);
  page.send({ Chrome: { page: page.id, report: { Blocking: { on, blocked: count ? count.blocked : 0 } } } });
}

/**
 * Counts a stopped request of a page and reports the count soon.
 * @param {Page} page The page.
 * @returns {void}
 */
function counted(page) {
  let count = COUNTS.get(page);
  if (!count) {
    count = { blocked: 0, timer: null };
    COUNTS.set(page, count);
  }
  count.blocked += 1;
  if (count.timer) return;
  const held = count;
  held.timer = setTimeout(() => {
    held.timer = null;
    if (!page.win.isDestroyed()) report(page);
  }, REPORT_DELAY);
}

/**
 * The page that made a request.
 * @param {{ webContentsId?: number }} details What the engine says of the request.
 * @returns {Page | undefined} The page, or nothing for a request of no open page, such as one of a service worker.
 */
function pageOfRequest(details) {
  if (!details.webContentsId) return undefined;
  const contents = webContents.fromId(details.webContentsId);
  return contents ? pageOf(contents) : undefined;
}

/**
 * Lets a request through or stops it, as the lists say for the page that made it.
 * @param {import('electron').OnBeforeRequestListenerDetails} details The request.
 * @param {(response: import('electron').CallbackResponse) => void} callback Takes the answer.
 * @returns {void}
 */
function beforeRequest(details, callback) {
  const page = pageOfRequest(details);
  if (!page || !engine || !activeOn(page)) {
    callback({});
    return;
  }
  engine.onBeforeRequest(details, (response) => {
    if (response.cancel || response.redirectURL) counted(page);
    callback(response);
  });
}

/**
 * Adds the content security policy the lists ask for to a document of a page the blocker works on.
 * @param {import('electron').OnHeadersReceivedListenerDetails} details The response.
 * @param {(response: import('electron').HeadersReceivedResponse) => void} callback Takes the answer.
 * @returns {void}
 */
function headersReceived(details, callback) {
  const page = pageOfRequest(details);
  if (!page || !engine || !activeOn(page)) {
    callback({});
    return;
  }
  engine.onHeadersReceived(details, callback);
}

/**
 * Starts the blocker in every profile pages of the web live in, the shared one and each made later; it blocks nothing
 * until the window turns it on.
 * @param {object} options Where it keeps its files and how it finds pages.
 * @param {string} options.profile The helper's profile folder; the engine and the sites let through go in `content-blocking` under it.
 * @param {(contents: import('electron').WebContents | null) => Page | undefined} options.pageOf Finds the page of a window's contents.
 * @returns {void}
 */
function install(options) {
  folder = path.join(options.profile, 'content-blocking');
  pageOf = options.pageOf;
  try {
    allowed = policy.allowedFrom(JSON.parse(fs.readFileSync(file('allowed-sites.json'), 'utf8')));
  } catch (_) {
    allowed = new Set();
  }
  partitions.onSession((pages) => {
    pages.webRequest.onBeforeRequest({ urls: ['<all_urls>'] }, beforeRequest);
    pages.webRequest.onHeadersReceived({ urls: ['<all_urls>'] }, headersReceived);
    pages.registerPreloadScript({ type: 'frame', filePath: COSMETIC_PRELOAD });
  });
  ipcMain.handle('@ghostery/adblocker/inject-cosmetic-filters', (event, url, message) => {
    const page = pageOf(event.sender);
    if (!engine || !activeOn(page)) return undefined;
    return engine.onInjectCosmeticFilters(event, String(url), message);
  });
  ipcMain.handle('@ghostery/adblocker/is-mutation-observer-enabled', () => Boolean(engine && enabled && engine.config.enableMutationObserver));
}

/**
 * Turns blocking on or off, as the reader's setting says, and tells the window how it stands on every page.
 * @param {boolean} on Whether to block.
 * @param {Iterable<Page>} open The open pages.
 * @returns {void}
 */
function setEnabled(on, open) {
  enabled = on;
  if (on) {
    freshen();
    if (!checker) {
      checker = setInterval(freshen, policy.CHECK_EVERY);
      checker.unref();
    }
  } else if (checker) {
    clearInterval(checker);
    checker = null;
  }
  for (const page of open) if (!page.win.isDestroyed()) report(page);
}

/**
 * Lets the site of a page through, or blocks on it again, keeps the choice on disk and loads the page again.
 * @param {Page} page The page.
 * @param {boolean} on Whether to block on its site.
 * @returns {void}
 */
function setSite(page, on) {
  const site = policy.siteOf(page.contents.getURL());
  if (!site) return;
  if (on) allowed.delete(site);
  else allowed.add(site);
  try {
    writeWhole(file('allowed-sites.json'), JSON.stringify([...allowed].sort()));
  } catch (error) {
    console.error('the sites let through could not be kept:', error);
  }
  report(page);
  page.contents.reload();
}

/**
 * Starts counting what the blocker stops on a page, from naught at each load of a new document.
 * @param {Page} page The page.
 * @returns {void}
 */
function watch(page) {
  page.contents.on('did-start-navigation', (details) => {
    if (!details.isMainFrame || details.isSameDocument) return;
    const count = COUNTS.get(page);
    if (count && count.timer) clearTimeout(count.timer);
    COUNTS.set(page, { blocked: 0, timer: null });
  });
  page.contents.on('did-navigate', () => report(page));
}

module.exports = { install, setEnabled, setSite, watch };
