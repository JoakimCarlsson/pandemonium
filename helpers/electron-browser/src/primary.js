'use strict';

/**
 * The primary password that seals saved logins where the system's keychain cannot: on a Linux desktop without a
 * keyring, where `safeStorage` falls back to a key every install shares. The reader sets it once; the key is derived
 * from it with scrypt and seals each login with AES-256-GCM. The file under the helper's profile holds the salt, the
 * cost of the derivation and a verifier, a known text sealed with the key, so a wrong password is told apart from a
 * right one without keeping the password or the key. The key lives in memory only: the vault is locked again whenever
 * the helper starts. Nothing here needs the engine, so it is tested on its own.
 */

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

/** The text the verifier seals. */
const VERIFIED = 'pandemonium saved logins';

/** The fewest characters a new primary password has. */
const SHORTEST = 8;

/** The cost of scrypt for a new primary password: 2^15 rounds, blocks of 8, one lane, about 32 MiB of memory. */
const COST = { N: 1 << 15, r: 8, p: 1 };

/** The length of the nonce of AES-256-GCM, in bytes. */
const NONCE = 12;

/** The length of the tag of AES-256-GCM, in bytes. */
const TAG = 16;

/**
 * The primary password's file.
 * @typedef {object} PrimaryFile
 * @property {number} version The format, 1.
 * @property {string} salt The salt of scrypt, in base64.
 * @property {number} N The rounds of scrypt.
 * @property {number} r The block size of scrypt.
 * @property {number} p The lanes of scrypt.
 * @property {string} verifier {@link VERIFIED} sealed with the key, in base64.
 */

/**
 * Derives the key of a primary password.
 * @param {string} password The primary password.
 * @param {Buffer} salt The salt.
 * @param {{ N: number, r: number, p: number }} cost The cost of scrypt.
 * @returns {Promise<Buffer>} The 32-byte key.
 * @throws {Error} When scrypt refuses the cost.
 */
function derive(password, salt, cost) {
  return new Promise((resolve, reject) => {
    crypto.scrypt(password.normalize('NFC'), salt, 32, { N: cost.N, r: cost.r, p: cost.p, maxmem: 256 * cost.N * cost.r }, (error, key) => {
      if (error) reject(error);
      else resolve(key);
    });
  });
}

/**
 * Seals text with a key: a fresh nonce, then the tag, then the ciphertext.
 * @param {Buffer} key The key.
 * @param {string} text The text.
 * @returns {Buffer} What was sealed.
 */
function seal(key, text) {
  const nonce = crypto.randomBytes(NONCE);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, nonce);
  const body = Buffer.concat([cipher.update(text, 'utf8'), cipher.final()]);
  return Buffer.concat([nonce, cipher.getAuthTag(), body]);
}

/**
 * Opens what {@link seal} sealed.
 * @param {Buffer} key The key.
 * @param {Buffer} sealed What was sealed.
 * @returns {string} The text.
 * @throws {Error} When the key is not the one it was sealed with or the bytes were changed.
 */
function open(key, sealed) {
  if (sealed.length < NONCE + TAG) throw new Error('too short to have been sealed');
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, sealed.subarray(0, NONCE));
  decipher.setAuthTag(sealed.subarray(NONCE, NONCE + TAG));
  return Buffer.concat([decipher.update(sealed.subarray(NONCE + TAG)), decipher.final()]).toString('utf8');
}

/**
 * The primary password kept in one file, and the key while the vault is unlocked.
 */
class PrimaryLock {
  /**
   * Reads the file, when there is one.
   * @param {string} file Where the salt and verifier are kept.
   */
  constructor(file) {
    /** @type {string} Where the salt and verifier are kept. */
    this.file = file;
    /** @type {Buffer | null} The key, while unlocked. */
    this.key = null;
    /** @type {PrimaryFile | null} What the file holds, once a primary password is set. */
    this.kept = this.read();
  }

  /**
   * What the file holds, when it is of the right shape.
   * @returns {PrimaryFile | null} It, or nothing when there is no primary password yet.
   */
  read() {
    try {
      const kept = JSON.parse(fs.readFileSync(this.file, 'utf8'));
      const whole = typeof kept.salt === 'string' && typeof kept.verifier === 'string'
        && [kept.N, kept.r, kept.p].every((value) => Number.isInteger(value) && value > 0);
      return whole ? kept : null;
    } catch (_) {
      return null;
    }
  }

  /**
   * Whether a primary password is set.
   * @returns {boolean} Whether it is.
   */
  isSet() {
    return this.kept !== null;
  }

  /**
   * Whether the vault is unlocked.
   * @returns {boolean} Whether it is.
   */
  unlocked() {
    return this.key !== null;
  }

  /**
   * Sets the primary password, writes the file and unlocks the vault.
   * @param {string} password The new primary password.
   * @returns {Promise<string>} An empty string when it was set, or why it was refused.
   * @throws {Error} When the file cannot be written.
   */
  async create(password) {
    if (this.kept) return 'A primary password is set already.';
    if ([...password].length < SHORTEST) return `Use at least ${SHORTEST} characters.`;
    const salt = crypto.randomBytes(16);
    const key = await derive(password, salt, COST);
    const kept = { version: 1, salt: salt.toString('base64'), ...COST, verifier: seal(key, VERIFIED).toString('base64') };
    fs.mkdirSync(path.dirname(this.file), { recursive: true });
    const part = `${this.file}.part`;
    fs.writeFileSync(part, JSON.stringify(kept), { mode: 0o600 });
    fs.renameSync(part, this.file);
    this.kept = kept;
    this.key = key;
    return '';
  }

  /**
   * Unlocks the vault with the primary password.
   * @param {string} password The password the reader typed.
   * @returns {Promise<boolean>} Whether it was right.
   */
  async unlock(password) {
    if (!this.kept) return false;
    try {
      const key = await derive(password, Buffer.from(this.kept.salt, 'base64'), this.kept);
      if (open(key, Buffer.from(this.kept.verifier, 'base64')) !== VERIFIED) return false;
      this.key = key;
      return true;
    } catch (_) {
      return false;
    }
  }

  /**
   * Locks the vault, forgetting the key.
   * @returns {void}
   */
  lock() {
    this.key = null;
  }

  /**
   * The cipher of the vault, which works while it is unlocked.
   * @returns {import('./vault').Cipher} Seals and opens with the key.
   */
  cipher() {
    return {
      seal: async (text) => seal(this.need(), text),
      open: async (sealed) => open(this.need(), sealed),
    };
  }

  /**
   * The key.
   * @returns {Buffer} It.
   * @throws {Error} When the vault is locked.
   */
  need() {
    if (!this.key) throw new Error('the saved logins are locked');
    return this.key;
  }
}

module.exports = { PrimaryLock, SHORTEST };
