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

/** The attribute that marks the element picked, for the script that reads what the page's own scripts say about it. */
const MARK = 'data-pm-picked';

/** The script that, run in the page's own world, finds the marked element, takes the mark off and gives the component that rendered it and that component's properties, when the page is built with React. */
const COMPONENT = `(() => {
  const el = document.querySelector('[${MARK}]');
  if (!el) return null;
  el.removeAttribute('${MARK}');
  const key = Object.keys(el).find((k) => k.startsWith('__reactFiber$') || k.startsWith('__reactInternalInstance$'));
  let fiber = key ? el[key] : null;
  while (fiber) {
    const type = fiber.type;
    let name = '';
    if (typeof type === 'function') {
      name = type.displayName || type.name || '';
    } else if (type && typeof type === 'object') {
      const inner = type.render || type.type;
      name = type.displayName || (inner && (inner.displayName || inner.name)) || '';
    }
    if (name) {
      const props = [];
      const own = fiber.memoizedProps || {};
      for (const k of Object.keys(own)) {
        if (k === 'children' || k === 'key' || k === 'ref') continue;
        const v = own[k];
        if (typeof v === 'string') props.push([k, JSON.stringify(v)]);
        else if (typeof v === 'number' || typeof v === 'boolean') props.push([k, '{' + v + '}']);
        else if (v === null) props.push([k, '{null}']);
        if (props.length >= 10) break;
      }
      return { name, props };
    }
    fiber = fiber.return;
  }
  return null;
})()`;

/**
 * Text as one line: runs of white space become one space, and what is longer than `most` is cut and ends in an ellipsis.
 * @param {unknown} value What a page said.
 * @param {number} most The most characters to keep.
 * @returns {string} The text, or nothing when `value` is not text.
 */
function line(value, most) {
  if (typeof value !== 'string') return '';
  const text = value.replace(/\s+/g, ' ').trim();
  return text.length > most ? `${text.slice(0, most)}\u2026` : text;
}

/**
 * Name and value pairs as a page gave them, kept short and without repeats.
 * @param {unknown} pairs What a page said: a list of two-item lists.
 * @param {number} most The most pairs to keep.
 * @param {string[]} [skip] Names to leave out.
 * @returns {[string, string][]} The pairs.
 */
function pairsOf(pairs, most, skip = []) {
  /** @type {[string, string][]} */
  const kept = [];
  const seen = new Set(skip);
  for (const pair of Array.isArray(pairs) ? pairs : []) {
    if (kept.length >= most) break;
    const name = line(Array.isArray(pair) ? pair[0] : '', 60);
    if (!name || seen.has(name)) continue;
    seen.add(name);
    kept.push([name, line(Array.isArray(pair) ? pair[1] : '', 400)]);
  }
  return kept;
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
 * What a picked element is told as, with every field cut to a size a prompt can take, as a page chooses what it says.
 * @param {Record<string, any>} element What the picker script reported.
 * @param {Record<string, any> | null} component What the page's own scripts said about the element's component, if anything.
 * @returns {PickedElement} The element, safe to pass on.
 */
function limited(element, component) {
  return {
    selector: line(element.selector, 600),
    tag: line(element.tag, 40),
    attributes: pairsOf(element.attributes, 60, [MARK]),
    text: line(element.text, 2000),
    styles: pairsOf(element.styles, 40),
    width: number(element.width),
    height: number(element.height),
    component: component ? line(component.name, 100) : '',
    props: component ? pairsOf(component.props, 10) : [],
    url: line(element.url, 500),
    title: line(element.title, 200),
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
      if (!found || !found.element) {
        report(null);
        return;
      }
      const element = found.element;
      cdp
        .call('Runtime.evaluate', { expression: COMPONENT, returnByValue: true })
        .then(
          (reply) => reply.result.value || null,
          () => null,
        )
        .then((component) => report(limited(element, component)));
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
