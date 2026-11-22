'use strict';

/**
 * Device emulation for agents: presets, the state that `browser_emulate` and `browser_resize` keep
 * on a page, and how that state is sent to the engine.
 */

/** @typedef {import('./types').Json} Json */

/**
 * What is emulated.
 * @typedef {object} Emulated
 * @property {number} width The viewport width in CSS pixels.
 * @property {number} height The viewport height in CSS pixels.
 * @property {number} deviceScaleFactor Device pixels per CSS pixel; 0 follows the pane.
 * @property {boolean} mobile Whether the page is shown as on a phone: meta viewport honoured and touch events present.
 */

/**
 * What is emulated on a page: the viewport and the user agent.
 * @typedef {object} State
 * @property {Emulated | null} viewport The viewport, or nothing when it follows the pane.
 * @property {string | null} userAgent The user agent, or nothing for the engine's own.
 */

/** The smallest and the largest a viewport side may be, in pixels. */
const VIEWPORT_LIMITS = [100, 4000];

/** The largest device pixel ratio accepted. */
const MOST_RATIO = 5;

/** The user agent of an iPhone. */
const IPHONE_AGENT = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1';

/** The user agent of an iPad. */
const IPAD_AGENT = 'Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1';

/** The user agent of a Pixel. */
const PIXEL_AGENT = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36';

/**
 * The device presets, by name.
 * @type {Record<string, { viewport: Emulated, userAgent: string | null }>}
 */
const PRESETS = {
  'iphone-se': { viewport: { width: 375, height: 667, deviceScaleFactor: 2, mobile: true }, userAgent: IPHONE_AGENT },
  'iphone-15': { viewport: { width: 393, height: 852, deviceScaleFactor: 3, mobile: true }, userAgent: IPHONE_AGENT },
  ipad: { viewport: { width: 820, height: 1180, deviceScaleFactor: 2, mobile: true }, userAgent: IPAD_AGENT },
  'pixel-8': { viewport: { width: 412, height: 915, deviceScaleFactor: 2.625, mobile: true }, userAgent: PIXEL_AGENT },
  'desktop-1280': { viewport: { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false }, userAgent: null },
  'desktop-1440': { viewport: { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false }, userAgent: null },
  'desktop-1920': { viewport: { width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false }, userAgent: null },
};

/** Other names for the presets. */
const ALIASES = /** @type {Record<string, string>} */ ({ iphone: 'iphone-15', pixel: 'pixel-8', desktop: 'desktop-1440' });

/** The names that work for `device`, for messages. */
const DEVICE_NAMES = [...Object.keys(PRESETS), ...Object.keys(ALIASES)].join(', ');

/**
 * Whether `side` is a viewport side that is allowed.
 * @param {unknown} side A value.
 * @returns {side is number} Whether it is.
 */
function validSide(side) {
  return Number.isInteger(side) && /** @type {number} */ (side) >= VIEWPORT_LIMITS[0] && /** @type {number} */ (side) <= VIEWPORT_LIMITS[1];
}

/**
 * Works out what to emulate from what an agent asked for and what is emulated now.
 * A `device` replaces everything, the user agent too; the other fields change what is there.
 * @param {Json} args `device`, `width`, `height`, `device_scale_factor`, `mobile` and `user_agent`.
 * @param {State} current What is emulated now.
 * @param {{ width: number, height: number }} pane The size the page has now, used when only a ratio or `mobile` is given.
 * @returns {State | { error: string }} What to emulate, or why not.
 */
function resolve(args, current, pane) {
  let { viewport, userAgent } = current;
  if (args.device !== undefined) {
    const name = typeof args.device === 'string' ? args.device.toLowerCase() : '';
    const preset = PRESETS[ALIASES[name] || name];
    if (!preset) return { error: `unknown device; use one of ${DEVICE_NAMES}` };
    viewport = { ...preset.viewport };
    userAgent = preset.userAgent;
  }
  if (['width', 'height', 'device_scale_factor', 'mobile'].some((name) => args[name] !== undefined)) {
    viewport = viewport ? { ...viewport } : { width: pane.width, height: pane.height, deviceScaleFactor: 0, mobile: false };
    const width = args.width === undefined ? viewport.width : args.width;
    const height = args.height === undefined ? viewport.height : args.height;
    if (!validSide(width) || !validSide(height)) {
      return { error: `give width and height as whole numbers from ${VIEWPORT_LIMITS[0]} to ${VIEWPORT_LIMITS[1]}` };
    }
    viewport.width = width;
    viewport.height = height;
    if (args.device_scale_factor !== undefined) {
      const ratio = args.device_scale_factor;
      if (typeof ratio !== 'number' || !(ratio > 0) || ratio > MOST_RATIO) {
        return { error: `give device_scale_factor as a number above 0 and up to ${MOST_RATIO}` };
      }
      viewport.deviceScaleFactor = ratio;
    }
    if (args.mobile !== undefined) {
      if (typeof args.mobile !== 'boolean') return { error: 'give mobile as true or false' };
      viewport.mobile = args.mobile;
    }
  }
  if (args.user_agent !== undefined) {
    if (typeof args.user_agent !== 'string' || args.user_agent.length > 500) return { error: 'give user_agent as text of at most 500 characters' };
    userAgent = args.user_agent === '' ? null : args.user_agent;
  }
  return { viewport, userAgent };
}

/**
 * The zoom each page's contents carry for the pane's display scale, by the contents, as {@link setZoomBase} last set it.
 * @type {WeakMap<import('electron').WebContents, number>}
 */
const ZOOM_BASES = new WeakMap();

/**
 * Whether a page renders at its pane's display scale by its zoom rather than by an emulated pixel ratio: when its
 * viewport follows the pane, or an agent set one that is not mobile and has no pixel ratio of its own. The zoom reaches
 * the page's cross-origin frames, which run in processes of their own and render at the pixel ratio of their zoom; an
 * emulated pixel ratio reaches the top frame alone and leaves them blurry.
 * @param {Emulated | null} viewport The viewport an agent set, or nothing.
 * @returns {boolean} Whether the zoom carries the scale.
 */
function scaledByZoom(viewport) {
  return !viewport || (!viewport.mobile && !viewport.deviceScaleFactor);
}

/**
 * Records the zoom a page's contents carry for the pane's display scale, so the reader's own zoom can be told apart.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {number} base The zoom for the scale: the scale, or 1 when an emulated pixel ratio carries it.
 * @returns {void}
 */
function setZoomBase(contents, base) {
  ZOOM_BASES.set(contents, base);
}

/**
 * The zoom the reader chose for a page: its contents' zoom without the part that carries the pane's display scale.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {number} The reader's zoom factor.
 */
function readerZoom(contents) {
  return contents.getZoomFactor() / (ZOOM_BASES.get(contents) || 1);
}

/**
 * Sends a viewport to the engine, with touch events when it is mobile; with none, gives the viewport back to the pane.
 * Every window paints offscreen at a device scale of 1 and is sized in device pixels by its page (`Page#fitWindow`): the
 * pane's, or the viewport's, size times the pane's display scale. When {@link scaledByZoom} holds, the page's zoom
 * carries the scale, so the override is cleared, or for a viewport of its own sets only the screen's size. Otherwise
 * the override keeps following the window's size (width and height 0), carries the viewport's pixel ratio, or the
 * pane's, and the pane's scale as its `scale`, so the page lays out at the logical size and paints a frame of device
 * pixels; an override of a fixed size would paint it at 1.
 * @param {import('./cdp').Cdp} cdp The page's protocol.
 * @param {Emulated | null} viewport What to emulate.
 * @param {number} [paneScale] The pane's display scale, device pixels per logical pixel; 1 when not given or not above 0.
 * @returns {Promise<void>} Resolves when the engine has taken it.
 */
async function applyViewport(cdp, viewport, paneScale = 1) {
  const scale = paneScale > 0 ? paneScale : 1;
  if (!viewport) {
    await cdp.call('Emulation.setTouchEmulationEnabled', { enabled: false });
    await cdp.call('Emulation.clearDeviceMetricsOverride', {});
    return;
  }
  await cdp.call('Emulation.setDeviceMetricsOverride', scaledByZoom(viewport) ? screenMetrics(viewport) : viewportMetrics(viewport, scale));
  await cdp.call('Emulation.setTouchEmulationEnabled', viewport.mobile ? { enabled: true, maxTouchPoints: 5 } : { enabled: false });
}

/**
 * The override for a viewport an agent set whose page renders at the pane's scale by its zoom: the window's size and
 * pixel ratio stay the engine's, and only the screen takes the viewport's size.
 * @param {Emulated} viewport The viewport.
 * @returns {Json} The parameters of `Emulation.setDeviceMetricsOverride`.
 */
function screenMetrics(viewport) {
  return { width: 0, height: 0, deviceScaleFactor: 0, mobile: false, screenWidth: viewport.width, screenHeight: viewport.height };
}

/**
 * The override that emulates a viewport an agent set, in a window its page sized to it in device pixels.
 * @param {Emulated} viewport The viewport.
 * @param {number} scale The pane's display scale, device pixels per logical pixel.
 * @returns {Json} The parameters of `Emulation.setDeviceMetricsOverride`.
 */
function viewportMetrics(viewport, scale) {
  const { width, height, mobile } = viewport;
  return {
    width: 0,
    height: 0,
    deviceScaleFactor: viewport.deviceScaleFactor || scale,
    mobile,
    scale,
    screenWidth: width,
    screenHeight: height,
  };
}

/**
 * The protocol command that makes a page without a zoom of its own to carry the scale, such as the developer tools or
 * a sealed page, render at the pane's display scale in a window sized in device pixels: an override at that ratio, or
 * none at a scale of 1.
 * @param {number} scale The pane's display scale, device pixels per logical pixel.
 * @returns {[string, Json]} The command and its parameters.
 */
function paneMetrics(scale) {
  if (scale === 1) return ['Emulation.clearDeviceMetricsOverride', {}];
  return ['Emulation.setDeviceMetricsOverride', { width: 0, height: 0, deviceScaleFactor: scale, mobile: false, scale }];
}

/**
 * Sends a user agent to the engine; an empty one gives back the engine's own.
 * @param {import('./cdp').Cdp} cdp The page's protocol.
 * @param {string | null} userAgent The user agent.
 * @returns {Promise<void>} Resolves when the engine has taken it.
 */
async function applyUserAgent(cdp, userAgent) {
  await cdp.call('Emulation.setUserAgentOverride', { userAgent: userAgent || '' });
}

/**
 * Says what is emulated.
 * @param {State} state What is emulated.
 * @returns {string} A sentence.
 */
function describe({ viewport, userAgent }) {
  const view = viewport
    ? `viewport ${viewport.width} by ${viewport.height}, pixel ratio ${viewport.deviceScaleFactor || 'of the pane'}, ${viewport.mobile ? 'mobile' : 'not mobile'}`
    : 'viewport follows the pane';
  return `${view}; user agent ${userAgent ? `"${userAgent}"` : "is the browser's own"}`;
}

module.exports = {
  VIEWPORT_LIMITS,
  PRESETS,
  resolve,
  scaledByZoom,
  setZoomBase,
  readerZoom,
  applyViewport,
  paneMetrics,
  viewportMetrics,
  screenMetrics,
  applyUserAgent,
  describe,
};
