'use strict';

/**
 * Tests of the shared files pages paint into, run with `node --test test/` in the helper's folder. Scratch files go
 * in the repository's ignored `build/` folder.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const { Surface, dirtyRows, joinRows } = require('../src/surface');

/** The ignored folder the tests write their files under. */
const SCRATCH = path.join(__dirname, '..', '..', '..', 'build', 'helper-tests');

/**
 * A fresh folder for one test's files.
 * @returns {string} Its path.
 */
function folder() {
  fs.mkdirSync(SCRATCH, { recursive: true });
  return fs.mkdtempSync(path.join(SCRATCH, 'surface-'));
}

/**
 * A stand-in for a painted image whose every pixel is `fill`.
 * @param {number} width Width in pixels.
 * @param {number} height Height in pixels.
 * @param {(index: number) => number} fill The byte at each index of the BGRA bitmap.
 * @returns {{ getScaleFactors: () => number[], getSize: () => { width: number, height: number }, toBitmap: () => Buffer }} The image.
 */
function image(width, height, fill) {
  const bitmap = Buffer.alloc(width * height * 4);
  for (let i = 0; i < bitmap.length; i += 1) bitmap[i] = fill(i) & 0xff;
  return { getScaleFactors: () => [1], getSize: () => ({ width, height }), toBitmap: () => bitmap };
}

/**
 * The pixels of a surface file after its header.
 * @param {string} file The file.
 * @returns {Buffer} The pixel bytes.
 */
function pixelsOf(file) {
  return fs.readFileSync(file).subarray(16);
}

test('dirty rows cover the rectangle in logical or device pixels', () => {
  assert.deepEqual(dirtyRows(undefined, 2, 100), { top: 0, bottom: 100 });
  assert.deepEqual(dirtyRows({ x: 0, y: 10, width: 5, height: 5 }, 1, 100), { top: 10, bottom: 15 });
  assert.deepEqual(dirtyRows({ x: 0, y: 10, width: 5, height: 5 }, 2, 100), { top: 10, bottom: 30 });
  assert.deepEqual(dirtyRows({ x: 0, y: 90, width: 5, height: 50 }, 2, 100), { top: 90, bottom: 100 });
  assert.deepEqual(joinRows(null, { top: 3, bottom: 4 }), { top: 3, bottom: 4 });
  assert.deepEqual(joinRows({ top: 1, bottom: 2 }, { top: 3, bottom: 4 }), { top: 1, bottom: 4 });
});

test('a file gets the rows dirty since it was last written, joined over both paints', () => {
  const dir = folder();
  /** @type {[string, string][]} */
  const announced = [];
  const surface = new Surface(dir, 1, (_w, _h, paths) => announced.push(paths));
  const frames = [0, 1, 2, 3].map((n) => image(4, 8, (i) => i + n * 7));
  surface.write(/** @type {any} */ (frames[0]));
  surface.write(/** @type {any} */ (frames[1]));
  const third = surface.write(/** @type {any} */ (frames[2]), { x: 0, y: 2, width: 4, height: 1 });
  const fourth = surface.write(/** @type {any} */ (frames[3]), { x: 0, y: 5, width: 4, height: 2 });
  const paths = announced[0];
  const bitmap2 = frames[2].toBitmap();
  const bitmap3 = frames[3].toBitmap();
  assert.deepEqual(pixelsOf(paths[/** @type {any} */ (third).buffer]), bitmap2);
  const expected3 = Buffer.from(frames[1].toBitmap());
  bitmap3.copy(expected3, 2 * 16, 2 * 16, 7 * 16);
  assert.deepEqual(pixelsOf(paths[/** @type {any} */ (fourth).buffer]), expected3);
  assert.equal(fs.readFileSync(paths[0]).subarray(8, 12).toString('latin1'), 'BGRA');
  assert.equal(fs.readFileSync(paths[0]).readBigUInt64LE(0) % 2n, 0n);
  surface.close();
  fs.rmSync(dir, { recursive: true, force: true });
});

test('files of an earlier size stay until they are retired long enough', () => {
  const dir = folder();
  /** @type {[string, string][]} */
  const announced = [];
  const surface = new Surface(dir, 2, (_w, _h, paths) => announced.push(paths));
  surface.write(/** @type {any} */ (image(4, 4, () => 1)));
  surface.write(/** @type {any} */ (image(6, 4, () => 2)));
  assert.equal(announced.length, 2);
  assert.ok(announced[0].every((file) => fs.existsSync(file)), 'old files are kept for the window');
  surface.retired[0].at -= 10_000;
  surface.write(/** @type {any} */ (image(6, 4, () => 3)));
  assert.ok(announced[0].every((file) => !fs.existsSync(file)), 'old files are removed later');
  surface.close();
  assert.ok(announced[1].every((file) => !fs.existsSync(file)), 'closing removes the rest');
  assert.equal(surface.retired.length, 0);
  fs.rmSync(dir, { recursive: true, force: true });
});

test('measure: a 1080p paint, before and after', () => {
  const width = 1920;
  const height = 1080;
  const dir = folder();
  const picture = image(width, height, (i) => i);
  const bitmap = picture.toBitmap();
  const rounds = 20;
  const file = fs.openSync(path.join(dir, 'before.rgba'), 'w+');
  fs.ftruncateSync(file, 16 + bitmap.length);
  const beforeStart = process.hrtime.bigint();
  for (let round = 0; round < rounds; round += 1) {
    const out = Buffer.alloc(bitmap.length);
    const from = new Uint32Array(bitmap.buffer, bitmap.byteOffset, bitmap.length >> 2);
    const to = new Uint32Array(out.buffer, out.byteOffset, out.length >> 2);
    for (let i = 0; i < from.length; i += 1) {
      const pixel = from[i];
      to[i] = (pixel & 0xff00ff00) | ((pixel & 0xff) << 16) | ((pixel >>> 16) & 0xff);
    }
    fs.writeSync(file, out, 0, out.length, 16);
  }
  const before = Number(process.hrtime.bigint() - beforeStart) / 1e6 / rounds;
  fs.closeSync(file);
  const surface = new Surface(dir, 3, () => {});
  surface.write(/** @type {any} */ (picture));
  surface.write(/** @type {any} */ (picture));
  const wholeStart = process.hrtime.bigint();
  for (let round = 0; round < rounds; round += 1) surface.write(/** @type {any} */ (picture));
  const whole = Number(process.hrtime.bigint() - wholeStart) / 1e6 / rounds;
  const caret = { x: 100, y: 500, width: 2, height: 20 };
  const dirtyStart = process.hrtime.bigint();
  for (let round = 0; round < rounds; round += 1) surface.write(/** @type {any} */ (picture), caret);
  const dirty = Number(process.hrtime.bigint() - dirtyStart) / 1e6 / rounds;
  console.log(
    `1080p paint: swizzle and whole write ${before.toFixed(2)} ms, whole write ${whole.toFixed(2)} ms, caret blink ${dirty.toFixed(3)} ms`,
  );
  surface.close();
  fs.rmSync(dir, { recursive: true, force: true });
  assert.ok(dirty <= before);
});
