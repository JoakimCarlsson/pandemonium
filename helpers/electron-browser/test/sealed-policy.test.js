'use strict';

/**
 * Tests of what a sealed page may do, run with `node --test test/` in the helper's folder; they need no engine.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const policy = require('../src/sealed-policy');

test('only data, blob and about addresses may be requested', () => {
  for (const url of ['data:text/html,hi', 'DATA:image/png;base64,AAAA', 'blob:null/1b2c', 'about:blank', 'about:srcdoc']) {
    assert.equal(policy.allowed(url), true, url);
  }
  const refused = [
    'http://example.com/',
    'https://example.com/a.js',
    'ws://example.com/',
    'wss://example.com/',
    'file:///etc/passwd',
    'ftp://example.com/',
    'chrome://settings',
    'devtools://devtools/bundled/inspector.html',
    'javascript:alert(1)',
    '//example.com/a.png',
    'example.com',
    '',
  ];
  for (const url of refused) {
    assert.equal(policy.allowed(url), false, url);
  }
  assert.equal(policy.allowed(undefined), false);
  assert.equal(policy.allowed(null), false);
});

test('a document is refused when it is empty or over the cap', () => {
  assert.equal(policy.refusal('<p>hi</p>'), null);
  assert.equal(policy.refusal('x'.repeat(policy.LARGEST_DOCUMENT)), null);
  assert.match(String(policy.refusal('x'.repeat(policy.LARGEST_DOCUMENT + 1))), /256 KiB/);
  assert.match(String(policy.refusal('')), /no document/);
  assert.match(String(policy.refusal(undefined)), /no document/);
});

test('the policy is the first element and the doctype stays first', () => {
  const plain = policy.sealedDocument('<p>hi</p>');
  assert.ok(plain.startsWith('<meta http-equiv="Content-Security-Policy"'));
  assert.ok(plain.endsWith('<p>hi</p>'));
  const typed = policy.sealedDocument('<!DOCTYPE html>\n<html><head><title>t</title></head></html>');
  assert.ok(typed.startsWith('<!DOCTYPE html><meta http-equiv="Content-Security-Policy"'));
  assert.ok(typed.indexOf('<meta') < typed.indexOf('<html>'));
});

test('the policy fetches nothing and blocks forms and WebRTC', () => {
  assert.match(policy.CONTENT_POLICY, /default-src 'none'/);
  assert.match(policy.CONTENT_POLICY, /form-action 'none'/);
  assert.match(policy.CONTENT_POLICY, /webrtc 'block'/);
  assert.doesNotMatch(policy.CONTENT_POLICY, /https?:|\*/);
});

test('the address is the sealed document as a data address', () => {
  const address = policy.address('<p>hé</p>');
  const prefix = 'data:text/html;charset=utf-8;base64,';
  assert.ok(address.startsWith(prefix));
  assert.equal(Buffer.from(address.slice(prefix.length), 'base64').toString('utf8'), policy.sealedDocument('<p>hé</p>'));
  assert.equal(policy.allowed(address), true);
});

test('a viewport side is a whole number within bounds', () => {
  assert.equal(policy.usableSide(1024), true);
  assert.equal(policy.usableSide(15), false);
  assert.equal(policy.usableSide(5000), false);
  assert.equal(policy.usableSide(10.5), false);
  assert.equal(policy.usableSide('640'), false);
});
