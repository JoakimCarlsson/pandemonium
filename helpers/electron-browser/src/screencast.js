'use strict';

/**
 * Painting for a window a page opened with `window.open`. Electron gives such a window of an offscreen page no
 * offscreen view, whatever options it is made with: it lays out at 0 by 0 and never sends `paint`, and sizing, showing
 * or invalidating the window changes nothing. The engine still renders it when the protocol fixes its size, and its
 * screencast hands over each frame, so the page is sized and painted through the protocol instead.
 *
 * The screencast's frames come at the override's size whatever pixel ratio it emulates. When the override has the device
 * size and a ratio of 1, as for a window that follows its pane while the zoom carries the scale, each frame is painted
 * as it comes. Otherwise, for a mobile viewport, one with a pixel ratio of its own or a zoom past the
 * engine's, a frame only says that the page changed: a picture is taken at the emulated ratio, brought to the device
 * size, and painted instead.
 */

const { nativeImage } = require('electron');
const emulation = require('./emulation');

/** @typedef {import('./types').Json} Json */

/**
 * The override that gives a window without a view of its own its size and rendering ({@link emulation.rendering} with
 * the device size fixed).
 * @param {{ width: number, height: number }} device The size in device pixels.
 * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
 * @param {number} scale The pane's display scale, device pixels per logical pixel.
 * @param {number} [zoom] The zoom the reader chose; 1 when not given.
 * @returns {Json} The parameters of `Emulation.setDeviceMetricsOverride`.
 */
function frameMetrics(device, viewport, scale, zoom = 1) {
  return /** @type {Json} */ (emulation.rendering(viewport, scale, zoom, device).metrics);
}

/**
 * Whether the frames of a window with this override come at the device size and its pixel ratio, so they are painted
 * as they come: the override has the device size and a pixel ratio of 1.
 * @param {Json} metrics The override.
 * @param {{ width: number, height: number }} device The size in device pixels.
 * @returns {boolean} Whether they do.
 */
function framesAtDeviceSize(metrics, device) {
  return Math.abs(Number(metrics.deviceScaleFactor) - 1) < 1e-6 && metrics.width === device.width && metrics.height === device.height;
}

/**
 * Sizes a window without a view of its own through its protocol and hands each picture of it to `paint`.
 */
class Screencast {
  /**
   * Listens for the frames of the page behind `cdp`; nothing is rendered until {@link Screencast#fit}.
   * @param {import('./cdp').Cdp} cdp The page's protocol.
   * @param {(image: import('electron').NativeImage) => void} paint Takes each picture, whole.
   */
  constructor(cdp, paint) {
    /** @type {import('./cdp').Cdp} The page's protocol. */
    this.cdp = cdp;
    /** @type {(image: import('electron').NativeImage) => void} Takes each picture. */
    this.paint = paint;
    /** @type {{ width: number, height: number }} The size pictures are painted at, in device pixels. */
    this.device = { width: 0, height: 0 };
    /** @type {boolean} Whether a picture is taken for each frame rather than the frame painted. */
    this.taking = false;
    /** @type {boolean} Whether a picture is being taken. */
    this.busy = false;
    /** @type {boolean} Whether the page changed while a picture was being taken, so another is due. */
    this.again = false;
    /** @type {string} The last frame, as the protocol sent it, to tell a frame that shows a change from one a picture caused. */
    this.last = '';
    /** @type {Promise<void>} The override and picture under way, which the next waits for: a picture taken while an override is emulated puts that override back when it is done, so one sent meanwhile would be lost. */
    this.chain = Promise.resolve();
    /** @type {boolean} Whether the screencast was stopped for good. */
    this.disposed = false;
    /** @type {import('./cdp').EventListener} Hears the frames. */
    this.listener = (method, params, session) => {
      if (method === 'Page.screencastFrame' && !session) this.frame(/** @type {{ data: string, sessionId: number }} */ (params));
    };
    cdp.listen(this.listener);
  }

  /**
   * Takes one frame: acknowledges it, so the engine sends the next, and paints it, or, when frames do not come at the
   * device size, takes a picture at the emulated ratio if the frame differs from the last: taking a picture makes the
   * engine send a frame again, which would otherwise take pictures without end.
   * @param {{ data: string, sessionId: number }} params The frame, as the protocol sent it.
   * @returns {void}
   */
  frame({ data, sessionId }) {
    this.cdp.call('Page.screencastFrameAck', { sessionId }).catch(() => {});
    if (this.taking) {
      if (data !== this.last) this.take();
      this.last = data;
      return;
    }
    const image = nativeImage.createFromBuffer(Buffer.from(data, 'base64'));
    if (!image.isEmpty()) this.paint(image);
  }

  /**
   * Takes a picture of what the page shows at its emulated pixel ratio and paints it at the device size; while one is
   * being taken, another follows it once it is done.
   * @returns {Promise<void>} Settles once painted, or once it was clear no picture came.
   */
  async take() {
    if (this.busy) {
      this.again = true;
      return;
    }
    this.busy = true;
    try {
      await this.inTurn(async () => {
        const reply = /** @type {{ data?: string } | null} */ (await this.cdp.call('Page.captureScreenshot', { format: 'png' }).catch(() => null));
        if (!reply || !reply.data || this.disposed || !this.taking) return;
        this.paint(this.atDeviceSize(nativeImage.createFromBuffer(Buffer.from(reply.data, 'base64'))));
      });
    } finally {
      this.busy = false;
    }
    if (this.again) {
      this.again = false;
      await this.take();
    }
  }

  /**
   * Runs `work` once the override or picture before it is done, so no two of them overlap.
   * @param {() => Promise<void>} work What to run.
   * @returns {Promise<void>} Settles as `work` does.
   */
  inTurn(work) {
    const done = this.chain.then(work, work);
    this.chain = done.catch(() => {});
    return done;
  }

  /**
   * A picture brought to the device size, when the emulated ratio differs from the pane's.
   * @param {import('electron').NativeImage} image The picture.
   * @returns {import('electron').NativeImage} It at the device size.
   */
  atDeviceSize(image) {
    const { width, height } = image.getSize();
    if (image.isEmpty() || (width === this.device.width && height === this.device.height)) return image;
    return image.resize({ width: this.device.width, height: this.device.height, quality: 'best' });
  }

  /**
   * Gives the page its size and its touch emulation, and starts the screencast, or starts it again after a navigation
   * that may have stopped it; the engine refuses to start one that runs already, which is not an error here. It waits
   * for a picture under way ({@link Screencast#inTurn}).
   * @param {{ width: number, height: number }} device The size in device pixels.
   * @param {import('./emulation').Emulated | null} viewport The viewport an agent set, or nothing.
   * @param {number} scale The pane's display scale, device pixels per logical pixel.
   * @param {number} [zoom] The zoom the reader chose; 1 when not given.
   * @returns {Promise<void>} Resolves when the engine has taken it; rejects with the protocol's error.
   */
  async fit(device, viewport, scale, zoom = 1) {
    const metrics = frameMetrics(device, viewport, scale, zoom);
    await this.inTurn(async () => {
      this.device = device;
      this.taking = !framesAtDeviceSize(metrics, device);
      this.last = '';
      await this.cdp.call('Emulation.setDeviceMetricsOverride', metrics);
      await this.cdp.call(
        'Emulation.setTouchEmulationEnabled',
        viewport && viewport.mobile ? { enabled: true, maxTouchPoints: 5 } : { enabled: false },
      );
      await this.cdp.call('Page.startScreencast', { format: 'png', everyNthFrame: 1 }).catch(() => {});
    });
    if (this.taking) await this.take();
  }

  /**
   * Stops hearing the frames.
   * @returns {void}
   */
  dispose() {
    this.disposed = true;
    this.cdp.unlisten(this.listener);
    this.cdp.call('Page.stopScreencast', {}).catch(() => {});
  }
}

module.exports = { Screencast, frameMetrics, framesAtDeviceSize };
