'use strict';

/**
 * What this helper can do beyond what every helper does, announced to the window in `Features` before `Ready`, so
 * that the window sends a request only to a helper that carries it out. Mirrors `SEALED_PAGES` in
 * `crates/pm-browser/src/wire.rs`.
 */

/** The helper opens and renders documents sealed from the network: `OpenHtml` and `RenderHtml`. */
const SEALED_PAGES = 'sealed-pages';

/** Every feature this helper announces. */
const FEATURES = Object.freeze([SEALED_PAGES]);

/**
 * The report that announces this helper's features.
 * @returns {{ Features: { features: string[] } }} The `Features` event.
 */
function announcement() {
  return { Features: { features: [...FEATURES] } };
}

module.exports = { SEALED_PAGES, FEATURES, announcement };
