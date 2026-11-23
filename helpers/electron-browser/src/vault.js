'use strict';

/**
 * The logins the reader chose to keep, by origin, and the origins never to offer for, in one file under the helper's
 * profile. Each login is sealed whole, user name and password together, by a cipher the caller gives, which in the
 * helper is the system's keychain (`safeStorage`); only the origin is written in the clear, so the file alone gives
 * nothing away. Nothing here needs the engine, so it is tested on its own.
 */

const fs = require('fs');
const path = require('path');

/**
 * Seals and opens text with a key the file does not hold.
 * @typedef {object} Cipher
 * @property {(text: string) => Promise<Buffer>} seal Seals text.
 * @property {(sealed: Buffer) => Promise<string>} open Opens what {@link Cipher#seal} sealed.
 */

/**
 * One kept login as the file holds it.
 * @typedef {object} KeptLogin
 * @property {string} origin The origin it is for, such as `https://example.com`.
 * @property {string} sealed The user name and password, sealed and written in base64.
 * @property {number} saved When it was kept, in milliseconds since the epoch.
 */

/**
 * A login opened from the file.
 * @typedef {object} Login
 * @property {string} username The user name, or an empty string.
 * @property {string} password The password.
 */

/**
 * The origin of a web address, the key logins are kept under.
 * @param {string} url The address.
 * @returns {string} Its origin, or an empty string when it is not an `http` or `https` address.
 */
function originOf(url) {
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.origin : '';
  } catch (_) {
    return '';
  }
}

/**
 * The logins kept in one file, opened with one cipher.
 */
class Vault {
  /**
   * Reads the file, when there is one.
   * @param {string} file Where the logins are kept.
   * @param {Cipher} cipher Seals and opens them.
   */
  constructor(file, cipher) {
    /** @type {string} Where the logins are kept. */
    this.file = file;
    /** @type {Cipher} Seals and opens them. */
    this.cipher = cipher;
    /** @type {KeptLogin[]} The logins, as the file holds them. */
    this.logins = [];
    /** @type {Set<string>} The origins never to offer for. */
    this.never = new Set();
    this.read();
  }

  /**
   * Takes in what the file holds, keeping only entries of the right shape; a missing or broken file is an empty vault.
   * @returns {void}
   */
  read() {
    try {
      const kept = JSON.parse(fs.readFileSync(this.file, 'utf8'));
      this.logins = (Array.isArray(kept.logins) ? kept.logins : []).filter(
        /** @param {any} login A kept entry. */
        (login) => login && typeof login.origin === 'string' && typeof login.sealed === 'string',
      );
      this.never = new Set((Array.isArray(kept.never) ? kept.never : []).filter(
        /** @param {unknown} origin A kept origin. */
        (origin) => typeof origin === 'string',
      ));
    } catch (_) {
      this.logins = [];
      this.never = new Set();
    }
  }

  /**
   * Writes the vault to its file whole, readable by its owner alone.
   * @returns {void}
   * @throws {Error} When the file cannot be written.
   */
  write() {
    fs.mkdirSync(path.dirname(this.file), { recursive: true });
    const part = `${this.file}.part`;
    fs.writeFileSync(part, JSON.stringify({ version: 1, logins: this.logins, never: [...this.never].sort() }), { mode: 0o600 });
    fs.renameSync(part, this.file);
  }

  /**
   * Opens one kept login.
   * @param {KeptLogin} login The login as the file holds it.
   * @returns {Promise<Login | null>} The login, or nothing when it no longer opens, as after the keychain's key changed.
   */
  async open(login) {
    try {
      const opened = JSON.parse(await this.cipher.open(Buffer.from(login.sealed, 'base64')));
      return typeof opened.password === 'string' ? { username: String(opened.username || ''), password: opened.password } : null;
    } catch (_) {
      return null;
    }
  }

  /**
   * The logins kept for an origin, opened; one that no longer opens is left out.
   * @param {string} origin The origin.
   * @returns {Promise<Login[]>} Its logins, in the order they were kept.
   */
  async loginsOf(origin) {
    const opened = await Promise.all(this.logins.filter((login) => login.origin === origin).map((login) => this.open(login)));
    return opened.filter(
      /** @param {Login | null} login An opened login. */
      (login) => login !== null,
    );
  }

  /**
   * Whether an origin has any login kept, without opening them.
   * @param {string} origin The origin.
   * @returns {boolean} Whether it has.
   */
  has(origin) {
    return this.logins.some((login) => login.origin === origin);
  }

  /**
   * What keeping a login would do.
   * @param {string} origin The origin.
   * @param {Login} login The login.
   * @returns {Promise<'new' | 'update' | 'same'>} `same` when it is kept as it is, `update` when its user name is kept with another password, `new` otherwise.
   */
  async compare(origin, login) {
    const known = (await this.loginsOf(origin)).find((kept) => kept.username === login.username);
    if (!known) return 'new';
    return known.password === login.password ? 'same' : 'update';
  }

  /**
   * Keeps a login, in place of one with the same user name, and writes the file.
   * @param {string} origin The origin.
   * @param {Login} login The login.
   * @param {number} now The time now, in milliseconds since the epoch.
   * @returns {Promise<void>} Resolves once the file is written.
   * @throws {Error} When it cannot be sealed or written.
   */
  async save(origin, login, now) {
    const sealed = (await this.cipher.seal(JSON.stringify({ username: login.username, password: login.password }))).toString('base64');
    const opened = await Promise.all(this.logins.map((kept) => (kept.origin === origin ? this.open(kept) : Promise.resolve(null))));
    this.logins = this.logins.filter((_kept, at) => opened[at]?.username !== login.username);
    this.logins.push({ origin, sealed, saved: now });
    this.never.delete(origin);
    this.write();
  }

  /**
   * Never offers to keep logins for an origin again, and writes the file.
   * @param {string} origin The origin.
   * @returns {void}
   * @throws {Error} When the file cannot be written.
   */
  refuse(origin) {
    this.never.add(origin);
    this.write();
  }
}

module.exports = { Vault, originOf };
