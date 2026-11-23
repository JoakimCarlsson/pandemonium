'use strict';

/**
 * Keeping and filling in logins. The preload `src/login-preload.js`, which runs in the top frame of every page of the
 * web, tells the helper when the reader sends a login form and when a field of one takes the keyboard. After a send
 * the page's next navigation offers to keep the login (`OfferPassword`), and the reader's answer (`AnswerPassword`)
 * keeps it in a {@link Vault} sealed by the system's keychain through `safeStorage`. When a field of a login form takes
 * the keyboard on an origin with logins kept, the window is told where it is and which user names there are
 * (`LoginField`); the reader's pick (`FillLogin`) sends the login to the preload, which fills the form.
 *
 * Where the keychain does not really protect anything, as on a Linux desktop without a keyring, where `safeStorage`
 * falls back to a key every install shares, the logins are kept in a vault of their own sealed by a primary password
 * instead ({@link PrimaryLock}). The window is asked for it in the pane (`PrimaryPassword`) the first time a login is
 * to be saved, to set it, and whenever the locked vault must be opened to save or fill one; the reader's answer
 * (`PrimaryPassword` action) unlocks the vault until the helper ends.
 *
 * Passwords never leave the helper for the window, and none of this works for agents: a login sent or a field focused
 * while a tool of an agent runs on the page is ignored, a fill is refused then, sealed pages never run the preload, and
 * no tool reads the vault.
 */

const { ipcMain, safeStorage } = require('electron');
const path = require('path');
const partitions = require('./partitions');
const { Vault, originOf } = require('./vault');
const { PrimaryLock } = require('./primary');

/** @typedef {import('./page').Page} Page */

/** How long after a login was sent the page's next navigation still offers to keep it, in milliseconds. */
const SENT_PATIENCE = 60 * 1000;

/** How long an offer stays open without an answer, in milliseconds. */
const OFFER_PATIENCE = 2 * 60 * 1000;

/** The longest user name or password taken from a page, in characters. */
const LONGEST = 1024;

/** Whether the reader has saving passwords on in the settings; off until the window says. */
let enabled = false;

/**
 * The kept logins, once installed and the way they are sealed is chosen.
 * @type {InstanceType<typeof Vault> | null}
 */
let vault = null;

/**
 * The primary password, when the logins are sealed by one rather than the system's keychain.
 * @type {InstanceType<typeof PrimaryLock> | null}
 */
let lock = null;

/**
 * A question for the primary password the reader has not answered, and what to do once the vault is open.
 * @typedef {object} Asking
 * @property {Page} page The page it is shown on.
 * @property {string} origin The origin the login is for.
 * @property {() => Promise<void>} then What to do once the vault is unlocked.
 */

/**
 * The open question for the primary password, at most one.
 * @type {Asking | null}
 */
let asking = null;

/**
 * The pages whose reader turned down unlocking the vault, not to be asked again on a login field before they navigate.
 * @type {WeakSet<Page>}
 */
const DECLINED = new WeakSet();

/** The number the next offer gets. */
let nextOffer = 1;

/**
 * Finds the page of a window's contents.
 * @type {(contents: import('electron').WebContents | null) => Page | undefined}
 */
let pageOf = () => undefined;

/**
 * A login a page sent, waiting for the page's next navigation to be offered.
 * @typedef {object} Sent
 * @property {string} origin The origin it was sent from.
 * @property {string} username The user name.
 * @property {string} password The password.
 * @property {number} at When it was sent, in milliseconds since the epoch.
 */

/**
 * The login each page sent last, until it is offered or too old.
 * @type {WeakMap<Page, Sent>}
 */
const SENT = new WeakMap();

/**
 * An offer the reader has not answered.
 * @typedef {object} Offer
 * @property {Page} page The page it is shown on.
 * @property {string} origin The origin the login is for.
 * @property {string} username The user name.
 * @property {string} password The password.
 * @property {ReturnType<typeof setTimeout>} timer Closes it when nobody answers.
 */

/**
 * The open offers, by number.
 * @type {Map<number, Offer>}
 */
const OFFERS = new Map();

/**
 * The pages whose window was told a login field holds the keyboard.
 * @type {WeakSet<Page>}
 */
const FOCUSED = new WeakSet();

/**
 * Whether the system's keychain can seal logins: `safeStorage` is available and, on Linux, backed by a keyring rather
 * than the fixed key it falls back to.
 * @returns {Promise<boolean>} Whether the keychain protects them; otherwise a primary password must.
 */
async function keychainReady() {
  if (!(await safeStorage.isAsyncEncryptionAvailable())) return false;
  if (process.platform !== 'linux') return true;
  return safeStorage.getSelectedStorageBackend() !== 'basic_text';
}

/**
 * Tells the window something about a page.
 * @param {Page} page The page.
 * @param {import('./types').Json | string} report The report, a `PageReport`.
 * @returns {void}
 */
function report(page, report) {
  if (!page.win.isDestroyed()) page.send({ Chrome: { page: page.id, report: /** @type {import('./types').Json} */ (report) } });
}

/**
 * The page a message of the preload came from, when it may be heard: from the top frame of an open page of the web,
 * with saving on and no tool of an agent running on the page.
 * @param {import('electron').IpcMainEvent} event The message.
 * @returns {Page | undefined} The page, or nothing when the message is ignored.
 */
function heard(event) {
  const page = pageOf(event.sender);
  if (!page || !keepsLogins(page) || !enabled || !vault || page.agentBusy()) return undefined;
  const frame = event.senderFrame;
  if (!frame || frame !== event.sender.mainFrame) return undefined;
  return page;
}

/**
 * Whether logins are kept and filled on a page: a page of the web in a profile kept on disk, never a sealed page or one
 * of a private tab, whose profile is kept in memory only.
 * @param {Page} page The page.
 * @returns {boolean} Whether they are.
 */
function keepsLogins(page) {
  return !page.sealed && partitions.kept(page.partition);
}

/**
 * Registers the login preload with a profile kept on disk; a private profile never gets it, so its pages never report
 * a login.
 * @param {import('electron').Session} pages The profile.
 * @returns {void}
 */
function guard(pages) {
  if (!partitions.kept(partitions.partitionOf(pages))) return;
  pages.registerPreloadScript({ type: 'frame', filePath: path.join(__dirname, 'login-preload.js') });
}

/**
 * A field of a page's text as the helper keeps it.
 * @param {unknown} value What the preload sent.
 * @returns {string} It as text, cut at {@link LONGEST}.
 */
function text(value) {
  return String(value ?? '').slice(0, LONGEST);
}

/**
 * Takes the window's question about a login field away, when it was asked.
 * @param {Page} page The page.
 * @returns {void}
 */
function leaveField(page) {
  if (!FOCUSED.has(page)) return;
  FOCUSED.delete(page);
  report(page, 'LoginFieldLeft');
}

/**
 * Closes an offer, and tells the window when it did not answer it itself.
 * @param {number} id The offer.
 * @param {boolean} answered Whether the reader answered it.
 * @returns {Offer | undefined} The offer, when it was open.
 */
function closeOffer(id, answered) {
  const offer = OFFERS.get(id);
  if (!offer) return undefined;
  OFFERS.delete(id);
  clearTimeout(offer.timer);
  if (!answered) report(offer.page, { PasswordOfferClosed: { id } });
  return offer;
}

/**
 * Offers to keep the login a page sent, when it is not kept as it is and its origin is not refused.
 * @param {Page} page The page.
 * @param {Sent} sent The login.
 * @returns {Promise<void>} Resolves once it is offered, or found not worth offering.
 */
async function offer(page, sent) {
  if (!vault || !enabled || vault.never.has(sent.origin)) return;
  const login = { username: sent.username, password: sent.password };
  const known = lock && !lock.unlocked() ? 'new' : await vault.compare(sent.origin, login);
  if (known === 'same' || !enabled || page.win.isDestroyed()) return;
  for (const [id, open] of OFFERS) if (open.page === page) closeOffer(id, false);
  const id = nextOffer++;
  const timer = setTimeout(() => closeOffer(id, false), OFFER_PATIENCE);
  OFFERS.set(id, { page, origin: sent.origin, username: sent.username, password: sent.password, timer });
  report(page, { OfferPassword: { id, origin: sent.origin, username: sent.username, update: known === 'update' } });
}

/**
 * Offers the login a page sent a moment ago, now that the page moved on.
 * @param {Page} page The page.
 * @returns {void}
 */
function moved(page) {
  leaveField(page);
  DECLINED.delete(page);
  const sent = SENT.get(page);
  if (!sent) return;
  SENT.delete(page);
  if (Date.now() - sent.at <= SENT_PATIENCE) {
    offer(page, sent).catch((error) => console.error('the login could not be offered:', error instanceof Error ? error.message : error));
  }
}

/**
 * Tells the window a field of a login form holds the keyboard on a page, with where it is and the user names kept for
 * the page's origin.
 * @param {Page} page The page.
 * @param {string} origin The page's origin.
 * @param {unknown} box Where the field is in the page's viewport, in CSS pixels, as the preload sent it.
 * @returns {Promise<void>} Resolves once the window is told, or nothing is to be told.
 */
async function suggest(page, origin, box) {
  if (!vault) return;
  if (lock && !lock.unlocked()) {
    if (!DECLINED.has(page)) ask(page, origin, () => suggest(page, origin, box), '');
    return;
  }
  const usernames = (await vault.loginsOf(origin)).map((kept) => kept.username);
  if (usernames.length === 0 || page.win.isDestroyed() || originOf(page.contents.getURL()) !== origin) return;
  const zoom = page.readerZoom();
  const at = /** @type {Record<string, unknown>} */ (box && typeof box === 'object' ? box : {});
  FOCUSED.add(page);
  report(page, {
    LoginField: {
      x: Math.round(Number(at.x) * zoom) || 0,
      y: Math.round(Number(at.y) * zoom) || 0,
      width: Math.round(Number(at.width) * zoom) || 0,
      height: Math.round(Number(at.height) * zoom) || 0,
      usernames,
    },
  });
}

/**
 * Starts keeping logins: opens the vault under the profile, registers the preload with every profile pages of the web
 * live in that is kept on disk, and listens to it; private tabs neither save nor fill logins.
 * @param {object} options Where the vault is and how pages are found.
 * @param {string} options.profile The helper's profile folder; the vault is `passwords.json` in it.
 * @param {(contents: import('electron').WebContents | null) => Page | undefined} options.pageOf Finds the page of a window's contents.
 * @returns {void}
 */
function install(options) {
  pageOf = options.pageOf;
  choose(options.profile).catch((error) => console.error('the saved logins could not be opened:', error instanceof Error ? error.message : error));
  partitions.onSession(guard);
  ipcMain.on('pandemonium:login-sent', (event, username, password) => {
    const page = heard(event);
    const origin = page ? originOf(event.senderFrame?.url || '') : '';
    if (!page || !origin || !password) return;
    SENT.set(page, { origin, username: text(username), password: text(password), at: Date.now() });
  });
  ipcMain.on('pandemonium:login-focus', (event, box) => {
    const page = heard(event);
    const origin = page ? originOf(event.senderFrame?.url || '') : '';
    if (!page || !vault || !origin || !vault.has(origin)) return;
    suggest(page, origin, box).catch((error) => console.error('the logins could not be read:', error instanceof Error ? error.message : error));
  });
  ipcMain.on('pandemonium:login-left', (event) => {
    const page = pageOf(event.sender);
    if (page) leaveField(page);
  });
}

/**
 * Chooses how logins are sealed and opens their vault: by the system's keychain in `passwords.json` where it protects
 * them, otherwise by a primary password in `passwords-primary.json`, locked until the reader gives it.
 * @param {string} profile The helper's profile folder.
 * @returns {Promise<void>} Resolves once the vault is open.
 */
async function choose(profile) {
  if (await keychainReady()) {
    vault = new Vault(path.join(profile, 'passwords.json'), {
      seal: (plain) => safeStorage.encryptStringAsync(plain),
      open: async (sealed) => (await safeStorage.decryptStringAsync(sealed)).result,
    });
    console.error(`saved logins are sealed by the system's keychain (${backendName()})`);
    return;
  }
  lock = new PrimaryLock(path.join(profile, 'passwords-primary.key.json'));
  vault = new Vault(path.join(profile, 'passwords-primary.json'), lock.cipher());
  console.error(`saved logins are sealed by a primary password (the keychain's backend is ${backendName()})`);
}

/**
 * The name of the backend `safeStorage` chose, for the log.
 * @returns {string} It, or the platform's name where there is no choice.
 */
function backendName() {
  return process.platform === 'linux' ? safeStorage.getSelectedStorageBackend() : process.platform;
}

/**
 * Asks the reader, on a page, for the primary password: to set one when none is, otherwise to unlock the vault. An
 * open question on another page is closed first.
 * @param {Page} page The page to ask on.
 * @param {string} origin The origin of the login that needs the vault.
 * @param {() => Promise<void>} then What to do once the vault is unlocked.
 * @param {string} problem Why the last answer was refused, or nothing.
 * @returns {void}
 */
function ask(page, origin, then, problem) {
  if (!lock || page.win.isDestroyed()) return;
  if (asking && asking.page !== page) report(asking.page, 'PrimaryPasswordClosed');
  asking = { page, origin, then };
  report(page, { PrimaryPassword: { create: !lock.isSet(), origin, problem } });
}

/**
 * Does something with the vault once it is open: at once with the keychain or an unlocked primary password, otherwise
 * after the reader gave the primary password.
 * @param {Page} page The page to ask on.
 * @param {string} origin The origin of the login.
 * @param {() => Promise<void>} then What to do.
 * @returns {Promise<void>} Resolves once it is done, or the question is asked.
 */
async function whenUnlocked(page, origin, then) {
  if (!lock || lock.unlocked()) return then();
  ask(page, origin, then, '');
}

/**
 * Carries out the reader's answer to the question for the primary password on a page: sets it or unlocks the vault
 * with it and does what waited for it, asks again saying why when it was refused, or drops the question when the
 * reader turned it down.
 * @param {Page} page The page the answer came from.
 * @param {string | null} password What the reader typed, or nothing when they turned it down.
 * @returns {Promise<void>} Resolves once the answer is carried out.
 */
async function primaryPassword(page, password) {
  const open = asking;
  if (!lock || !open || open.page !== page) return;
  if (password === null) {
    asking = null;
    DECLINED.add(page);
    return;
  }
  const problem = lock.isSet()
    ? ((await lock.unlock(password)) ? '' : 'That is not the primary password.')
    : await lock.create(password);
  if (asking !== open) return;
  if (problem) {
    ask(page, open.origin, open.then, problem);
    return;
  }
  asking = null;
  report(page, 'PrimaryPasswordClosed');
  await open.then();
}

/**
 * Turns saving passwords on or off, as the reader's setting says; turning it off closes every offer and question.
 * @param {boolean} on Whether to offer and fill.
 * @param {Iterable<Page>} open The open pages.
 * @returns {void}
 */
function setEnabled(on, open) {
  enabled = on;
  if (on) return;
  if (asking) {
    report(asking.page, 'PrimaryPasswordClosed');
    asking = null;
  }
  for (const id of [...OFFERS.keys()]) closeOffer(id, false);
  for (const page of open) leaveField(page);
}

/**
 * Carries out the reader's answer to an offer: keeps the login, refuses its origin for good, or neither.
 * @param {number} id The offer.
 * @param {boolean} save Whether to keep the login.
 * @param {boolean} never Whether never to offer for its origin again.
 * @returns {Promise<void>} Resolves once the vault is written, or nothing was to be written.
 */
async function answer(id, save, never) {
  const offer = closeOffer(id, true);
  if (!offer || !vault) return;
  try {
    if (save) {
      const kept = vault;
      await whenUnlocked(offer.page, offer.origin, () => kept.save(offer.origin, { username: offer.username, password: offer.password }, Date.now()));
    } else if (never) vault.refuse(offer.origin);
  } catch (error) {
    console.error('the login could not be kept:', error instanceof Error ? error.message : error);
  }
}

/**
 * Fills the login form of a page with the login kept for a user name on the page's origin, when no tool of an agent
 * runs on the page.
 * @param {Page} page The page.
 * @param {string} username The user name the reader picked.
 * @returns {Promise<void>} Resolves once the login is sent to the page, or refused.
 */
async function fill(page, username) {
  if (!vault || !enabled || !keepsLogins(page) || page.agentBusy()) return;
  const origin = originOf(page.contents.getURL());
  leaveField(page);
  if (!origin) return;
  const kept = vault;
  await whenUnlocked(page, origin, async () => {
    const login = (await kept.loginsOf(origin)).find((known) => known.username === username);
    if (!login || page.win.isDestroyed() || page.agentBusy() || originOf(page.contents.getURL()) !== origin) return;
    page.contents.mainFrame.send('pandemonium:login-fill', origin, login.username, login.password);
  });
}

/**
 * Follows a page so a login it sent is offered once it moves on, and its offers close when it goes away.
 * @param {Page} page The page.
 * @returns {void}
 */
function watch(page) {
  page.contents.on('did-navigate', () => moved(page));
  page.contents.on('did-navigate-in-page', (_event, _url, isMainFrame) => {
    if (isMainFrame) moved(page);
  });
  page.contents.once('destroyed', () => {
    if (asking?.page === page) asking = null;
    for (const [id, open] of OFFERS) if (open.page === page) closeOffer(id, true);
  });
}

module.exports = { install, setEnabled, answer, fill, watch, primaryPassword };
