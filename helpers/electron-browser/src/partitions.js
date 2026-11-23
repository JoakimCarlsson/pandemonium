'use strict';

/**
 * The profiles pages live in, each the engine's session of a partition: the shared one every page uses unless the window
 * names another in `Open`, profiles kept on disk whose names start with `persist:`, such as the profile of one project,
 * and profiles kept in memory only, such as the one private tabs share, which are emptied when their last page closes.
 * Every profile gets the helper's guards, for permissions and downloads, before a page lives in it.
 */

const { session } = require('electron');

/** What the name of a profile the engine keeps on disk starts with. */
const KEPT = 'persist:';

/**
 * What every profile is given before a page lives in it.
 * @type {((pages: import('electron').Session) => void)[]}
 */
const guards = [];

/**
 * The profiles made so far, by name; the shared one is not among them.
 * @type {Map<string, import('electron').Session>}
 */
const made = new Map();

/**
 * What is told the name of a profile kept in memory once its last page closed, so that what was kept for it goes too.
 * @type {((partition: string) => void)[]}
 */
const releasers = [];

/**
 * Has `forget` told the name of each profile kept in memory once its last page closed.
 * @param {(partition: string) => void} forget Forgets what was kept for the profile.
 * @returns {void}
 */
function onRelease(forget) {
  releasers.push(forget);
}

/**
 * Gives every profile `guard`: the shared one and those made so far at once, and each made later as it is made.
 * @param {(pages: import('electron').Session) => void} guard What a profile is given.
 * @returns {void}
 */
function onSession(guard) {
  guards.push(guard);
  guard(session.defaultSession);
  for (const pages of made.values()) guard(pages);
}

/**
 * Whether the profile named `partition` is kept across runs: the shared one and every profile whose name starts with
 * `persist:`; any other is kept in memory only.
 * @param {string | undefined} partition The profile's name; nothing or empty for the shared one.
 * @returns {boolean} Whether it is kept.
 */
function kept(partition) {
  return !partition || partition.startsWith(KEPT);
}

/**
 * The profile named `partition`, made and guarded the first time it is asked for.
 * @param {string | undefined} partition The profile's name; nothing or empty for the shared one.
 * @returns {import('electron').Session} The profile.
 */
function sessionOf(partition) {
  if (!partition) return session.defaultSession;
  const held = made.get(partition);
  if (held) return held;
  const fresh = session.fromPartition(partition, { cache: kept(partition) });
  made.set(partition, fresh);
  for (const guard of guards) guard(fresh);
  return fresh;
}

/**
 * The name of the profile `pages` is, as a page that another opened learns which profile it lives in.
 * @param {import('electron').Session} pages A profile.
 * @returns {string} Its name, or empty for the shared one and any profile not made here.
 */
function partitionOf(pages) {
  for (const [name, held] of made) {
    if (held === pages) return name;
  }
  return '';
}

/**
 * Empties a profile kept in memory once none of `open` lives in it: its cookies, storage, cache and HTTP
 * authentication go, so what a private tab kept is gone when the last private tab closes. A profile kept on disk is
 * left as it is. What was kept for it elsewhere, such as the reader's answers to its pages' questions, is forgotten too.
 * @param {string | undefined} partition The profile of a page that closed.
 * @param {Iterable<{ partition: string }>} open The pages still open.
 * @returns {Promise<void>} Resolves once the profile is emptied, or at once when it is not to be.
 */
async function release(partition, open) {
  if (kept(partition)) return;
  for (const page of open) {
    if (page.partition === partition) return;
  }
  const name = /** @type {string} */ (partition);
  for (const forget of releasers) forget(name);
  const held = made.get(name);
  if (!held) return;
  await Promise.all([held.clearStorageData(), held.clearCache(), held.clearAuthCache()]).catch(() => {});
}

module.exports = { onSession, onRelease, kept, sessionOf, partitionOf, release };
