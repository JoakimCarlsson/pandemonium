'use strict';

/**
 * Picking an element of a page: a script that runs in a scripting world the page cannot see, and a
 * binding only that world can call to report what was picked.
 */

const fs = require('fs');
const path = require('path');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').PickedElement} PickedElement */

/** The script that, run in a page, lets the reader pick an element and reports it through the binding. */
const SCRIPT = fs.readFileSync(path.join(__dirname, 'pick-page.js'), 'utf8');

/** The name of the scripting world the script runs in. */
const WORLD = 'pm-pick';

/** The name of the function that world reports through. */
const BINDING = '__pmPicked';

/**
 * A string cut to at most `most` characters.
 * @param {unknown} value What a page said.
 * @param {number} most The most characters to keep.
 * @returns {string} The text cut short, or nothing when `value` is not text.
 */
function cut(value, most) {
  return typeof value === 'string' ? value.slice(0, most) : '';
}

/**
 * A number the page gave, or zero when it gave something else.
 * @param {unknown} value What a page said.
 * @returns {number} The number.
 */
function number(value) {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

/**
 * A picked element with every field cut to a size a prompt can take, as a page chooses what it says.
 * @param {Record<string, any>} element What the page reported.
 * @returns {PickedElement} The element, safe to pass on.
 */
function limited(element) {
  const classes = (Array.isArray(element.classes) ? element.classes : []).slice(0, 20).map((c) => cut(c, 100));
  /** @type {[string, string][]} */
  const styles = (Array.isArray(element.styles) ? element.styles : [])
    .slice(0, 20)
    .map((/** @type {[unknown, unknown]} */ [name, value]) => [cut(name, 40), cut(value, 200)]);
  return {
    selector: cut(element.selector, 300),
    tag: cut(element.tag, 40),
    id: cut(element.id, 200),
    classes,
    text: cut(element.text, 400),
    html: cut(element.html, 2000),
    x: number(element.x),
    y: number(element.y),
    width: number(element.width),
    height: number(element.height),
    scroll_x: number(element.scroll_x),
    scroll_y: number(element.scroll_y),
    screenshot: null,
    styles,
    url: cut(element.url, 500),
    title: cut(element.title, 200),
  };
}

/**
 * Turns pick mode on in `page`.
 * @param {Page} page The page, which must have a protocol to speak.
 * @param {(element: PickedElement | null) => void} report Called with the element picked, or with nothing when the mode ended without a pick.
 * @returns {Promise<void>} Resolves once the mode is on.
 */
async function start(page, report) {
  const cdp = page.cdp;
  if (!cdp) throw new Error('the page has no protocol');
  if (!page.pickListening) {
    page.pickListening = true;
    cdp.listen((method, params) => {
      if (method !== 'Runtime.bindingCalled' || params.name !== BINDING) return;
      if (params.executionContextId === undefined || params.executionContextId !== page.pickContext) return;
      /** @type {Record<string, any> | null} */
      let found;
      try {
        found = JSON.parse(params.payload);
      } catch (_) {
        return;
      }
      page.pickContext = null;
      report(found && found.element ? limited(found.element) : null);
    });
  }
  await cdp.call('Runtime.enable');
  await cdp.call('Page.enable');
  await cdp.call('Runtime.addBinding', { name: BINDING, executionContextName: WORLD });
  const tree = await cdp.call('Page.getFrameTree');
  const world = await cdp.call('Page.createIsolatedWorld', {
    frameId: tree.frameTree.frame.id,
    worldName: WORLD,
    grantUniveralAccess: false,
  });
  page.pickContext = world.executionContextId;
  await cdp.call('Runtime.evaluate', { contextId: world.executionContextId, expression: SCRIPT });
}

/**
 * Turns pick mode off in `page`.
 * @param {Page} page The page.
 * @returns {Promise<void>} Resolves once the mode is off.
 */
async function stop(page) {
  if (page.pickContext === null || !page.cdp) return;
  const contextId = page.pickContext;
  page.pickContext = null;
  await page.cdp
    .call('Runtime.evaluate', { contextId, expression: 'window.__pmPick && window.__pmPick.stop(true);' })
    .catch(() => {});
}

module.exports = { start, stop };
