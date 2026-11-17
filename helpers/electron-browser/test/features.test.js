'use strict';

/**
 * Tests of what the helper announces to the window, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const features = require('../src/features');

test('the helper announces sealed pages in the shape the window reads', () => {
  assert.deepEqual(JSON.parse(JSON.stringify(features.announcement())), {
    Features: { features: ['sealed-pages'] },
  });
  assert.equal(features.SEALED_PAGES, 'sealed-pages');
});
