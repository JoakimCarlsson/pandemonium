'use strict';

/**
 * End-to-end tests of what a page paints: its surface at display scales 1 and 2, and after resizing and rescaling.
 */

const assert = require('node:assert/strict');
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
 * Waits for the surface of `page` to have the size `width` by `height` and a painted picture in it with colour.
 * @param {number} page The page.
 * @param {number} width The width wanted, in device pixels.
 * @param {number} height The height wanted, in device pixels.
 * @param {number} from The first report to look at.
 * @returns {Promise<void>} Settles once it does.
 */
async function paintedAt(page, width, height, from) {
  await helper.waitReport('Surface', page, (b) => b.width === width && b.height === height, { from });
  const end = Date.now() + 10000;
  for (;;) {
    const picture = helper.painted(page) ? helper.picture(page) : { width: 0, height: 0, pixels: Buffer.alloc(0) };
    if (picture.width === width && picture.height === height && colours(picture.pixels) >= 3) return;
    if (Date.now() > end) assert.fail(`page ${page} painted ${picture.width}x${picture.height} with ${colours(picture.pixels)} colours`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

test('a page opened at scale 1 paints its logical size with content', async () => {
  const from = helper.mark();
  await helper.open(1, `${fixtures.main}/paint`, { width: 400, height: 300, scale: 1 });
  await paintedAt(1, 400, 300, from);
  assert.equal(await helper.evaluate(1, 'devicePixelRatio'), '1');
});

test('a page opened at scale 2 paints twice its size and reports a pixel ratio of 2', async () => {
  const from = helper.mark();
  await helper.open(2, `${fixtures.main}/paint`, { width: 400, height: 300, scale: 2 });
  await paintedAt(2, 800, 600, from);
  await helper.until(2, 'devicePixelRatio', 2);
  await helper.until(2, '[innerWidth, innerHeight]', [400, 300]);
});

test('resizing and rescaling a page paints at the new size and keeps its state', async () => {
  await helper.evaluate(1, 'window.kept = 42');
  let from = helper.mark();
  helper.send({ Resize: { page: 1, width: 500, height: 200, scale: 1 } });
  await paintedAt(1, 500, 200, from);
  await helper.until(1, '[innerWidth, innerHeight]', [500, 200]);
  from = helper.mark();
  helper.send({ Resize: { page: 1, width: 500, height: 200, scale: 2 } });
  await paintedAt(1, 1000, 400, from);
  await helper.until(1, 'devicePixelRatio', 2);
  await helper.until(1, '[innerWidth, innerHeight, window.kept]', [500, 200, 42]);
  from = helper.mark();
  helper.send({ Resize: { page: 1, width: 500, height: 200, scale: 1 } });
  await paintedAt(1, 500, 200, from);
  await helper.until(1, 'devicePixelRatio', 1);
});

test('a page larger than the screen paints its whole size', async () => {
  const from = helper.mark();
  await helper.open(3, `${fixtures.main}/paint`, { width: 2400, height: 1600, scale: 1 });
  await paintedAt(3, 2400, 1600, from);
  await helper.until(3, '[innerWidth, innerHeight]', [2400, 1600]);
  helper.send({ Close: { page: 3 } });
});
