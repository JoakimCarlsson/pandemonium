'use strict';

/**
 * Tests of the blocker's decisions and of the vault of logins, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const policy = require('../src/blocker-policy');
const { Vault, originOf } = require('../src/vault');

/**
 * A cipher for tests that only reverses the bytes, so sealed text differs from plain text.
 * @returns {import('../src/vault').Cipher} The cipher.
 */
function reversing() {
  return {
    seal: async (plain) => Buffer.from(plain, 'utf8').reverse(),
    open: async (sealed) => Buffer.from(sealed).reverse().toString('utf8'),
  };
}

/**
 * A folder for one test under the user's cache, removed when the test ends.
 * @param {import('node:test').TestContext} context The test.
 * @returns {string} The folder.
 */
function scratch(context) {
  const base = path.join(os.homedir(), '.cache', 'scratch', 'pandemonium');
  fs.mkdirSync(base, { recursive: true });
  const folder = fs.mkdtempSync(path.join(base, 'vault-'));
  context.after(() => fs.rmSync(folder, { recursive: true, force: true }));
  return folder;
}

test('the blocker leaves this machine and the sites let through alone', () => {
  const allowed = new Set(['news.example.com']);
  assert.equal(policy.blocksOn(true, 'https://shop.example.com/cart', allowed), true);
  assert.equal(policy.blocksOn(false, 'https://shop.example.com/cart', allowed), false);
  assert.equal(policy.blocksOn(true, 'https://news.example.com/', allowed), false);
  assert.equal(policy.blocksOn(true, 'http://localhost:5173/', allowed), false);
  assert.equal(policy.blocksOn(true, 'http://app.localhost:3000/', allowed), false);
  assert.equal(policy.blocksOn(true, 'http://127.0.0.1:8080/', allowed), false);
  assert.equal(policy.blocksOn(true, 'http://[::1]:8080/', allowed), false);
  assert.equal(policy.blocksOn(true, 'about:blank', allowed), false);
  assert.equal(policy.blocksOn(true, 'data:text/html,hi', allowed), false);
});

test('the filter lists are fetched again once a day', () => {
  const now = 10 * policy.MAX_AGE;
  assert.equal(policy.stale(null, now), true);
  assert.equal(policy.stale(now - policy.MAX_AGE + 1, now), false);
  assert.equal(policy.stale(now - policy.MAX_AGE, now), true);
  assert.equal(policy.stale(now + 2 * policy.MAX_AGE, now), true);
});

test('the sites let through are read leniently', () => {
  assert.deepEqual([...policy.allowedFrom(['A.example.com', '', 3, ' b.example '])], ['a.example.com', 'b.example']);
  assert.deepEqual([...policy.allowedFrom({ nope: true })], []);
});

test('logins are kept sealed by origin and user name', async (context) => {
  const file = path.join(scratch(context), 'passwords.json');
  const vault = new Vault(file, reversing());
  const origin = originOf('https://example.com/login?next=/');
  assert.equal(origin, 'https://example.com');
  assert.equal(originOf('file:///etc/passwd'), '');
  assert.equal(await vault.compare(origin, { username: 'ada', password: 'one' }), 'new');
  await vault.save(origin, { username: 'ada', password: 'one' }, 1);
  assert.equal(await vault.compare(origin, { username: 'ada', password: 'one' }), 'same');
  assert.equal(await vault.compare(origin, { username: 'ada', password: 'two' }), 'update');
  await vault.save(origin, { username: 'ada', password: 'two' }, 2);
  await vault.save(origin, { username: 'grace', password: 'three' }, 3);
  const written = fs.readFileSync(file, 'utf8');
  assert.equal(written.includes('two'), false);
  assert.equal(written.includes('ada'), false);
  const again = new Vault(file, reversing());
  assert.deepEqual(await again.loginsOf(origin), [
    { username: 'ada', password: 'two' },
    { username: 'grace', password: 'three' },
  ]);
  assert.deepEqual(await again.loginsOf('https://other.example.com'), []);
  assert.equal(again.has(origin), true);
});

test('an origin refused is never offered again until a login is kept for it', async (context) => {
  const file = path.join(scratch(context), 'passwords.json');
  const vault = new Vault(file, reversing());
  vault.refuse('https://example.com');
  assert.equal(new Vault(file, reversing()).never.has('https://example.com'), true);
  await vault.save('https://example.com', { username: '', password: 'pin' }, 1);
  assert.equal(new Vault(file, reversing()).never.has('https://example.com'), false);
});

test('a broken vault file is an empty vault', (context) => {
  const file = path.join(scratch(context), 'passwords.json');
  fs.writeFileSync(file, '{not json');
  const vault = new Vault(file, reversing());
  assert.deepEqual(vault.logins, []);
  assert.equal(vault.has('https://example.com'), false);
});
