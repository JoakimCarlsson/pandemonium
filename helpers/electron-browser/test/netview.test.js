'use strict';

/**
 * Tests of how recorded requests and console lines are filtered and written for agents, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const netview = require('../src/netview');

test('types are parsed case-insensitively and unknown ones are refused', () => {
  assert.equal(netview.parseTypes(undefined), null);
  assert.deepEqual([.../** @type {Set<string>} */ (netview.parseTypes('XHR, fetch'))], ['xhr', 'fetch']);
  assert.ok('error' in /** @type {object} */ (netview.parseTypes('pictures')));
});

test('an address matches text it contains or a wildcard pattern over all of it', () => {
  assert.ok(netview.urlMatches('http://localhost:5173/api/items?x=1', '/API/items'));
  assert.ok(netview.urlMatches('http://localhost:5173/api/items?x=1', '*/api/*'));
  assert.ok(!netview.urlMatches('http://localhost:5173/static/app.js', '*/api/*'));
  assert.ok(netview.urlMatches('http://a.test/x.(y)', 'http://a.test/x.(*'));
});

test('a stack is written one frame to a line with lines and columns counted from 1', () => {
  const stack = netview.stackText({ callFrames: [{ functionName: 'go', url: 'http://a.test/app.js', lineNumber: 9, columnNumber: 4 }, { functionName: '', url: '', lineNumber: 0, columnNumber: 0 }] });
  assert.equal(stack, 'at go (http://a.test/app.js:10:5)\nat (anonymous) (<anonymous>:1:1)');
});

test('a console line carries its number, time, level, source and indented stack', () => {
  const text = netview.consoleLine({ seq: 3, at: Date.UTC(2026, 0, 1, 12, 4, 5, 120), level: 'error', text: 'TypeError: x is undefined', source: 'http://a.test/app.js:10:5', stack: 'at go (http://a.test/app.js:10:5)', uncaught: true });
  assert.equal(text, '#3 12:04:05.120 [error] Uncaught TypeError: x is undefined (http://a.test/app.js:10:5)\n    at go (http://a.test/app.js:10:5)');
});
