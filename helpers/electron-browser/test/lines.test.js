'use strict';

/**
 * Tests of how the helper splits what the window sends into lines, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { LineReader } = require('../src/lines');

test('a character split across two chunks arrives whole', () => {
  const bytes = Buffer.from('{"text":"héllo ✓ 😀"}\n', 'utf8');
  for (let cut = 1; cut < bytes.length; cut += 1) {
    const reader = new LineReader();
    const lines = [...reader.push(bytes.subarray(0, cut)), ...reader.push(bytes.subarray(cut))];
    assert.deepEqual(lines, ['{"text":"héllo ✓ 😀"}'], `cut at byte ${cut}`);
  }
});

test('a chunk byte by byte gives the same lines as one chunk', () => {
  const text = 'första\nzweite ✓\n\nlast 😀\n';
  const reader = new LineReader();
  const lines = [];
  for (const byte of Buffer.from(text, 'utf8')) lines.push(...reader.push(Buffer.from([byte])));
  assert.deepEqual(lines, ['första', 'zweite ✓', '', 'last 😀']);
});

test('several lines in one chunk come out in order and the rest is kept', () => {
  const reader = new LineReader();
  assert.deepEqual(reader.push(Buffer.from('a\nb\nc')), ['a', 'b']);
  assert.deepEqual(reader.push(Buffer.from('d\n')), ['cd']);
  assert.deepEqual(reader.push(Buffer.alloc(0)), []);
});
