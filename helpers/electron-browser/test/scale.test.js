'use strict';

/**
 * Tests of how a page follows its pane's display scale, run with `node --test test/` in the helper's folder: windows
 * sized in device pixels, the zoom or emulation that renders at the pane's scale in them, and points sent in their pixels.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { ZOOM_LIMITS, applyViewport, engineScale, paneMetrics, readerZoom, rendering, setReaderZoom } = require('../src/emulation');
const { frameMetrics, framesAtDeviceSize } = require('../src/screencast');
const { toWindow } = require('../src/input');
const { ENGINE_SCALE, deviceSide } = require('../src/surface');

/**
 * A stand-in for a page's protocol that keeps every command sent to it.
 * @returns {{ sent: [string, unknown][], call: (method: string, params?: unknown) => Promise<object> }} The stand-in.
 */
function recorder() {
  /** @type {[string, unknown][]} */
  const sent = [];
  return {
    sent,
    call(method, params = {}) {
      sent.push([method, params]);
      return Promise.resolve({});
    },
  };
}

test('every window paints at a device scale of 1', () => {
  assert.equal(ENGINE_SCALE, 1);
});

test('a side of a pane is its logical length times the scale, rounded and at least 1', () => {
  assert.equal(deviceSide(300, 1), 300);
  assert.equal(deviceSide(300, 2), 600);
  assert.equal(deviceSide(301, 1.5), 452);
  assert.equal(deviceSide(0, 2), 1);
});

test('a pane at scale 1 clears the override and one above follows the window at that scale', () => {
  assert.deepEqual(paneMetrics(1), ['Emulation.clearDeviceMetricsOverride', {}]);
  assert.deepEqual(paneMetrics(2), [
    'Emulation.setDeviceMetricsOverride',
    { width: 0, height: 0, deviceScaleFactor: 2, mobile: false, scale: 2 },
  ]);
});

/**
 * Whether two numbers are equal up to rounding.
 * @param {unknown} actual The number found.
 * @param {number} expected The number wanted.
 * @returns {void}
 */
function near(actual, expected) {
  assert.ok(Math.abs(Number(actual) - expected) < 1e-9, `${actual} is not ${expected}`);
}

test('a page that follows its pane renders by its zoom alone while the engine takes it, with no override', () => {
  assert.deepEqual(rendering(null, 1), { zoom: 1, metrics: null });
  assert.deepEqual(rendering(null, 2), { zoom: 2, metrics: null });
  assert.deepEqual(rendering(null, 2, 2.5), { zoom: 5, metrics: null });
  assert.deepEqual(ZOOM_LIMITS, [0.25, 5]);
});

test('a zoom past the engine limit leaves the rest to an override that follows the window', () => {
  const { zoom, metrics } = rendering(null, 2, 5);
  assert.equal(zoom, 5);
  assert.deepEqual(metrics, { width: 0, height: 0, deviceScaleFactor: 2, mobile: false, scale: 2 });
  const small = rendering(null, 1, 0.2);
  assert.equal(small.zoom, 0.25);
  near(small.metrics?.deviceScaleFactor, 0.8);
  near(small.metrics?.scale, 0.8);
});

test('a viewport with a pixel ratio of its own carries it by the zoom, so cross-origin frames get it too', () => {
  const viewport = { width: 300, height: 200, deviceScaleFactor: 3, mobile: false };
  const { zoom, metrics } = rendering(viewport, 2);
  assert.equal(zoom, 3);
  assert.equal(metrics?.deviceScaleFactor, 1);
  near(metrics?.scale, 2 / 3);
  assert.deepEqual([metrics?.width, metrics?.height, metrics?.screenWidth, metrics?.screenHeight], [0, 0, 300, 200]);
  const zoomed = rendering(viewport, 2, 2);
  assert.equal(zoomed.zoom, 5);
  near(zoomed.metrics?.deviceScaleFactor, 1.2);
  near(zoomed.metrics?.scale, 0.8);
});

test('a viewport that follows the pane ratio sets only the screen beside the zoom', () => {
  const { zoom, metrics } = rendering({ width: 400, height: 300, deviceScaleFactor: 0, mobile: false }, 2);
  assert.equal(zoom, 2);
  assert.deepEqual(metrics, { width: 0, height: 0, deviceScaleFactor: 1, mobile: false, scale: 1, screenWidth: 400, screenHeight: 300 });
});

test('a mobile viewport keeps a zoom of 1 and carries its ratio and the reader zoom in the override', () => {
  const phone = { width: 375, height: 667, deviceScaleFactor: 3, mobile: true };
  assert.deepEqual(rendering(phone, 2), {
    zoom: 1,
    metrics: { width: 0, height: 0, deviceScaleFactor: 3, mobile: true, scale: 2, screenWidth: 375, screenHeight: 667 },
  });
  const zoomed = rendering(phone, 2, 1.5);
  assert.equal(zoomed.zoom, 1);
  near(zoomed.metrics?.deviceScaleFactor, 4.5);
  near(zoomed.metrics?.scale, 3);
});

test('a mobile viewport is painted at its ratio, or the pane scale without one, and any other at 1', () => {
  assert.equal(engineScale(null, 2), 1);
  assert.equal(engineScale({ width: 300, height: 200, deviceScaleFactor: 3, mobile: false }, 2), 1);
  assert.equal(engineScale({ width: 300, height: 200, deviceScaleFactor: 2.625, mobile: true }, 2), 2.625);
  assert.equal(engineScale({ width: 300, height: 200, deviceScaleFactor: 0, mobile: true }, 1.5), 1.5);
});

test('in a window painted at its ratio a mobile viewport gives its layout size and shrinks it into the window', () => {
  const phone = { width: 300, height: 200, deviceScaleFactor: 3, mobile: true };
  const { zoom, metrics } = rendering(phone, 2, 1, null, 3);
  assert.equal(zoom, 1);
  near(metrics?.scale, 2 / 3);
  assert.deepEqual({ ...metrics, scale: 0 }, {
    width: 300,
    height: 200,
    deviceScaleFactor: 3,
    mobile: true,
    scale: 0,
    dontSetVisibleSize: true,
    screenWidth: 300,
    screenHeight: 200,
  });
  const zoomed = rendering(phone, 2, 1.5, null, 3).metrics;
  assert.equal(zoomed?.width, 200);
  near(zoomed?.deviceScaleFactor, 4.5);
  near(zoomed?.scale, 1);
});

test('a window drawn by its screencast gets a fixed size whose frames are the device size only at a ratio of 1', () => {
  const device = { width: 600, height: 400 };
  const plain = frameMetrics(device, null, 2);
  assert.deepEqual(plain, { width: 600, height: 400, deviceScaleFactor: 1, mobile: false });
  assert.equal(framesAtDeviceSize(plain, device), true);
  const phone = frameMetrics(device, { width: 300, height: 200, deviceScaleFactor: 3, mobile: true }, 2);
  assert.deepEqual(phone, { width: 300, height: 200, deviceScaleFactor: 3, mobile: true, screenWidth: 300, screenHeight: 200 });
  assert.equal(framesAtDeviceSize(phone, device), false);
  const zoomed = frameMetrics(device, null, 2, 5);
  assert.deepEqual(zoomed, { width: 300, height: 200, deviceScaleFactor: 2, mobile: false });
  assert.equal(framesAtDeviceSize(zoomed, device), false);
});

test('giving the viewport back to the pane clears the override and turns touch off', async () => {
  const cdp = recorder();
  await applyViewport(/** @type {any} */ (cdp), null, 2);
  assert.deepEqual(cdp.sent, [
    ['Emulation.clearDeviceMetricsOverride', {}],
    ['Emulation.setTouchEmulationEnabled', { enabled: false }],
  ]);
});

test('a viewport is sent as its rendering gives it, with touch when mobile', async () => {
  const phone = { width: 375, height: 667, deviceScaleFactor: 2, mobile: true };
  const cdp = recorder();
  await applyViewport(/** @type {any} */ (cdp), phone, 2, 1.25);
  assert.deepEqual(cdp.sent, [
    ['Emulation.setDeviceMetricsOverride', rendering(phone, 2, 1.25).metrics],
    ['Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 }],
  ]);
});

test('the reader zoom is the one recorded for the contents, 1 before any', () => {
  const contents = /** @type {any} */ ({ getZoomFactor: () => 5 });
  assert.equal(readerZoom(contents), 1);
  setReaderZoom(contents, 5);
  assert.equal(readerZoom(contents), 5);
});

test('points the window gives in logical pixels are sent in the page window pixels', () => {
  assert.deepEqual(toWindow(50, 20, 1), { x: 50, y: 20 });
  assert.deepEqual(toWindow(50, 20, 2), { x: 100, y: 40 });
  assert.deepEqual(toWindow(10.5, 3, 1.5), { x: 16, y: 5 });
});
