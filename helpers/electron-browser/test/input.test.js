'use strict';

/**
 * Tests of the editing shortcuts the helper runs itself, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { protocolKey, shortcutEdit } = require('../src/input');

/** The bit the window sets for the shortcut key on this platform. */
const SHORTCUT = process.platform === 'darwin' ? 128 : 4;

/**
 * A press of the letter `letter` with the modifier bits `modifiers`.
 * @param {string} letter The uppercase letter.
 * @param {number} modifiers The modifier bits held.
 * @returns {import('../src/types').Key} The key.
 */
function press(letter, modifiers) {
  return { code: letter.charCodeAt(0), pressed: true, modifiers, text: '' };
}

test('the shortcut key with C, X and V copies, cuts and pastes', () => {
  assert.equal(shortcutEdit(press('C', SHORTCUT)), 'copy');
  assert.equal(shortcutEdit(press('X', SHORTCUT)), 'cut');
  assert.equal(shortcutEdit(press('V', SHORTCUT)), 'paste');
});

test('the shortcut key with Z undoes, and with shift redoes', () => {
  assert.equal(shortcutEdit(press('Z', SHORTCUT)), 'undo');
  assert.equal(shortcutEdit(press('Z', SHORTCUT | 2)), 'redo');
});

test('a letter without the shortcut key or with alt is no shortcut', () => {
  assert.equal(shortcutEdit(press('C', 0)), null);
  assert.equal(shortcutEdit(press('C', SHORTCUT | 8)), null);
  assert.equal(shortcutEdit(press('C', SHORTCUT | 2)), null);
});

test('a key sent through the protocol carries its DOM key and code, and text only while it types', () => {
  assert.deepEqual(protocolKey({ code: 65, pressed: true, modifiers: 0, text: 'a' }), {
    type: 'keyDown',
    modifiers: 0,
    windowsVirtualKeyCode: 65,
    nativeVirtualKeyCode: 65,
    key: 'a',
    code: 'KeyA',
    text: 'a',
    unmodifiedText: 'a',
  });
  const slash = protocolKey({ code: 191, pressed: true, modifiers: SHORTCUT, text: null });
  assert.equal(slash.type, 'rawKeyDown');
  assert.equal(slash.key, '/');
  assert.equal(slash.code, 'Slash');
  assert.equal(slash.text, undefined);
  assert.equal(protocolKey({ code: 37, pressed: false, modifiers: 0, text: null }).key, 'ArrowLeft');
  assert.equal(protocolKey({ code: 13, pressed: true, modifiers: 0, text: '\r' }).key, 'Enter');
});
