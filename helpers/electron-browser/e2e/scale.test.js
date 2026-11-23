'use strict';

/**
 * End-to-end tests of how a page renders at its pane's display scale together with the reader's zoom and an agent's
 * emulated viewport: the pixel ratio of its top frame and of a cross-origin frame, the size it lays out at, where
 * pointer presses land, and the size of what an adopted popup paints. With `PANDEMONIUM_E2E_SHOTS` set to a folder,
 * each painted picture checked is also written there as raw BGRA, named with its size, to look at.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { after, before, test } = require('node:test');
const { Helper, colours } = require('./harness');
const { startFixtures } = require('./fixtures');

/** @type {Helper} */
let helper;
/** @type {import('./fixtures').Fixtures} */
let fixtures;

before(async () => {
  fixtures = await startFixtures();
  helper = await Helper.start();
});

after(async () => {
  await helper?.stop();
  await fixtures?.close();
});

/**
 * Presses and releases the left button at a point of `page`, in logical pixels of its pane.
 * @param {number} page The page.
 * @param {number} x The point's left, in logical pixels.
 * @param {number} y The point's top, in logical pixels.
 * @returns {void}
 */
function click(page, x, y) {
  helper.send({ PointerMoved: { page, x, y, modifiers: 0, left: false } });
  helper.send({ PointerButton: { page, x, y, button: 'Left', pressed: true, clicks: 1, modifiers: 16 } });
  helper.send({ PointerButton: { page, x, y, button: 'Left', pressed: false, clicks: 1, modifiers: 0 } });
}

/**
 * Waits for the surface of `page` to have the size `width` by `height` and a painted picture in it with colour, and
 * writes it to `PANDEMONIUM_E2E_SHOTS` when that is set.
 * @param {number} page The page.
 * @param {number} width The width wanted, in device pixels.
 * @param {number} height The height wanted, in device pixels.
 * @param {string} name What the picture is called when written.
 * @returns {Promise<void>} Settles once it does.
 */
async function paintedAt(page, width, height, name) {
  const end = Date.now() + 10000;
  for (;;) {
    const picture = helper.painted(page) ? helper.picture(page) : { width: 0, height: 0, pixels: Buffer.alloc(0) };
    if (picture.width === width && picture.height === height && colours(picture.pixels) >= 3) {
      const folder = process.env.PANDEMONIUM_E2E_SHOTS;
      if (folder) fs.writeFileSync(path.join(folder, `${name}-${width}x${height}.bgra`), picture.pixels);
      return;
    }
    if (Date.now() > end) assert.fail(`page ${page} painted ${picture.width}x${picture.height} with ${colours(picture.pixels)} colours`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/**
 * Steps the reader's zoom of `page` in until it reports `factor`.
 * @param {number} page The page.
 * @param {number} factor The zoom factor wanted.
 * @returns {Promise<void>} Settles once the page reports it.
 */
async function zoomTo(page, factor) {
  for (let step = 0; step < 20; step++) {
    const from = helper.mark();
    helper.send({ Zoom: { page, action: 'In' } });
    const zoomed = await helper.waitReport('Zoomed', page, () => true, { from });
    if (Math.abs(zoomed.factor - factor) < 1e-6) return;
  }
  assert.fail(`page ${page} never zoomed to ${factor}`);
}

/**
 * Resets the reader's zoom of `page` and waits for it to report 100%.
 * @param {number} page The page.
 * @returns {Promise<void>} Settles once it does.
 */
async function resetZoom(page) {
  const from = helper.mark();
  helper.send({ Zoom: { page, action: 'Reset' } });
  await helper.waitReport('Zoomed', page, (b) => Math.abs(b.factor - 1) < 1e-6, { from });
}

/**
 * Runs `browser_emulate` on `page` and checks that it worked.
 * @param {number} page The page.
 * @param {Record<string, unknown>} args Its arguments.
 * @returns {Promise<void>} Settles once the viewport is set.
 */
async function emulate(page, args) {
  const result = await helper.tool(page, 'browser_emulate', args);
  assert.equal(result.ok, true, result.text);
}

test('at scale 2 the reader zoom reaches 500%, past what the engine zoom carries, and presses land', async () => {
  await helper.open(1, `${fixtures.main}/ratio`, { width: 400, height: 400, scale: 2 });
  await helper.until(1, 'window.cross.ratio', 2);
  await zoomTo(1, 5);
  await helper.until(1, '[devicePixelRatio, innerWidth]', [10, 80]);
  await helper.until(1, 'window.cross.width', 200);
  await helper.until(1, 'window.cross.ratio', 5);
  await paintedAt(1, 800, 800, 'zoom-500-scale-2');
  click(1, 100, 350);
  await helper.until(1, 'window.pressed', [20, 70]);
  click(1, 50, 100);
  await helper.until(1, 'window.cross.pressed', [10, 20]);
  await resetZoom(1);
  await helper.until(1, '[devicePixelRatio, innerWidth]', [2, 400]);
  await helper.until(1, '[window.cross.ratio, window.cross.width]', [2, 200]);
  helper.send({ Close: { page: 1 } });
});

test('a viewport with a pixel ratio of its own reaches cross-origin frames, also zoomed', async () => {
  await helper.open(2, `${fixtures.main}/ratio`, { width: 400, height: 400, scale: 2 });
  await emulate(2, { width: 300, height: 200, device_scale_factor: 3 });
  await helper.until(2, '[devicePixelRatio, innerWidth, innerHeight]', [3, 300, 200]);
  await helper.until(2, '[window.cross.ratio, window.cross.width]', [3, 200]);
  await paintedAt(2, 600, 400, 'ratio-3-scale-2');
  click(2, 50, 30);
  await helper.until(2, 'window.cross.pressed', [50, 30]);
  click(2, 250, 150);
  await helper.until(2, 'window.pressed', [250, 150]);
  await zoomTo(2, 2);
  await helper.until(2, '[Math.round(devicePixelRatio * 100) / 100, innerWidth]', [6, 150]);
  await helper.until(2, '[window.cross.ratio, window.cross.width]', [5, 200]);
  click(2, 250, 150);
  await helper.until(2, 'window.pressed', [125, 75]);
  helper.send({ Close: { page: 2 } });
});

test('a mobile viewport renders its top frame and cross-origin frames at its ratio and zooms', async () => {
  await helper.open(3, `${fixtures.main}/ratio`, { width: 400, height: 400, scale: 2 });
  await emulate(3, { width: 300, height: 200, device_scale_factor: 3, mobile: true });
  await helper.until(3, '[devicePixelRatio, innerWidth, innerHeight]', [3, 300, 200]);
  await helper.until(3, '[window.cross.ratio, window.cross.width]', [3, 200]);
  await paintedAt(3, 600, 400, 'mobile-3-scale-2');
  click(3, 250, 150);
  await helper.until(3, 'window.pressed', [250, 150]);
  click(3, 50, 30);
  await helper.until(3, 'window.cross.pressed', [50, 30]);
  await zoomTo(3, 1.5);
  await helper.until(3, '[devicePixelRatio, innerWidth]', [4.5, 200]);
  await helper.until(3, '[window.cross.ratio, window.cross.width]', [3, 200]);
  click(3, 150, 150);
  await helper.until(3, 'window.pressed', [100, 100]);
  click(3, 75, 45);
  await helper.until(3, 'window.cross.pressed', [50, 30]);
  await resetZoom(3);
  await emulate(3, { device: 'pixel-8' });
  await helper.until(3, '[devicePixelRatio, innerWidth, innerHeight]', [2.625, 412, 915]);
  await helper.until(3, '[window.cross.ratio, window.cross.width]', [2.625, 200]);
  await helper.until(3, "navigator.userAgent.includes('Pixel 8')", true);
  await paintedAt(3, 824, 1830, 'pixel-8-scale-2');
  click(3, 50, 30);
  await helper.until(3, 'window.cross.pressed', [50, 30]);
  await emulate(3, { reset: true });
  await helper.until(3, '[devicePixelRatio, innerWidth, innerHeight]', [2, 400, 400]);
  await helper.until(3, '[window.cross.ratio, window.cross.width]', [2, 200]);
  await paintedAt(3, 800, 800, 'mobile-reset-scale-2');
  click(3, 50, 30);
  await helper.until(3, 'window.cross.pressed', [50, 30]);
  helper.send({ Close: { page: 3 } });
});

test('an adopted popup renders at device size, also zoomed past the engine and with a viewport of its own ratio', async () => {
  await helper.open(4, `${fixtures.main}/ratio-opener`, { width: 400, height: 300, scale: 2 });
  await helper.readerTurn();
  const from = helper.mark();
  click(4, 100, 40);
  const opened = await helper.waitReport('PopupOpened', 4, () => true, { from });
  const adopt = helper.mark();
  helper.send({ AdoptPopup: { page: 5, popup: opened.popup, width: 300, height: 200, scale: 2 } });
  try {
    await helper.waitReport('Painted', 5, () => true, { from: adopt });
    await helper.until(5, '[devicePixelRatio, innerWidth, innerHeight]', [2, 300, 200]);
    await paintedAt(5, 600, 400, 'popup-scale-2');
    click(5, 250, 150);
    await helper.until(5, 'window.pressed', [250, 150]);
    click(5, 50, 30);
    await helper.until(5, 'window.cross.pressed', [50, 30]);
    await zoomTo(5, 5);
    await helper.until(5, '[Math.round(devicePixelRatio * 100) / 100, innerWidth]', [10, 60]);
    await paintedAt(5, 600, 400, 'popup-zoom-500-scale-2');
    click(5, 100, 50);
    await helper.until(5, 'window.cross.pressed', [20, 10]);
    await resetZoom(5);
    await helper.until(5, '[devicePixelRatio, innerWidth]', [2, 300]);
    await emulate(5, { width: 300, height: 200, device_scale_factor: 3, mobile: true });
    await helper.until(5, '[devicePixelRatio, innerWidth, innerHeight]', [3, 300, 200]);
    await helper.until(5, 'window.cross.width', 200);
    await paintedAt(5, 600, 400, 'popup-mobile-3-scale-2');
    click(5, 250, 150);
    await helper.until(5, 'window.pressed', [250, 150]);
    await emulate(5, { width: 200, height: 150, device_scale_factor: 1.5, mobile: false });
    await helper.until(5, '[devicePixelRatio, innerWidth, innerHeight]', [1.5, 200, 150]);
    await helper.until(5, '[window.cross.ratio, window.cross.width]', [1.5, 200]);
    await paintedAt(5, 400, 300, 'popup-ratio-1.5-scale-2');
    click(5, 150, 100);
    await helper.until(5, 'window.pressed', [150, 100]);
  } finally {
    helper.send({ Close: { page: 5 } });
    helper.send({ Close: { page: 4 } });
  }
});
