'use strict';

/**
 * Tests of how the reader's answers to the permissions pages ask for are kept: by profile and origin, and forgotten
 * with a profile kept in memory once its last page closes. Run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const partitions = require('../src/partitions');
const safety = require('../src/safety');

test('an answer in one profile is not an answer in another', () => {
  const origin = 'https://meet.example.com';
  const shared = safety.answerKey('', origin, 'camera');
  const project = safety.answerKey('persist:project-00ff', origin, 'camera');
  const secret = safety.answerKey('private', origin, 'camera');
  assert.equal(safety.answerKey(undefined, origin, 'camera'), shared);
  assert.equal(new Set([shared, project, secret]).size, 3);
});

test('the answers of a profile kept in memory go when its last page closes, and no other profile loses its own', async () => {
  const origin = 'https://meet.example.com';
  /** @type {string[]} */
  const released = [];
  partitions.onRelease((name) => released.push(name));
  partitions.onRelease(safety.forgetProfile);
  safety.remembered.set(safety.answerKey('', origin, 'camera'), true);
  safety.remembered.set(safety.answerKey('persist:project-00ff', origin, 'camera'), true);
  safety.remembered.set(safety.answerKey('private', origin, 'camera'), true);
  safety.remembered.set(safety.answerKey('private-2', origin, 'camera'), true);
  await partitions.release('private', [{ partition: 'private' }]);
  assert.equal(safety.remembered.get(safety.answerKey('private', origin, 'camera')), true);
  await partitions.release('persist:project-00ff', []);
  await partitions.release('private', []);
  assert.deepEqual(released, ['private']);
  assert.equal(safety.remembered.has(safety.answerKey('private', origin, 'camera')), false);
  assert.equal(safety.remembered.get(safety.answerKey('private-2', origin, 'camera')), true);
  assert.equal(safety.remembered.get(safety.answerKey('', origin, 'camera')), true);
  assert.equal(safety.remembered.get(safety.answerKey('persist:project-00ff', origin, 'camera')), true);
});
