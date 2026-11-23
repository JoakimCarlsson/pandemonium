'use strict';

/**
 * Tests of which profiles the helper keeps across runs, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const partitions = require('../src/partitions');

test('the shared profile and persist: profiles are kept, others live in memory only', () => {
  assert.equal(partitions.kept(undefined), true);
  assert.equal(partitions.kept(''), true);
  assert.equal(partitions.kept('persist:project-00ff'), true);
  assert.equal(partitions.kept('private'), false);
});

test('a profile kept on disk, or one a page still lives in, is not emptied', async () => {
  await partitions.release('persist:project-00ff', []);
  await partitions.release('private', [{ partition: 'private' }]);
  await partitions.release('private', []);
});
