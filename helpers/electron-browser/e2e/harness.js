'use strict';

/**
 * Starts the real browser helper the way the window does (`Engine::start` in `crates/pm-browser/src/engine.rs`): a
 * loopback listener, a random token in `PANDEMONIUM_BROWSER_TOKEN`, `--control`, `--surfaces` and `--profile`, the
 * token as the first line, then `Features` and `Ready`. It then speaks the line protocol of
 * `crates/pm-browser/src/wire.rs` with it, and reads the surface files its pages paint into.
 */

const { execFileSync, spawn } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { LineReader } = require('../src/lines');
const { GRACE } = require('../src/restriction');

/** The folder of the helper, which holds its launcher. */
const HELPER_FOLDER = path.resolve(__dirname, '..');

/** How many bytes of a surface file come before the pixels, as `HEADER` in `src/surface.js`. */
const SURFACE_HEADER = 16;

/** How long a wait for a report lasts before it fails, in milliseconds, unless the caller gives another. */
const WAIT_MS = 15000;

/**
 * The folder scratch files of the tests go in: `PANDEMONIUM_E2E_SCRATCH`, or `~/.cache/scratch/pandemonium`, never
 * a RAM-backed temporary folder.
 * @returns {string} A new, empty folder of its own.
 */
function scratchFolder() {
  const root = process.env.PANDEMONIUM_E2E_SCRATCH || path.join(os.homedir(), '.cache', 'scratch', 'pandemonium');
  fs.mkdirSync(root, { recursive: true });
  return fs.mkdtempSync(path.join(root, 'e2e-'));
}

/**
 * The program that starts the helper and the arguments before the window's: `PANDEMONIUM_BROWSER_HELPER` when set,
 * otherwise the launcher of this checkout. `PANDEMONIUM_E2E_ARGS` adds engine switches, such as
 * `--ozone-platform=headless`.
 * @returns {{ program: string, args: string[] }} What to run.
 */
function launcher() {
  const extra = (process.env.PANDEMONIUM_E2E_ARGS || '').split(/\s+/).filter(Boolean);
  const program = process.env.PANDEMONIUM_BROWSER_HELPER || path.join(HELPER_FOLDER, 'pandemonium-browser-electron');
  return { program, args: extra };
}

/**
 * A report of the helper, as parsed from its line.
 * @typedef {string | Record<string, any>} Report
 */

/**
 * The name of a report: its one key, or the report itself when it is a bare word such as `Ready`.
 * @param {Report} report The report.
 * @returns {string} Its name.
 */
function nameOf(report) {
  return typeof report === 'string' ? report : Object.keys(report)[0];
}

/**
 * The body of a report: the value under its one key, or an empty object for a bare word.
 * @param {Report} report The report.
 * @returns {Record<string, any>} Its body.
 */
function bodyOf(report) {
  return typeof report === 'string' ? {} : Object.values(report)[0];
}

/**
 * Resolves after `ms` milliseconds.
 * @param {number} ms How long to wait.
 * @returns {Promise<void>} Settles when the time is up.
 */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * A running helper and the connection to it.
 */
class Helper {
  /**
   * Starts the helper, waits for its token and its `Ready`.
   * @returns {Promise<Helper>} The connected helper.
   * @throws {Error} When the helper exits, sends a wrong token or does not get ready within 30 seconds.
   */
  static async start() {
    const helper = new Helper();
    await helper.launch();
    return helper;
  }

  /** A helper not yet started; use {@link Helper.start}. */
  constructor() {
    /** @type {string} Scratch folder holding the surfaces, the profile and the downloads. */
    this.folder = scratchFolder();
    /** @type {string} The folder the helper paints into. */
    this.surfaces = path.join(this.folder, 'surfaces');
    /** @type {string} The engine's profile. */
    this.profile = path.join(this.folder, 'profile');
    /** @type {string} The home folder the helper sees, which holds its Downloads folder. */
    this.home = path.join(this.folder, 'home');
    /** @type {Report[]} Every report received, in order. */
    this.reports = [];
    /** @type {{ at: number, report: Report }[]} Every report with the time it arrived. */
    this.timed = [];
    /** @type {Set<() => void>} Waiters to wake on each report. */
    this.waiters = new Set();
    /** @type {net.Socket | null} The connection. */
    this.socket = null;
    /** @type {import('node:child_process').ChildProcess | null} The helper's process. */
    this.child = null;
    /** @type {string} What the helper wrote on its error stream, for failures. */
    this.log = '';
    /** @type {number} When the last agent tool answered, in milliseconds since the epoch. */
    this.toolEnded = 0;
    /** @type {number} The number of the last tool call. */
    this.calls = 0;
    /** @type {boolean} Whether the helper's process ended. */
    this.exited = false;
  }

  /**
   * Listens, starts the process and takes the connection that proves itself with the token.
   * @returns {Promise<void>} Settles once `Ready` arrived.
   * @throws {Error} When the helper does not get ready.
   */
  async launch() {
    for (const folder of [this.surfaces, this.profile, path.join(this.home, 'Downloads')]) fs.mkdirSync(folder, { recursive: true, mode: 0o700 });
    const token = crypto.randomBytes(16).toString('hex');
    const server = net.createServer();
    await new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(undefined)));
    const address = /** @type {net.AddressInfo} */ (server.address());
    const { program, args } = launcher();
    this.child = spawn(
      program,
      [...args, '--control', `127.0.0.1:${address.port}`, '--surfaces', this.surfaces, '--profile', this.profile],
      {
        env: { ...process.env, PANDEMONIUM_BROWSER_TOKEN: token, HOME: this.home, XDG_CONFIG_HOME: path.join(this.home, '.config') },
        stdio: ['ignore', 'ignore', 'pipe'],
      },
    );
    this.child.stderr?.on('data', (chunk) => {
      this.log = (this.log + chunk.toString()).slice(-20000);
    });
    this.child.on('exit', () => {
      this.exited = true;
      this.wake();
    });
    const lines = new LineReader();
    let proven = false;
    /** @type {NodeJS.Timeout | undefined} */
    let timer;
    const connected = new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error(`the helper did not connect in time\n${this.log}`)), 30000);
      server.on('connection', (socket) => {
        socket.on('data', (chunk) => {
          for (const text of lines.push(chunk)) {
            if (!proven) {
              if (text.trim() !== token) {
                socket.destroy();
                return;
              }
              proven = true;
              this.socket = socket;
              resolve(undefined);
              continue;
            }
            if (!text.trim()) continue;
            const report = JSON.parse(text);
            this.reports.push(report);
            this.timed.push({ at: performance.now(), report });
            this.wake();
          }
        });
      });
      this.child?.on('exit', () => reject(new Error(`the helper exited while starting\n${this.log}`)));
    });
    try {
      await connected;
      await this.waitFor((report) => report === 'Ready', 30000);
    } catch (error) {
      this.child.kill('SIGKILL');
      fs.rmSync(this.folder, { recursive: true, force: true });
      throw error;
    } finally {
      clearTimeout(timer);
      server.close();
    }
  }

  /** Wakes every waiter to look at the reports again. */
  wake() {
    for (const waiter of [...this.waiters]) waiter();
  }

  /**
   * Sends one request as a line of JSON.
   * @param {Report} request The request, such as `{ Open: { ... } }`.
   * @returns {void}
   */
  send(request) {
    this.socket?.write(`${JSON.stringify(request)}\n`);
  }

  /**
   * Waits for a report received at or after index `from` that `match` accepts.
   * @param {(report: Report) => boolean} match Which report.
   * @param {number} [timeout] How long to wait, in milliseconds.
   * @param {number} [from] The first index of {@link Helper#reports} to look at; 0 by default.
   * @returns {Promise<Report>} The report.
   * @throws {Error} When none arrives in time or the helper exits.
   */
  waitFor(match, timeout = WAIT_MS, from = 0) {
    return new Promise((resolve, reject) => {
      let index = from;
      const look = () => {
        for (; index < this.reports.length; index += 1) {
          if (match(this.reports[index])) {
            finish();
            resolve(this.reports[index]);
            return;
          }
        }
        if (this.exited) {
          finish();
          reject(new Error(`the helper exited\n${this.log.slice(-4000)}`));
        }
      };
      const timer = setTimeout(() => {
        finish();
        reject(new Error(`timed out; last reports: ${JSON.stringify(this.reports.slice(-8)).slice(0, 2000)}`));
      }, timeout);
      const finish = () => {
        clearTimeout(timer);
        this.waiters.delete(look);
      };
      this.waiters.add(look);
      look();
    });
  }

  /**
   * Waits for a report named `name` about `page` that `match` accepts, received after index `from`.
   * @param {string} name The report's name, such as `Loading`.
   * @param {number | null} page The page, or nothing for reports about no page.
   * @param {(body: Record<string, any>) => boolean} [match] What else its body must hold.
   * @param {{ from?: number, timeout?: number }} [options] Where to start looking and how long to wait.
   * @returns {Promise<Record<string, any>>} The report's body.
   */
  async waitReport(name, page, match = () => true, options = {}) {
    const report = await this.waitFor(
      (r) => nameOf(r) === name && (page === null || bodyOf(r).page === page) && match(bodyOf(r)),
      options.timeout,
      options.from ?? 0,
    );
    return bodyOf(report);
  }

  /**
   * The index the next report will have, to wait only for what comes after now.
   * @returns {number} The count of reports so far.
   */
  mark() {
    return this.reports.length;
  }

  /**
   * Opens a page on `url` and waits until it finished loading and painted.
   * @param {number} page The page's number.
   * @param {string} url The address.
   * @param {{ width?: number, height?: number, scale?: number, partition?: string }} [size] Its size in logical pixels, its display scale and the profile it lives in, the shared one when not given.
   * @returns {Promise<void>} Settles once loaded and painted.
   */
  async open(page, url, size = {}) {
    const from = this.mark();
    /** @type {Record<string, unknown>} */
    const body = { page, url, width: size.width ?? 800, height: size.height ?? 600, scale: size.scale ?? 1 };
    if (size.partition) body.partition = size.partition;
    this.send({ Open: body });
    this.send({ Focus: { page, focused: true } });
    await this.waitReport('Loading', page, (b) => b.loading === false, { from });
    await this.waitReport('Painted', page, () => true, { from });
  }

  /**
   * Loads `url` in `page` and waits for the load to finish.
   * @param {number} page The page.
   * @param {string} url The address.
   * @returns {Promise<void>} Settles once loaded.
   */
  async load(page, url) {
    const from = this.mark();
    this.send({ Load: { page, url } });
    await this.waitReport('Address', page, (b) => b.url === url, { from });
    await this.waitReport('Loading', page, (b) => b.loading === false, { from });
  }

  /**
   * Runs an agent tool on `page` and waits for its result.
   * @param {number} page The page.
   * @param {string} tool The tool's name, such as `browser_evaluate`.
   * @param {Record<string, unknown>} [args] Its arguments.
   * @param {number} [timeout] How long to wait, in milliseconds.
   * @returns {Promise<{ ok: boolean, text: string, image?: string | null }>} The result.
   */
  async tool(page, tool, args = {}, timeout = WAIT_MS) {
    return this.toolAs(page, null, tool, args, timeout);
  }

  /**
   * Runs an agent tool on `page` as the agent session `caller` names, and waits for its result.
   * @param {number} page The page.
   * @param {{ session: string, name: string } | null} caller The session that calls, or nothing to name none.
   * @param {string} tool The tool's name, such as `browser_evaluate`.
   * @param {Record<string, unknown>} [args] Its arguments.
   * @param {number} [timeout] How long to wait, in milliseconds.
   * @returns {Promise<{ ok: boolean, text: string, image?: string | null, step?: Record<string, any> }>} The result.
   */
  async toolAs(page, caller, tool, args = {}, timeout = WAIT_MS) {
    this.calls += 1;
    const call = this.calls;
    /** @type {Record<string, unknown>} */
    const body = { page, call, tool, arguments: JSON.stringify(args) };
    if (caller) Object.assign(body, { session: caller.session, session_name: caller.name });
    this.send({ Tool: body });
    try {
      const done = await this.waitReport('ToolDone', page, (b) => b.result.call === call, { timeout });
      return done.result;
    } finally {
      this.toolEnded = Date.now();
    }
  }

  /**
   * Waits until no agent tool ran for longer than the helper's grace (`GRACE` in `src/restriction.js`), so what the
   * page does next counts as the reader's doing, not an agent's.
   * @returns {Promise<void>} Settles once the grace is over.
   */
  async readerTurn() {
    const left = (this.toolEnded ?? 0) + GRACE + 200 - Date.now();
    if (left > 0) await sleep(left);
  }

  /**
   * Evaluates `expression` in the top frame of `page` through the agent tool and returns what it gave, as text.
   * @param {number} page The page.
   * @param {string} expression JavaScript whose value is wanted.
   * @returns {Promise<string>} The value as the tool printed it.
   * @throws {Error} When the tool failed.
   */
  async evaluate(page, expression) {
    const result = await this.tool(page, 'browser_evaluate', { expression });
    if (!result.ok) throw new Error(`browser_evaluate failed: ${result.text}`);
    return result.text;
  }

  /**
   * Polls `expression` in `page` until its JSON value equals `expected`.
   * @param {number} page The page.
   * @param {string} expression JavaScript to evaluate.
   * @param {unknown} expected The value wanted.
   * @param {number} [timeout] How long to try, in milliseconds.
   * @returns {Promise<void>} Settles once the value matches.
   * @throws {Error} With the last value when it never matches.
   */
  async until(page, expression, expected, timeout = WAIT_MS) {
    const want = JSON.stringify(expected);
    const end = Date.now() + timeout;
    let last = '';
    while (Date.now() < end) {
      last = await this.evaluate(page, `JSON.stringify(${expression})`);
      if (unquote(last) === want) return;
      await sleep(100);
    }
    throw new Error(`${expression} stayed ${last}, wanted ${want}`);
  }

  /**
   * Whether `page` painted into the surface it announced last.
   * @param {number} page The page.
   * @returns {boolean} Whether a `Painted` report followed its last `Surface` report.
   */
  painted(page) {
    const at = this.reports.findLastIndex((r) => nameOf(r) === 'Surface' && bodyOf(r).page === page);
    return at >= 0 && this.reports.slice(at).some((r) => nameOf(r) === 'Painted' && bodyOf(r).page === page);
  }

  /**
   * The newest picture `page` painted, read from its surface file as the window reads it.
   * @param {number} page The page.
   * @returns {{ width: number, height: number, pixels: Buffer }} Its size in device pixels and its BGRA pixels.
   * @throws {Error} When the page announced no surface or painted nothing.
   */
  picture(page) {
    const surface = [...this.reports].reverse().find((r) => nameOf(r) === 'Surface' && bodyOf(r).page === page);
    if (!surface) throw new Error(`page ${page} announced no surface`);
    const { width, height, paths } = bodyOf(surface);
    const at = this.reports.lastIndexOf(surface);
    const painted = this.reports.slice(at).reverse().find((r) => nameOf(r) === 'Painted' && bodyOf(r).page === page);
    if (!painted) throw new Error(`page ${page} painted nothing into its surface`);
    const bytes = fs.readFileSync(paths[bodyOf(painted).buffer]);
    return { width, height, pixels: bytes.subarray(SURFACE_HEADER, SURFACE_HEADER + width * height * 4) };
  }

  /**
   * Kills every renderer process the helper started, as a renderer crash would end them.
   * @returns {number} How many were killed.
   */
  killRenderers() {
    const table = execFileSync('ps', ['-A', '-o', 'pid=,ppid=,command='], { encoding: 'utf8' })
      .split('\n')
      .map((line) => line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/))
      .filter((match) => match !== null)
      .map((match) => ({ pid: Number(match[1]), parent: Number(match[2]), command: match[3] }));
    const ours = new Set([this.child?.pid]);
    for (let grown = true; grown; ) {
      grown = false;
      for (const entry of table) {
        if (!ours.has(entry.pid) && ours.has(entry.parent)) {
          ours.add(entry.pid);
          grown = true;
        }
      }
    }
    const renderers = table.filter((p) => ours.has(p.pid) && p.command.includes('--type=renderer'));
    for (const renderer of renderers) process.kill(renderer.pid, 'SIGKILL');
    return renderers.length;
  }

  /**
   * Asks the helper to end and waits for it, killing it after three seconds, then removes the scratch folder.
   * @returns {Promise<void>} Settles once it ended.
   */
  async stop() {
    this.send('Shutdown');
    const end = Date.now() + 3000;
    while (!this.exited && Date.now() < end) await sleep(20);
    if (!this.exited) this.child?.kill('SIGKILL');
    this.socket?.destroy();
    fs.rmSync(this.folder, { recursive: true, force: true });
  }
}

/**
 * The text of a tool's answer with the quotes JSON gave a string taken off, when it has them.
 * @param {string} text What the tool answered.
 * @returns {string} The text inside.
 */
function unquote(text) {
  const trimmed = text.trim();
  try {
    const value = JSON.parse(trimmed);
    return typeof value === 'string' ? value : trimmed;
  } catch (_) {
    return trimmed;
  }
}

/**
 * How many distinct colours a picture holds, up to `limit`, to tell a painted page from an empty one.
 * @param {Buffer} pixels BGRA pixels.
 * @param {number} [limit] When to stop counting.
 * @returns {number} The count of colours seen.
 */
function colours(pixels, limit = 16) {
  const seen = new Set();
  for (let at = 0; at + 4 <= pixels.length && seen.size < limit; at += 4 * 97) seen.add(pixels.readUInt32LE(at));
  return seen.size;
}

module.exports = { Helper, bodyOf, colours, nameOf, sleep, unquote };
