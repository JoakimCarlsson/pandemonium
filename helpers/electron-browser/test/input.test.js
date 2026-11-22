'use strict';

/**
 * Tests of the editing shortcuts the helper runs itself, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { shortcutEdit } = require('../src/input');

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
