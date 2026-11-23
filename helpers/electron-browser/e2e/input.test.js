'use strict';

/**
 * End-to-end tests of the reader's input: clicks and typing in the top frame and in same-origin and cross-origin
 * frames, a box laid over a cross-origin frame, how fast pointer moves reach the page, the clipboard and undo, and keys with Control and of other layouts.
 */

const assert = require('node:assert/strict');
const { after, before, test } = require('node:test');
const { Helper, sleep } = require('./harness');
const { startFixtures } = require('./fixtures');

/** The bit of `modifiers` for Control, as `Modifiers::CONTROL` in `crates/pm-browser/src/input.rs`. */
const CONTROL = 4;

/** The bit of the platform's shortcut key: Command on macOS, Control elsewhere. */
const SHORTCUT = process.platform === 'darwin' ? 128 : CONTROL;

/** @type {Helper} */
let helper;
/** @type {import('./fixtures').Fixtures} */
let fixtures;

before(async () => {
  fixtures = await startFixtures();
  helper = await Helper.start();
  await helper.open(1, `${fixtures.main}/input`, { width: 600, height: 400, scale: 1 });
  await helper.until(1, "document.querySelectorAll('iframe').length", 2);
});

after(async () => {
  await helper?.stop();
  await fixtures?.close();
});

/**
 * Clicks the primary button at `x`, `y` of page 1.
 * @param {number} x Logical pixels from the left.
 * @param {number} y Logical pixels from the top.
 * @returns {void}
 */
function click(x, y) {
  helper.send({ PointerMoved: { page: 1, x, y, modifiers: 0, left: false } });
  helper.send({ PointerButton: { page: 1, x, y, button: 'Left', pressed: true, clicks: 1, modifiers: 16 } });
  helper.send({ PointerButton: { page: 1, x, y, button: 'Left', pressed: false, clicks: 1, modifiers: 0 } });
}

/**
 * Presses and releases a key on page 1.
 * @param {number} code Its Windows virtual-key code.
 * @param {string | null} text What it types, or nothing.
 * @param {number} [modifiers] The modifier bits held.
 * @returns {void}
 */
function key(code, text, modifiers = 0) {
  helper.send({ Key: { page: 1, key: { pressed: true, code, text, modifiers } } });
  helper.send({ Key: { page: 1, key: { pressed: false, code, text: null, modifiers } } });
}

/**
 * Types `word`, made of lower-case letters, key by key into page 1.
 * @param {string} word The letters.
 * @returns {void}
 */
function typeWord(word) {
  for (const letter of word) key(letter.toUpperCase().charCodeAt(0), letter);
}

test('a click and typing reach the field of the top frame', async () => {
  click(100, 25);
  await helper.until(1, "document.activeElement.id", 'top');
  typeWord('top');
  await helper.until(1, "document.getElementById('top').value", 'top');
});

test('a click and typing reach a same-origin frame', async () => {
  click(90, 125);
  await helper.until(1, 'frames_seen.same.clicks', 1);
  typeWord('same');
  await helper.until(1, 'frames_seen.same.value', 'same');
});

test('a click and typing reach a cross-origin frame', async () => {
  click(90, 275);
  await helper.until(1, 'frames_seen.cross.clicks', 1);
  typeWord('cross');
  await helper.until(1, 'frames_seen.cross.value', 'cross');
  helper.send({ CommitText: { page: 1, text: 'é日本' } });
  await helper.until(1, 'frames_seen.cross.value', 'crossé日本');
});

test('Control with a slash reaches the page as a key with Control', async () => {
  click(100, 25);
  await helper.until(1, "document.activeElement.id", 'top');
  await helper.evaluate(1, 'keys.length = 0');
  key(191, null, CONTROL);
  await helper.until(1, "keys.filter((k) => k.ctrl).map((k) => k.key + ':' + k.code)", ['/:Slash']);
});

test('a key of another layout and committed text type what they carry', async () => {
  await helper.evaluate(1, "(() => { const f = document.getElementById('top'); f.value = ''; f.focus(); return true; })()");
  key(186, 'ü');
  await helper.until(1, "document.getElementById('top').value", 'ü');
  helper.send({ CommitText: { page: 1, text: 'ß日本語' } });
  await helper.until(1, "document.getElementById('top').value", 'üß日本語');
});

test('copy, cut, paste and undo work on the field that holds the keyboard', async () => {
  await helper.evaluate(1, "(() => { const f = document.getElementById('top'); f.value = 'hello'; f.focus(); f.select(); return true; })()");
  let from = helper.mark();
  helper.send({ Edit: { page: 1, action: 'Copy' } });
  assert.equal((await helper.waitReport('Copied', 1, () => true, { from })).text, 'hello');
  from = helper.mark();
  key(67, null, SHORTCUT);
  assert.equal((await helper.waitReport('Copied', 1, () => true, { from })).text, 'hello');
  from = helper.mark();
  helper.send({ Edit: { page: 1, action: 'Cut' } });
  assert.equal((await helper.waitReport('Copied', 1, () => true, { from })).text, 'hello');
  await helper.until(1, "document.getElementById('top').value", '');
  helper.send({ PasteText: { page: 1, text: 'pasted' } });
  await helper.until(1, "document.getElementById('top').value", 'pasted');
  helper.send({ Edit: { page: 1, action: 'Undo' } });
  await helper.until(1, "document.getElementById('top').value", '');
  key(90, null, SHORTCUT);
  await helper.until(1, "document.getElementById('top').value", 'hello');
});

/**
 * Moves the pointer to `x`, `y` of page 1.
 * @param {number} x Logical pixels from the left.
 * @param {number} y Logical pixels from the top.
 * @returns {void}
 */
function move(x, y) {
  helper.send({ PointerMoved: { page: 1, x, y, modifiers: 0, left: false } });
}

test('a box laid over part of a cross-origin frame takes the pointer there, and the rest reaches the frame', async () => {
  await helper.load(1, `${fixtures.main}/covered`);
  await helper.until(1, '!!window.overs', true);
  await sleep(500);
  move(400, 300);
  move(60, 80);
  await helper.until(1, 'overs.cover', 1);
  await sleep(150);
  await helper.until(1, 'overs.frame', 0);
  move(250, 200);
  await helper.until(1, 'overs.frame', 1);
  await helper.evaluate(1, "document.getElementById('cover').style.left = '160px', 1");
  move(250, 200);
  move(200, 70);
  await helper.until(1, 'overs.cover', 2);
  move(60, 80);
  await helper.until(1, 'overs.frame', 2);
});

test('a box 2 pixels square over a cross-origin frame takes the pointer exactly there', async () => {
  await helper.load(1, `${fixtures.main}/covered`);
  await helper.until(1, '!!window.overs', true);
  await sleep(500);
  move(400, 300);
  move(270, 230);
  await helper.until(1, 'overs.frame', 1);
  move(281, 221);
  await helper.until(1, 'overs.tiny', 1);
  move(283, 221);
  await helper.until(1, 'overs.frame', 2);
});

test('a box a :hover rule shows over a cross-origin frame takes the pointer, and the rest reaches the frame', async () => {
  await helper.load(1, `${fixtures.main}/covered`);
  await helper.until(1, '!!window.overs', true);
  await sleep(500);
  move(400, 390);
  move(50, 290);
  await helper.until(1, 'overs.reveal', 1);
  await sleep(150);
  await helper.until(1, 'overs.held', 0);
  move(200, 340);
  await helper.until(1, 'overs.held', 1);
  move(50, 290);
  await helper.until(1, 'overs.reveal', 2);
});

test('the last of a burst of 300 pointer moves reaches the page within 100 ms', async () => {
  await helper.load(1, `${fixtures.main}/hover`);
  await sleep(200);
  const from = helper.mark();
  for (let step = 0; step < 300; step += 1) {
    helper.send({ PointerMoved: { page: 1, x: 10 + (step % 500), y: 10 + Math.floor(step / 5), modifiers: 0, left: false } });
  }
  const sent = performance.now();
  helper.send({ PointerMoved: { page: 1, x: 321, y: 123, modifiers: 0, left: false } });
  await helper.waitReport('Title', 1, (b) => b.title === 'at 321,123', { from, timeout: 5000 });
  const arrived = helper.timed.find((t) => t.report.Title?.title === 'at 321,123' && t.report.Title.page === 1);
  const latency = (arrived?.at ?? Infinity) - sent;
  assert.ok(latency < 100, `the last move took ${latency.toFixed(1)} ms to reach the page`);
});
