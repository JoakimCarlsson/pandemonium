'use strict';

/**
 * Tests of how keys the window reports become the engine's key events, run with `node --test test/` in the helper's
 * folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { cursorOf, keyEvents, keyName, macEditing } = require('../src/input');

/**
 * A press of the key with virtual-key code `code`, typing `text`, with the modifier bits `modifiers`.
 * @param {number} code The key's virtual-key code.
 * @param {number} modifiers The modifier bits held.
 * @param {string | null} [text] What it types.
 * @returns {import('../src/types').Key} The key.
 */
function press(code, modifiers, text = null) {
  return { code, pressed: true, modifiers, text };
}

test('punctuation keys are named by their unshifted character', () => {
  assert.equal(keyName(0xbf), '/');
  assert.equal(keyName(0xdb), '[');
  assert.equal(keyName(0xde), "'");
  assert.equal(keyName(0xdc), '\\');
});

test('the number pad, the locks, Insert and the media keys have names', () => {
  assert.equal(keyName(0x65), 'num5');
  assert.equal(keyName(0x6b), 'numadd');
  assert.equal(keyName(0x14), 'CapsLock');
  assert.equal(keyName(0x2d), 'Insert');
  assert.equal(keyName(0xb3), 'MediaPlayPause');
  assert.equal(keyName(0x87), 'F24');
});

test('control with slash sends the key and types nothing', () => {
  const events = keyEvents(press(0xbf, 4));
  assert.deepEqual(events, [{ type: 'keyDown', keyCode: '/', modifiers: ['control'] }]);
});

test('a character of another layout types its text', () => {
  const events = keyEvents(press(0x32, 0, 'é'));
  assert.deepEqual(
    events.map((event) => [event.type, event.keyCode]),
    [
      ['keyDown', '2'],
      ['char', 'é'],
    ],
  );
});

test('command and option with arrows and Backspace edit as macOS does', () => {
  assert.equal(macEditing(press(37, 128), 'darwin'), 'moveToBeginningOfLine');
  assert.equal(macEditing(press(39, 8 | 2), 'darwin'), 'moveWordRightAndModifySelection');
  assert.equal(macEditing(press(8, 128), 'darwin'), 'deleteToBeginningOfLine');
  assert.equal(macEditing(press(8, 8), 'darwin'), 'deleteWordBackward');
  assert.equal(macEditing(press(37, 128), 'linux'), null);
  assert.equal(macEditing(press(37, 0), 'darwin'), null);
  assert.equal(macEditing(press(37, 128 | 8), 'darwin'), null);
});

test('the engine cursors map to the closest look the window has', () => {
  assert.equal(cursorOf('grab'), 'Grab');
  assert.equal(cursorOf('grabbing'), 'Grabbing');
  assert.equal(cursorOf('not-allowed'), 'NotAllowed');
  assert.equal(cursorOf('nwse-resize'), 'ResizeNwse');
  assert.equal(cursorOf('nesw-resize'), 'ResizeNesw');
  assert.equal(cursorOf('all-scroll'), 'Move');
  assert.equal(cursorOf('cell'), 'Cross');
  assert.equal(cursorOf('zoom-in'), 'Arrow');
});
