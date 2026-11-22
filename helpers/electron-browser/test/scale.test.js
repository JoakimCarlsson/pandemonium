'use strict';

/**
 * Tests of how a page follows its pane's display scale, run with `node --test test/` in the helper's folder: windows
 * sized in device pixels, the zoom or emulation that renders at the pane's scale in them, and points sent in their pixels.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { applyViewport, paneMetrics, readerZoom, scaledByZoom, screenMetrics, setZoomBase, viewportMetrics } = require('../src/emulation');
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

test('a viewport an agent set follows its window, renders at the pane scale and sets the screen', () => {
  assert.deepEqual(viewportMetrics({ width: 375, height: 667, deviceScaleFactor: 0, mobile: false }, 2), {
    width: 0,
    height: 0,
    deviceScaleFactor: 2,
    mobile: false,
    scale: 2,
    screenWidth: 375,
    screenHeight: 667,
  });
  assert.equal(viewportMetrics({ width: 393, height: 852, deviceScaleFactor: 3, mobile: true }, 1.5).deviceScaleFactor, 3);
});

test('giving the viewport back to the pane turns touch off and clears the override, since the zoom carries the scale', async () => {
  const cdp = recorder();
  await applyViewport(/** @type {any} */ (cdp), null, 2);
  assert.deepEqual(cdp.sent, [
    ['Emulation.setTouchEmulationEnabled', { enabled: false }],
    ['Emulation.clearDeviceMetricsOverride', {}],
  ]);
});

test('the zoom carries the pane scale unless a viewport is mobile or has a pixel ratio of its own', () => {
  assert.equal(scaledByZoom(null), true);
  assert.equal(scaledByZoom({ width: 400, height: 300, deviceScaleFactor: 0, mobile: false }), true);
  assert.equal(scaledByZoom({ width: 400, height: 300, deviceScaleFactor: 3, mobile: false }), false);
  assert.equal(scaledByZoom({ width: 400, height: 300, deviceScaleFactor: 0, mobile: true }), false);
});

test('a viewport the zoom carries sets only the screen, and one it does not keeps the emulated ratio', async () => {
  const plain = { width: 400, height: 300, deviceScaleFactor: 0, mobile: false };
  const zoomed = recorder();
  await applyViewport(/** @type {any} */ (zoomed), plain, 2);
  assert.deepEqual(zoomed.sent[0], ['Emulation.setDeviceMetricsOverride', screenMetrics(plain)]);
  assert.deepEqual(screenMetrics(plain), { width: 0, height: 0, deviceScaleFactor: 0, mobile: false, screenWidth: 400, screenHeight: 300 });
  const phone = { width: 375, height: 667, deviceScaleFactor: 2, mobile: true };
  const emulated = recorder();
  await applyViewport(/** @type {any} */ (emulated), phone, 2);
  assert.deepEqual(emulated.sent[0], ['Emulation.setDeviceMetricsOverride', viewportMetrics(phone, 2)]);
});

test('the reader zoom is the contents zoom without the part that carries the scale', () => {
  const contents = /** @type {any} */ ({ getZoomFactor: () => 2.2 });
  assert.equal(readerZoom(contents), 2.2);
  setZoomBase(contents, 2);
  assert.ok(Math.abs(readerZoom(contents) - 1.1) < 1e-9);
});

test('points the window gives in logical pixels are sent in the page window pixels', () => {
  assert.deepEqual(toWindow(50, 20, 1), { x: 50, y: 20 });
  assert.deepEqual(toWindow(50, 20, 2), { x: 100, y: 40 });
  assert.deepEqual(toWindow(10.5, 3, 1.5), { x: 16, y: 5 });
});
