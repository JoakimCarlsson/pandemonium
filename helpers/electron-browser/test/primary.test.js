'use strict';

/**
 * Tests of the primary password that seals saved logins where the system's keychain cannot, and of the vault sealed
 * by it.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { afterEach, test } = require('node:test');
const { PrimaryLock, SHORTEST } = require('../src/primary');
const { Vault } = require('../src/vault');

/** @type {string[]} The scratch folders the tests made. */
const folders = [];

/**
 * A new, empty scratch folder under `~/.cache/scratch/pandemonium`, never a RAM-backed temporary folder.
 * @returns {string} Its path.
 */
function scratch() {
  const root = path.join(os.homedir(), '.cache', 'scratch', 'pandemonium');
  fs.mkdirSync(root, { recursive: true });
  const folder = fs.mkdtempSync(path.join(root, 'primary-'));
  folders.push(folder);
  return folder;
}

afterEach(() => {
  for (const folder of folders.splice(0)) fs.rmSync(folder, { recursive: true, force: true });
});

test('a new primary password must be long enough, and is set only once', async () => {
  const lock = new PrimaryLock(path.join(scratch(), 'key.json'));
  assert.equal(lock.isSet(), false);
  assert.match(await lock.create('short'), new RegExp(`${SHORTEST} characters`));
  assert.equal(lock.isSet(), false);
  assert.equal(await lock.create('correct horse'), '');
  assert.equal(lock.isSet(), true);
  assert.equal(lock.unlocked(), true);
  assert.notEqual(await lock.create('another one'), '');
});

test('the file keeps a verifier, never the password or the key, and only its owner reads it', async () => {
  const file = path.join(scratch(), 'key.json');
  const lock = new PrimaryLock(file);
  await lock.create('correct horse');
  const text = fs.readFileSync(file, 'utf8');
  assert.ok(!text.includes('correct horse'));
  assert.ok(!text.includes(/** @type {Buffer} */ (lock.key).toString('base64')));
  const kept = JSON.parse(text);
  assert.deepEqual(Object.keys(kept).sort(), ['N', 'p', 'r', 'salt', 'verifier', 'version']);
  assert.equal(fs.statSync(file).mode & 0o777, 0o600);
});

test('a lock read again from its file is locked, opens with the right password only, and locks again', async () => {
  const file = path.join(scratch(), 'key.json');
  await new PrimaryLock(file).create('correct horse');
  const again = new PrimaryLock(file);
  assert.equal(again.isSet(), true);
  assert.equal(again.unlocked(), false);
  assert.equal(await again.unlock('wrong horse'), false);
  assert.equal(again.unlocked(), false);
  assert.equal(await again.unlock('correct horse'), true);
  again.lock();
  assert.equal(again.unlocked(), false);
});

test('a vault sealed by the primary password keeps logins across a restart and opens none while locked', async () => {
  const folder = scratch();
  const keyFile = path.join(folder, 'key.json');
  const vaultFile = path.join(folder, 'vault.json');
  const lock = new PrimaryLock(keyFile);
  await lock.create('correct horse');
  const vault = new Vault(vaultFile, lock.cipher());
  await vault.save('https://example.com', { username: 'ada', password: 'secret-1' }, 1);
  assert.ok(!fs.readFileSync(vaultFile, 'utf8').includes('secret-1'));
  assert.ok(!fs.readFileSync(vaultFile, 'utf8').includes('ada'));

  const restarted = new PrimaryLock(keyFile);
  const reopened = new Vault(vaultFile, restarted.cipher());
  assert.equal(reopened.has('https://example.com'), true);
  assert.deepEqual(await reopened.loginsOf('https://example.com'), []);
  await assert.rejects(reopened.save('https://example.com', { username: 'bob', password: 'x' }, 2), /locked/);
  assert.equal(await restarted.unlock('correct horse'), true);
  assert.deepEqual(await reopened.loginsOf('https://example.com'), [{ username: 'ada', password: 'secret-1' }]);
});

test('a sealed login changed on disk no longer opens', async () => {
  const folder = scratch();
  const lock = new PrimaryLock(path.join(folder, 'key.json'));
  await lock.create('correct horse');
  const sealed = await lock.cipher().seal('hello');
  sealed[sealed.length - 1] ^= 1;
  await assert.rejects(lock.cipher().open(sealed));
});
