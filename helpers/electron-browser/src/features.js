'use strict';

/**
 * What this helper can do beyond what every helper does, announced to the window in `Features` before `Ready`, so
 * that the window sends a request only to a helper that carries it out. Mirrors `SEALED_PAGES` in
 * `crates/pm-browser/src/wire.rs`.
 */

/** The helper opens and renders documents sealed from the network: `OpenHtml` and `RenderHtml`. */
const SEALED_PAGES = 'sealed-pages';

/** The helper blocks ads and trackers as `Privacy` asks. Mirrors `CONTENT_BLOCKING` in `wire.rs`. */
const CONTENT_BLOCKING = 'content-blocking';

/** The helper offers to keep logins in the system's keychain and fills them in, as `Privacy` asks. Mirrors `PASSWORDS` in `wire.rs`. */
const PASSWORDS = 'passwords';

/** The helper opens a page in the profile the `partition` of `Open` names. Mirrors `PARTITIONS` in `wire.rs`. */
const PARTITIONS = 'partitions';

/** Every feature this helper announces. */
const FEATURES = Object.freeze([SEALED_PAGES, CONTENT_BLOCKING, PASSWORDS, PARTITIONS]);

/**
 * The report that announces this helper's features.
 * @returns {{ Features: { features: string[] } }} The `Features` event.
 */
function announcement() {
  return { Features: { features: [...FEATURES] } };
}

module.exports = { SEALED_PAGES, CONTENT_BLOCKING, PASSWORDS, PARTITIONS, FEATURES, announcement };
