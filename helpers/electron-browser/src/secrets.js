'use strict';

/**
 * The fields of a page that hold secrets, such as passwords and one-time codes, kept out of recordings: what is typed
 * into them is not recorded, and they are blurred in the screenshots a recording takes.
 *
 * The scripts run in the tools' own scripting world, which the page cannot see, so the fields an agent typed a secret
 * into are remembered there even after a page turns a password field into a plain one to show what it holds.
 */

/** @typedef {import('./page').Page} Page */

/** The name of the scripting world the tools read and act on pages in. */
const WORLD = 'pm-agent';

/**
 * The script of a function that says whether an element is a field holding a secret: a password field, a field whose
 * `autocomplete` asks for a password or a one-time code, or a field an agent typed a secret into before.
 */
const IS_SECRET = `(el) => {
    if (!el || !(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return false;
    if (window.__pmSecrets && window.__pmSecrets.has(el)) return true;
    if (el instanceof HTMLInputElement && el.type === 'password') return true;
    const tokens = String(el.getAttribute('autocomplete') || '').toLowerCase().split(/\\s+/);
    return tokens.includes('current-password') || tokens.includes('new-password') || tokens.includes('one-time-code');
  }`;

/**
 * The script that tells, for each element an expression in `elements` finds, whether it holds a secret, and remembers
 * those that do so that they stay secret.
 * @param {string[]} elements Expressions that each find one element, or nothing.
 * @returns {string} A script that returns JSON text: a list of whether each is secret.
 */
function checkScript(elements) {
  const found = elements.map((element) => `(() => { try { return ${element}; } catch (_) { return null; } })()`).join(', ');
  return `(() => {
    const secret = ${IS_SECRET};
    window.__pmSecrets = window.__pmSecrets || new WeakSet();
    return JSON.stringify([${found}].map((el) => {
      const is = secret(el);
      if (is) window.__pmSecrets.add(el);
      return is;
    }));
  })()`;
}

/** The script that blurs every secret field of the page and keeps how each was styled, to be put back after. */
const MASK = `(() => {
    const secret = ${IS_SECRET};
    const masked = [];
    for (const el of document.querySelectorAll('input, textarea')) {
      if (!secret(el)) continue;
      const style = el.style;
      masked.push([el, style.getPropertyValue('filter'), style.getPropertyPriority('filter'), style.getPropertyValue('-webkit-text-security'), style.getPropertyPriority('-webkit-text-security')]);
      style.setProperty('filter', 'blur(8px)', 'important');
      style.setProperty('-webkit-text-security', 'disc', 'important');
    }
    window.__pmMasked = masked;
    return JSON.stringify(masked.length);
  })()`;

/** The script that styles the fields {@link MASK} blurred as they were before. */
const UNMASK = `(() => {
    for (const [el, filter, filterPriority, security, securityPriority] of window.__pmMasked || []) {
      if (filter) el.style.setProperty('filter', filter, filterPriority); else el.style.removeProperty('filter');
      if (security) el.style.setProperty('-webkit-text-security', security, securityPriority); else el.style.removeProperty('-webkit-text-security');
    }
    window.__pmMasked = [];
    return '0';
  })()`;

/**
 * Runs `script` in the tools' world of the page's main frame.
 * @param {Page} page The page, which must have a protocol to speak.
 * @param {string} script A script that returns JSON text.
 * @returns {Promise<any>} What it said; rejects when the page cannot be read.
 */
async function inWorld(page, script) {
  const cdp = /** @type {import('./cdp').Cdp} */ (page.cdp);
  const tree = await cdp.call('Page.getFrameTree');
  const made = await cdp.call('Page.createIsolatedWorld', { frameId: tree.frameTree.frame.id, worldName: WORLD, grantUniveralAccess: false });
  const reply = await cdp.call('Runtime.evaluate', { expression: script, contextId: made.executionContextId, returnByValue: true });
  if (!reply || reply.exceptionDetails || !reply.result) throw new Error('the page could not be read');
  return JSON.parse(reply.result.value);
}

/**
 * Whether each element the expressions find is a field holding a secret; those that are stay marked as secret.
 * @param {Page} page The page.
 * @param {string[]} elements Expressions that each find one element in the tools' world.
 * @returns {Promise<boolean[]>} Whether each is secret; every one is taken as secret when the page cannot be read, so
 *   that a secret is never kept for want of knowing.
 */
async function check(page, elements) {
  if (!elements.length) return [];
  const found = await inWorld(page, checkScript(elements)).catch(() => null);
  if (Array.isArray(found) && found.length === elements.length) return found.map((is) => is === true);
  return elements.map(() => true);
}

/**
 * Runs `take` with every secret field of the page blurred, then puts the fields back as they were.
 * @template T
 * @param {Page} page The page.
 * @param {() => Promise<T>} take What to do while the fields are blurred, such as taking a screenshot.
 * @returns {Promise<{ value: T, masked: boolean }>} What `take` gave, and whether the fields could be blurred; when not,
 *   the page could not be read, and the caller decides whether what `take` gave may be kept.
 */
async function masked(page, take) {
  const blurred = await inWorld(page, MASK).then(
    () => true,
    () => false,
  );
  try {
    return { value: await take(), masked: blurred };
  } finally {
    if (blurred) await inWorld(page, UNMASK).catch(() => null);
  }
}

module.exports = { WORLD, IS_SECRET, check, checkScript, masked };
