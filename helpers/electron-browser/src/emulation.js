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

/** The smallest and the largest zoom factor the engine takes; it holds any other to the nearer of them. */
const ZOOM_LIMITS = [0.25, 5];

/**
 * The zoom the reader chose for each page, by its contents, as {@link setReaderZoom} last set it.
 * @type {WeakMap<import('electron').WebContents, number>}
 */
const READER_ZOOMS = new WeakMap();

/**
 * How the engine renders a page: the zoom of its contents and the device metrics override beside it.
 * @typedef {object} Rendering
 * @property {number} zoom The contents' zoom factor, within {@link ZOOM_LIMITS}.
 * @property {Json | null} metrics The parameters of `Emulation.setDeviceMetricsOverride`, or nothing to clear it.
 */

/**
 * Works out how the engine renders a page in its window of device pixels, painted at a device scale of 1, so that its
 * top frame lays out at the logical size over the reader's zoom and has a pixel ratio of the viewport's ratio, or the
 * pane's, times that zoom.
 *
 * The contents' zoom carries as much of the pixel ratio as the engine takes ({@link ZOOM_LIMITS}): it is the only part
 * that reaches cross-origin frames, which run in processes of their own and render at the pixel ratio of their zoom,
 * while the override reaches the top frame alone. What the zoom cannot carry, above 500% or below 25%, goes to an
 * override that follows the window's size (width and height 0), with that rest as its pixel ratio and the layout's
 * share of it as its `scale`: the engine lays out at the window's size over `scale` times the zoom and reports a pixel
 * ratio of `deviceScaleFactor` times the zoom. With nothing left over and no viewport of an agent, there is no override.
 *
 * A fixed size is for a window without a view of its own, whose frames come at the override's size: the engine then
 * lays out at that size over the zoom (over 1 when mobile) and leaves `scale` aside, so the size is the device size over
 * the layout's share and `scale` is left out. Only with a pixel ratio of 1 in the override do its frames come at the
 * device size; otherwise the window's pictures are taken at the emulated ratio instead (`screencast.js`).
 *
 * A mobile viewport is the exception: the engine ignores the zoom in the top frame while it emulates a phone but still
 * zooms cross-origin frames by it, which then lay out too large. Its zoom stays 1 and the override carries all of it.
 * Cross-origin frames then render at the device scale of the window, so its page moves into a window painted at the
 * viewport's ratio ({@link engineScale}), `engine` here, sized to the device size over that ratio: the override gives
 * the layout size itself, over the reader's zoom, with `dontSetVisibleSize` so the engine keeps the window's size and
 * shrinks the layout into it by `scale`, the pane's scale times the reader's zoom over the window's.
 * @param {Emulated | null} viewport The viewport an agent set, or nothing when it follows the pane.
 * @param {number} paneScale The pane's display scale, device pixels per logical pixel; 1 when not above 0.
 * @param {number} [readerZoom] The zoom the reader chose; 1 when not given or not above 0.
 * @param {{ width: number, height: number } | null} [size] A fixed size for the override in device pixels, for a window
 *   the engine gives no size of its own; then there is always an override.
 * @param {number} [engine] The device scale the page's window is painted at; 1 when not given.
 * @returns {Rendering} The zoom and the override.
 */
function rendering(viewport, paneScale, readerZoom = 1, size = null, engine = 1) {
  const scale = paneScale > 0 ? paneScale : 1;
  const zoomed = readerZoom > 0 ? readerZoom : 1;
  const ratio = ((viewport && viewport.deviceScaleFactor) || scale) * zoomed;
  const mobile = Boolean(viewport && viewport.mobile);
  const zoom = mobile ? 1 : Math.min(Math.max(ratio, ZOOM_LIMITS[0]), ZOOM_LIMITS[1]);
  const deviceScaleFactor = ratio / zoom;
  const layout = (scale * zoomed) / zoom;
  const plain = Math.abs(deviceScaleFactor - 1) < 1e-6 && Math.abs(layout - 1) < 1e-6;
  const screen = viewport ? { screenWidth: viewport.width, screenHeight: viewport.height } : {};
  if (size) {
    return {
      zoom,
      metrics: {
        width: Math.max(1, Math.round(size.width / layout)),
        height: Math.max(1, Math.round(size.height / layout)),
        deviceScaleFactor,
        mobile,
        ...screen,
      },
    };
  }
  if (viewport && mobile && engine > 0 && engine !== 1) {
    return {
      zoom,
      metrics: {
        width: Math.max(1, Math.round(viewport.width / zoomed)),
        height: Math.max(1, Math.round(viewport.height / zoomed)),
        deviceScaleFactor,
        mobile,
        scale: layout / engine,
        dontSetVisibleSize: true,
        ...screen,
      },
    };
  }
  if (!viewport && plain) return { zoom, metrics: null };
  return { zoom, metrics: { width: 0, height: 0, deviceScaleFactor, mobile, scale: layout, ...screen } };
}

/**
 * The device scale a page's window is to be painted at for `viewport`: the pixel ratio of a mobile viewport, or of the
 * pane when it gives none, so its cross-origin frames render at it ({@link rendering}); 1 for any other, whose ratio
 * reaches cross-origin frames through the contents' zoom.
 * @param {Emulated | null} viewport The viewport an agent set, or nothing.
 * @param {number} paneScale The pane's display scale, device pixels per logical pixel.
 * @returns {number} The device scale.
 */
function engineScale(viewport, paneScale) {
  if (!viewport || !viewport.mobile) return 1;
  return viewport.deviceScaleFactor || (paneScale > 0 ? paneScale : 1);
}

/**
 * Records the zoom the reader chose for a page, for what turns the page's CSS pixels into the window's.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {number} factor The reader's zoom factor.
 * @returns {void}
 */
function setReaderZoom(contents, factor) {
  READER_ZOOMS.set(contents, factor);
}

/**
 * The zoom the reader chose for a page: the window's pixels per CSS pixel of its top frame over the pane's scale.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {number} The reader's zoom factor; 1 for a page that recorded none.
 */
function readerZoom(contents) {
  return READER_ZOOMS.get(contents) || 1;
}

/**
 * Sends how a page renders to the engine ({@link rendering}), with touch events when the viewport is mobile; the
 * contents' zoom is the caller's to set. A window paints offscreen at a device scale of 1, or a mobile viewport's
 * ratio ({@link engineScale}), and is sized by its page (`Page#fitWindow`): the pane's, or the viewport's, size times
 * the pane's display scale, over that device scale.
 * @param {import('./cdp').Cdp} cdp The page's protocol.
 * @param {Emulated | null} viewport What to emulate, or nothing to follow the pane.
 * @param {number} [paneScale] The pane's display scale, device pixels per logical pixel; 1 when not given or not above 0.
 * @param {number} [zoom] The zoom the reader chose; 1 when not given.
 * @param {number} [engine] The device scale the page's window is painted at; 1 when not given.
 * @returns {Promise<void>} Resolves when the engine has taken it.
 */
async function applyViewport(cdp, viewport, paneScale = 1, zoom = 1, engine = 1) {
  const { metrics } = rendering(viewport, paneScale, zoom, null, engine);
  if (metrics) await cdp.call('Emulation.setDeviceMetricsOverride', metrics);
  else await cdp.call('Emulation.clearDeviceMetricsOverride', {});
  await cdp.call('Emulation.setTouchEmulationEnabled', viewport && viewport.mobile ? { enabled: true, maxTouchPoints: 5 } : { enabled: false });
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
  ZOOM_LIMITS,
  rendering,
  engineScale,
  setReaderZoom,
  readerZoom,
  applyViewport,
  paneMetrics,
  applyUserAgent,
  describe,
};
