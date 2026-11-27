'use strict';

/**
 * End-to-end tests of saved logins with the real helper, under both ways they are sealed: the system's keychain, and
 * the primary password used where `safeStorage` would fall back to its fixed key, which the engine's switch
 * `--password-store=basic` forces on Linux. A login is sent by the reader, offered, saved, and after a restart of the
 * helper filled in again; with a primary password the restarted helper is locked until the reader gives it.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { after, before, test } = require('node:test');
const { Helper } = require('./harness');

/** The page with a login form, its fields and button at known places. */
const LOGIN_PAGE = `<!doctype html><html><head><title>sign in</title></head><body style="margin:0">
<form action="/welcome" method="get">
<input id="user" name="user" autocomplete="username" style="position:absolute;left:10px;top:10px;width:200px;height:30px">
<input id="pass" name="pass" type="password" style="position:absolute;left:10px;top:60px;width:200px;height:30px">
<button id="go" style="position:absolute;left:10px;top:110px;width:100px;height:30px">go</button>
</form></body></html>`;

/** @type {http.Server} */
let server;
/** @type {string} The origin the fixture is served on. */
let origin = '';

before(async () => {
  server = http.createServer((request, response) => {
    response.writeHead(200, { 'content-type': 'text/html' });
    response.end(request.url?.startsWith('/welcome') ? '<!doctype html><title>welcome</title>welcome' : LOGIN_PAGE);
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(undefined)));
  origin = `http://127.0.0.1:${/** @type {import('node:net').AddressInfo} */ (server.address()).port}`;
});

after(() => server?.close());

/**
 * Starts a helper with extra engine switches, its profile seeded with files kept from an earlier run.
 * @param {string[]} switches Engine switches, such as `--password-store=basic`.
 * @param {Record<string, string>} [kept] Files to put in its profile before it starts, by name.
 * @returns {Promise<Helper>} The helper, with saving passwords on.
 */
async function start(switches, kept = {}) {
  const before = process.env.PANDEMONIUM_E2E_ARGS;
  process.env.PANDEMONIUM_E2E_ARGS = [before || '', ...switches].join(' ');
  const helper = new Helper();
  fs.mkdirSync(helper.profile, { recursive: true, mode: 0o700 });
  for (const [name, text] of Object.entries(kept)) fs.writeFileSync(path.join(helper.profile, name), text, { mode: 0o600 });
  try {
    await helper.launch();
  } finally {
    if (before === undefined) delete process.env.PANDEMONIUM_E2E_ARGS;
    else process.env.PANDEMONIUM_E2E_ARGS = before;
  }
  helper.send({ Privacy: { block_trackers: false, save_passwords: true } });
  for (let tries = 0; tries < 100 && !/saved logins are sealed/.test(helper.log); tries += 1) await new Promise((resolve) => setTimeout(resolve, 100));
  return helper;
}

/**
 * Clicks at a point of a page, as the reader would.
 * @param {Helper} helper The helper.
 * @param {number} page The page.
 * @param {number} x Logical pixels from the left.
 * @param {number} y Logical pixels from the top.
 * @returns {void}
 */
function click(helper, page, x, y) {
  helper.send({ PointerMoved: { page, x, y, modifiers: 0, left: false } });
  helper.send({ PointerButton: { page, x, y, button: 'Left', pressed: true, clicks: 1, modifiers: 16 } });
  helper.send({ PointerButton: { page, x, y, button: 'Left', pressed: false, clicks: 1, modifiers: 0 } });
}

/**
 * Waits for a `Chrome` report of a page whose report is named `name`.
 * @param {Helper} helper The helper.
 * @param {string} name The report inside, such as `OfferPassword`.
 * @param {number} from The first report to look at.
 * @returns {Promise<any>} The report inside; `{}` for a bare word.
 */
async function chromeReport(helper, name, from) {
  const body = await helper.waitReport(
    'Chrome',
    1,
    (b) => b.report === name || (typeof b.report === 'object' && name in b.report),
    { from },
  );
  return typeof body.report === 'string' ? {} : body.report[name];
}

/**
 * Sends a chrome action for page 1.
 * @param {Helper} helper The helper.
 * @param {any} action The `PageAction`.
 * @returns {void}
 */
function act(helper, action) {
  helper.send({ Chrome: { page: 1, action } });
}

/**
 * Has the reader send a login from the form, and waits for the offer to save it.
 * @param {Helper} helper The helper.
 * @returns {Promise<any>} The offer.
 */
async function sendLogin(helper) {
  await helper.open(1, `${origin}/sign-in`);
  await helper.evaluate(1, `(document.getElementById('user').value = 'ada', document.getElementById('pass').value = 'secret-1', 'ok')`);
  await helper.readerTurn();
  const from = helper.mark();
  click(helper, 1, 40, 125);
  return chromeReport(helper, 'OfferPassword', from);
}

/**
 * Has the reader focus the user name field of the form, after a fresh load.
 * @param {Helper} helper The helper.
 * @returns {Promise<number>} The mark before the click.
 */
async function focusUser(helper) {
  await helper.open(1, `${origin}/sign-in`);
  await helper.readerTurn();
  const from = helper.mark();
  click(helper, 1, 40, 25);
  return from;
}

/**
 * Picks the saved login of `ada` from the list and waits until the form holds it.
 * @param {Helper} helper The helper.
 * @returns {Promise<void>} Settles once it is filled.
 */
async function fillAda(helper) {
  act(helper, { FillLogin: { username: 'ada' } });
  await new Promise((resolve) => setTimeout(resolve, 300));
  await helper.until(1, `[document.getElementById('user').value, document.getElementById('pass').value]`, ['ada', 'secret-1']);
}

test('with the fallback key forced, logins are sealed by a primary password that a restart locks again', { skip: process.platform !== 'linux' && 'only Linux has a fallback key to force' }, async () => {
  const first = await start(['--password-store=basic']);
  /** @type {Record<string, string>} */
  const kept = {};
  try {
    assert.match(first.log, /sealed by a primary password \(the keychain's backend is basic_text\)/);
    const offer = await sendLogin(first);
    assert.equal(offer.username, 'ada');
    assert.equal(offer.origin, origin);
    let from = first.mark();
    act(first, { AnswerPassword: { id: offer.id, save: true, never: false } });
    const create = await chromeReport(first, 'PrimaryPassword', from);
    assert.equal(create.create, true);
    from = first.mark();
    act(first, { PrimaryPassword: { password: 'short' } });
    const refused = await chromeReport(first, 'PrimaryPassword', from);
    assert.match(refused.problem, /at least 8/);
    from = first.mark();
    act(first, { PrimaryPassword: { password: 'correct horse' } });
    await chromeReport(first, 'PrimaryPasswordClosed', from);
    const vaultFile = path.join(first.profile, 'passwords-primary.json');
    for (let tries = 0; tries < 50 && !fs.existsSync(vaultFile); tries += 1) await new Promise((resolve) => setTimeout(resolve, 100));
    const text = fs.readFileSync(vaultFile, 'utf8');
    assert.ok(text.includes(origin) && !text.includes('secret-1') && !text.includes('"ada"'));
    assert.equal(fs.existsSync(path.join(first.profile, 'passwords.json')), false);
    const fromField = await focusUser(first);
    const field = await chromeReport(first, 'LoginField', fromField);
    assert.deepEqual(field.usernames, ['ada']);
    for (const name of ['passwords-primary.json', 'passwords-primary.key.json']) kept[name] = fs.readFileSync(path.join(first.profile, name), 'utf8');
  } finally {
    await first.stop();
  }

  const second = await start(['--password-store=basic'], kept);
  try {
    let from = await focusUser(second);
    const unlock = await chromeReport(second, 'PrimaryPassword', from);
    assert.equal(unlock.create, false);
    assert.equal(unlock.origin, origin);
    from = second.mark();
    act(second, { PrimaryPassword: { password: 'wrong horse' } });
    const wrong = await chromeReport(second, 'PrimaryPassword', from);
    assert.match(wrong.problem, /not the primary password/);
    from = second.mark();
    act(second, { PrimaryPassword: { password: 'correct horse' } });
    await chromeReport(second, 'PrimaryPasswordClosed', from);
    const field = await chromeReport(second, 'LoginField', from);
    assert.deepEqual(field.usernames, ['ada']);
    await fillAda(second);
  } finally {
    await second.stop();
  }
});

test('with a real keychain, logins are sealed by it and never ask for a primary password', async (t) => {
  const first = await start([]);
  /** @type {Record<string, string>} */
  const kept = {};
  try {
    if (!/sealed by the system's keychain/.test(first.log)) {
      t.skip(`no keychain here: ${first.log.match(/saved logins are sealed[^\n]*/)?.[0] ?? 'unknown backend'}`);
      return;
    }
    t.diagnostic(first.log.match(/saved logins are sealed[^\n]*/)?.[0] ?? '');
    const offer = await sendLogin(first);
    const from = first.mark();
    act(first, { AnswerPassword: { id: offer.id, save: true, never: false } });
    const vaultFile = path.join(first.profile, 'passwords.json');
    for (let tries = 0; tries < 100 && !fs.existsSync(vaultFile); tries += 1) await new Promise((resolve) => setTimeout(resolve, 100));
    assert.ok(fs.existsSync(vaultFile), first.log.slice(-2000));
    assert.ok(!fs.readFileSync(vaultFile, 'utf8').includes('secret-1'));
    assert.ok(!first.reports.slice(from).some((r) => r.Chrome?.report?.PrimaryPassword));
    kept['passwords.json'] = fs.readFileSync(vaultFile, 'utf8');
  } finally {
    await first.stop();
  }
  if (!kept['passwords.json']) return;
  const second = await start([], kept);
  try {
    const from = await focusUser(second);
    const field = await chromeReport(second, 'LoginField', from);
    assert.deepEqual(field.usernames, ['ada']);
    await fillAda(second);
    assert.ok(!second.reports.some((r) => r.Chrome?.report?.PrimaryPassword));
  } finally {
    await second.stop();
  }
});
