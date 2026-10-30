'use strict';

/**
 * What `browser_cdp` may send to a page: a short allowlist of read-mostly DevTools commands, decided
 * by one pure function.
 *
 * Raw protocol access could navigate (past the origin restriction), read files, intercept traffic or
 * detach the helper's own listeners, so everything not named here is refused.
 */

/** The domains every command of which is allowed, except the ones in `DENIED`. */
const WHOLE_DOMAINS = ['DOM', 'CSS', 'Accessibility', 'DOMSnapshot', 'Performance'];

/** The commands allowed one by one, for the domains that hold others that are not. */
const ALLOWED_COMMANDS = new Set([
  'Runtime.evaluate',
  'Runtime.callFunctionOn',
  'Runtime.getProperties',
  'Runtime.releaseObject',
  'Runtime.releaseObjectGroup',
  'Runtime.globalLexicalScopeNames',
  'Runtime.getHeapUsage',
  'Page.getLayoutMetrics',
  'Page.getFrameTree',
  'Page.captureScreenshot',
  'Network.getResponseBody',
  'Emulation.setEmulatedMedia',
  'Emulation.setEmulatedVisionDeficiency',
  'Emulation.setScrollbarsHidden',
  'Overlay.enable',
  'Overlay.disable',
  'Overlay.highlightNode',
  'Overlay.highlightRect',
  'Overlay.highlightQuad',
  'Overlay.hideHighlight',
]);

/** The commands of an allowed domain that are refused, with why. */
const DENIED = new Map([
  ['DOM.setFileInputFiles', 'it hands local files to the page'],
  ['DOM.getFileInfo', 'it reads local file paths'],
]);

/** The commands that would work against what agent tools keep, with what to use instead. */
const REDIRECTED = new Map([
  ['Emulation.setDeviceMetricsOverride', 'use browser_emulate or browser_resize, which keep the size when the pane is resized'],
  ['Emulation.clearDeviceMetricsOverride', 'use browser_emulate or browser_resize with reset'],
  ['Emulation.setUserAgentOverride', 'use browser_emulate with user_agent'],
  ['Emulation.setTouchEmulationEnabled', 'use browser_emulate with mobile'],
]);

/** Commands and domains that are refused by name, so that the refusal says why. */
const REFUSED_PREFIXES = [
  ['Page.navigate', 'it would move the page past the agent origin restriction'],
  ['Target.', 'it can open pages and reach other targets'],
  ['Browser.', 'it reaches the whole browser, such as downloads and permissions'],
  ['IO.', 'it reads streams, which can hold files'],
  ['Fetch.', "it intercepts and rewrites the page's traffic"],
  ['Network.', 'it reads or changes traffic and cookies; use browser_network'],
  ['Storage.', 'it reads and clears site storage and cookies'],
  ['FileSystem.', 'it reaches local files'],
  ['Page.setDocumentContent', 'it replaces the page'],
  ['Page.addScriptToEvaluateOnNewDocument', 'it plants scripts that run on every page'],
  ['Page.setDownloadBehavior', 'it controls downloads to disk'],
  ['Page.setInterceptFileChooserDialog', 'it is about local files'],
  ['Debugger.', 'it pauses the page'],
  ['Input.', 'use the click, type and key tools'],
];

/** The allowed commands, as a message lists them. */
const ALLOWED_TEXT = `${WHOLE_DOMAINS.map((domain) => `${domain}.*`).join(', ')} (except DOM.setFileInputFiles and DOM.getFileInfo), and only ${[...ALLOWED_COMMANDS].join(', ')}`;

/**
 * Whether a DevTools command may be sent by an agent.
 * @param {unknown} method The command, such as `DOM.getDocument`.
 * @returns {{ allowed: true } | { allowed: false, reason: string }} The verdict; a refusal says why and what is allowed.
 */
function checkMethod(method) {
  if (typeof method !== 'string' || !/^[A-Za-z]+\.[A-Za-z0-9]+$/.test(method)) {
    return { allowed: false, reason: `give method as a protocol command such as DOM.getDocument. Allowed: ${ALLOWED_TEXT}.` };
  }
  const denied = DENIED.get(method) || REDIRECTED.get(method);
  if (denied) return { allowed: false, reason: `${method} is refused: ${denied}.` };
  if (ALLOWED_COMMANDS.has(method)) return { allowed: true };
  if (WHOLE_DOMAINS.includes(method.slice(0, method.indexOf('.')))) return { allowed: true };
  const named = REFUSED_PREFIXES.find(([prefix]) => method.startsWith(prefix));
  const why = named ? named[1] : 'it is not on the allowlist';
  return { allowed: false, reason: `${method} is refused: ${why}. Allowed: ${ALLOWED_TEXT}.` };
}

module.exports = { checkMethod, ALLOWED_TEXT };
