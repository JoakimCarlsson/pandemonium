'use strict';

/**
 * Reader view: whether a page looks like an article, and its article taken out of it without menus, sidebars and
 * advertising, both found by the readability algorithm of `@mozilla/readability` run in a world of the page's own that
 * its scripts cannot reach or change. The window shows the article as a sealed page styled like the app.
 */

const fs = require('fs');

/** The scripting world reader view runs in, apart from the page's own scripts and the other tools' worlds. */
const READER_WORLD = 7351;

/** The most bytes of article HTML reported, which leaves room in a sealed document for the styles around it. */
const LARGEST_ARTICLE = 240 * 1024;

/** How long after a page stopped loading it is looked at again, for pages whose scripts put the article in late, in milliseconds. */
const LOOK_AGAIN = 1500;

/**
 * The source of the readability algorithm and its quick check, read once.
 * @type {string | null}
 */
let source = null;

/**
 * The script that makes the readability algorithm ready in the world it runs in, once per document, as
 * `globalThis.pandemoniumReader`.
 * @returns {string} The script.
 * @throws {Error} When the algorithm's files are not installed beside the helper.
 */
function readyScript() {
  if (source === null) {
    const article = fs.readFileSync(require.resolve('@mozilla/readability/Readability.js'), 'utf8');
    const check = fs.readFileSync(require.resolve('@mozilla/readability/Readability-readerable.js'), 'utf8');
    source = `globalThis.pandemoniumReader = globalThis.pandemoniumReader || (function () {\n${article}\n${check}\nreturn { Readability, isProbablyReaderable };\n})();`;
  }
  return source;
}

/**
 * Runs `script` in reader view's world of the page's top frame, after making the algorithm ready there.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {string} script The script; its value is the answer.
 * @returns {Promise<unknown>} What the script came to.
 */
function inWorld(contents, script) {
  return contents.executeJavaScriptInIsolatedWorld(READER_WORLD, [{ code: `${readyScript()}\n${script}` }]);
}

/**
 * Whether the page's document looks like an article reader view can show.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {Promise<boolean>} Whether it does; no when the page cannot be read.
 */
async function readable(contents) {
  try {
    const url = contents.getURL();
    if (!/^https?:/i.test(url)) return false;
    return (await inWorld(contents, 'pandemoniumReader.isProbablyReaderable(document)')) === true;
  } catch {
    return false;
  }
}

/**
 * The article of the page, as a `PageReport::Article` holds it, or why there is none.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {Promise<{ url: string, title: string, byline: string, site: string, lang: string, dir: string, html: string, error: string }>} The article.
 */
async function extract(contents) {
  const url = contents.getURL();
  const none = { url, title: contents.getTitle(), byline: '', site: '', lang: '', dir: '', html: '', error: '' };
  try {
    const found = /** @type {{ title?: unknown, byline?: unknown, siteName?: unknown, lang?: unknown, dir?: unknown, content?: unknown } | null} */ (
      await inWorld(
        contents,
        `(() => { const found = new pandemoniumReader.Readability(document.cloneNode(true)).parse(); return found && { title: found.title, byline: found.byline, siteName: found.siteName, lang: found.lang, dir: found.dir, content: found.content }; })()`,
      )
    );
    const text = (/** @type {unknown} */ value) => (typeof value === 'string' ? value : '');
    if (!found || !text(found.content)) return { ...none, error: 'No article was found on this page.' };
    const html = text(found.content);
    if (Buffer.byteLength(html, 'utf8') > LARGEST_ARTICLE) return { ...none, error: 'The article is too long for reader view.' };
    return {
      url,
      title: text(found.title) || none.title,
      byline: text(found.byline),
      site: text(found.siteName),
      lang: text(found.lang),
      dir: text(found.dir),
      html,
      error: '',
    };
  } catch (error) {
    return { ...none, error: `The article could not be read: ${error instanceof Error ? error.message : String(error)}` };
  }
}

/**
 * Tells the window whether the page looks like an article each time it stops loading, and again a moment later for
 * pages that build their article late, and that it does not once it starts loading another document; only changes are told.
 * @param {import('./page').Page} page The page.
 * @param {(report: import('./types').Json) => void} report Tells the window a `PageReport` of the page.
 * @returns {void}
 */
function watch(page, report) {
  const contents = page.contents;
  /** @type {boolean} What the window was last told. */
  let told = false;
  /** @type {number} How many documents the page started, so a late look at an older one says nothing. */
  let turn = 0;
  /**
   * Tells the window `now` when it is not what it was told last.
   * @param {boolean} now Whether the page looks like an article.
   * @returns {void}
   */
  const tell = (now) => {
    if (now === told) return;
    told = now;
    report({ Readable: { readable: now } });
  };
  /**
   * Looks at the document of the turn `asked` and tells what it finds, unless another document started since.
   * @param {number} asked The turn the look is for.
   * @returns {void}
   */
  const look = (asked) => {
    if (contents.isDestroyed()) return;
    readable(contents).then((now) => {
      if (asked === turn && !contents.isDestroyed()) tell(now);
    });
  };
  contents.on('did-start-navigation', (details) => {
    if (!details.isMainFrame || details.isSameDocument) return;
    turn += 1;
    tell(false);
  });
  contents.on('did-stop-loading', () => {
    const asked = turn;
    look(asked);
    setTimeout(() => {
      if (!told) look(asked);
    }, LOOK_AGAIN);
  });
}

/**
 * Takes the article out of the page and tells the window, as a `PageReport::Article`.
 * @param {import('./page').Page} page The page.
 * @param {(report: import('./types').Json) => void} report Tells the window a `PageReport` of the page.
 * @returns {Promise<void>} Settles once the window has been told.
 */
async function read(page, report) {
  const article = page.sealed ? { url: '', title: '', byline: '', site: '', lang: '', dir: '', html: '', error: 'This page has no article.' } : await extract(page.contents);
  if (!page.contents.isDestroyed()) report({ Article: article });
}

module.exports = { extract, read, readable, watch, READER_WORLD };
