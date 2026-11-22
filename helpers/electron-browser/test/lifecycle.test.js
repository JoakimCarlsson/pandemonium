'use strict';

/**
 * Tests of which failed loads the window is told of, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { shownFailure } = require('../src/lifecycle');

test('a stopped load and certificate errors are not shown as failures', () => {
  assert.equal(shownFailure(-3), false);
  assert.equal(shownFailure(-200), false);
  assert.equal(shownFailure(-202), false);
  assert.equal(shownFailure(-299), false);
});

test('names that do not resolve, refused connections and timeouts are shown', () => {
  assert.equal(shownFailure(-105), true);
  assert.equal(shownFailure(-102), true);
  assert.equal(shownFailure(-7), true);
  assert.equal(shownFailure(-118), true);
  assert.equal(shownFailure(-106), true);
});
