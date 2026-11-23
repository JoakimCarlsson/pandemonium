'use strict';

/**
 * End-to-end tests of what is kept per agent session and per profile: each session's own recording, secrets kept out
 * of recordings and blurred in their screenshots, `browser_tabs` steps the window hands over, and the reader's answers
 * to permissions kept per profile and origin, with the private profile's answers gone once it closes.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { after, before, test } = require('node:test');
const { Helper } = require('./harness');

/**
 * The login page: a user name, a password, a one-time code, and a watcher that notes which fields were ever blurred,
 * so the test can tell what the recording's screenshots hid.
 */
const LOGIN_PAGE = `<!doctype html><title>Login</title>
<input id="user" style="font-size:40px" value="">
<input id="pw" type="password" style="font-size:40px">
<input id="otp" autocomplete="one-time-code" style="font-size:40px">
<script>
window.blurred = [];
new MutationObserver((changes) => {
  for (const change of changes) {
    const el = change.target;
    if (String(el.style.filter).includes('blur') && !window.blurred.includes(el.id)) window.blurred.push(el.id);
  }
}).observe(document.body, { attributes: true, attributeFilter: ['style'], subtree: true });
</script>`;

/** The page that asks to show notifications once no agent has acted on it for a while, and shows the answer as its title. */
const ASK_PAGE = `<!doctype html><title>asking</title>
<script>
setTimeout(() => Notification.requestPermission().then((answer) => { document.title = answer; }), 300);
</script>`;

/** @type {Helper} */
let helper;
/** @type {http.Server} */
let server;
/** @type {string} */
let origin = '';

/** The folder this file's recordings go in. */
const SCRATCH = process.env.PANDEMONIUM_E2E_SCRATCH || path.join(os.homedir(), '.cache', 'scratch', 'pandemonium');

/**
 * Serves the login page and the page that asks for a permission.
 * @param {http.IncomingMessage} request The request.
 * @param {http.ServerResponse} response The response.
 * @returns {void}
 */
function serve(request, response) {
  const url = new URL(request.url || '/', 'http://fixture');
  const pages = /** @type {Record<string, string>} */ ({ '/login': LOGIN_PAGE, '/ask': ASK_PAGE });
  const page = pages[url.pathname];
  if (page) {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(page);
  } else {
    response.writeHead(404);
    response.end();
  }
}

before(async () => {
  fs.mkdirSync(SCRATCH, { recursive: true });
  server = http.createServer(serve);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(undefined)));
  origin = `http://127.0.0.1:${/** @type {any} */ (server.address()).port}`;
  helper = await Helper.start();
  await helper.open(1, `${origin}/login`, { width: 600, height: 400, scale: 1 });
});

after(async () => {
  await helper?.stop();
  server?.closeAllConnections();
  await new Promise((resolve) => server.close(() => resolve(undefined)));
});

test('each session records alone, secrets are redacted and blurred, and browser_tabs steps are kept', async () => {
  const ada = { session: 'session-1', name: 'Fix the sign-in form' };
  const bob = { session: 'session-2', name: 'Check the console' };
  const folder = fs.mkdtempSync(path.join(SCRATCH, 'sessions-'));
  const started = await helper.toolAs(1, ada, 'browser_record', { action: 'start', folder });
  assert.ok(started.ok, started.text);
  const unrecorded = await helper.toolAs(1, bob, 'browser_evaluate', { expression: '1' });
  assert.equal(unrecorded.step, undefined);
  assert.match((await helper.toolAs(1, bob, 'browser_record', { action: 'status' })).text, /No recording of this session/);
  const filled = await helper.toolAs(1, ada, 'browser_fill_form', {
    fields: [
      { selector: '#user', value: 'ada@example.com' },
      { selector: '#pw', value: 'hunter2-password' },
    ],
  });
  assert.ok(filled.ok, filled.text);
  const step = /** @type {Record<string, any>} */ (filled.step);
  assert.equal(step.session, 'session-1');
  assert.equal(step.session_name, 'Fix the sign-in form');
  assert.match(step.arguments, /ada@example\.com/);
  assert.doesNotMatch(step.arguments, /hunter2/);
  assert.ok(fs.statSync(step.picture).size > 100);
  const typed = await helper.toolAs(1, ada, 'browser_type', { selector: '#otp', text: '424242' });
  assert.ok(typed.ok, typed.text);
  assert.doesNotMatch(/** @type {Record<string, any>} */ (typed.step).arguments, /424242/);
  const shown = await helper.toolAs(1, ada, 'browser_evaluate', { expression: "document.getElementById('pw').type = 'text', 1" });
  assert.ok(shown.step);
  const blurred = await helper.toolAs(1, bob, 'browser_evaluate', {
    expression: "JSON.stringify({ blurred: window.blurred, now: ['user', 'pw', 'otp'].map((id) => getComputedStyle(document.getElementById(id)).filter) })",
  });
  const seen = JSON.parse(JSON.parse(blurred.text));
  assert.deepEqual([...seen.blurred].sort(), ['otp', 'pw']);
  assert.deepEqual(seen.now, ['none', 'none', 'none']);
  helper.calls += 1;
  const call = helper.calls;
  helper.send({ RecordStep: { page: 1, call, tool: 'browser_tabs', arguments: '{"action":"list"}', ok: true, text: 'Tab 1: Login', session: ada.session, session_name: ada.name } });
  const tabs = await helper.waitReport('ToolDone', 1, (body) => body.result.call === call);
  assert.equal(tabs.result.step.tool, 'browser_tabs');
  assert.equal(tabs.result.step.session, 'session-1');
  assert.ok(fs.statSync(tabs.result.step.picture).size > 100);
  helper.send({ RecordStep: { page: 1, call: call + 1000, tool: 'browser_tabs', arguments: '{}', ok: true, text: 'x', session: bob.session } });
  const stopped = await helper.toolAs(1, ada, 'browser_record', { action: 'stop' });
  assert.ok(stopped.ok, stopped.text);
  assert.match(stopped.text, /Session: Fix the sign-in form/);
  assert.match(stopped.text, /`browser_tabs` \{"action":"list"\}/);
  const kept = fs.readFileSync(path.join(folder, 'steps.json'), 'utf8') + fs.readFileSync(path.join(folder, 'summary.md'), 'utf8');
  assert.doesNotMatch(kept, /hunter2|424242/);
  assert.equal(JSON.parse(fs.readFileSync(path.join(folder, 'steps.json'), 'utf8')).length, 4);
  assert.ok(!helper.reports.some((report) => JSON.stringify(report).includes(`"call":${call + 1000}`)));
  fs.rmSync(folder, { recursive: true, force: true });
});

/**
 * Opens a page that asks for notifications in `partition` and answers the question it asks with `allow`, or expects
 * none to be asked when `allow` is nothing.
 * @param {number} page The page's number.
 * @param {string} partition The profile; empty for the shared one.
 * @param {boolean | null} allow The answer to give, or nothing when the page must not ask.
 * @returns {Promise<string>} The title the page ends with: what the permission came to.
 */
async function ask(page, partition, allow) {
  const from = helper.mark();
  helper.send({ Open: { page, url: `${origin}/ask`, width: 300, height: 200, scale: 1, ...(partition ? { partition } : {}) } });
  const settled = helper.waitReport('Title', page, (body) => body.title === 'granted' || body.title === 'denied', { from });
  if (allow !== null) {
    const asked = await helper.waitReport('PermissionAsked', page, () => true, { from });
    helper.send({ PermissionAnswer: { page, id: asked.id, allow } });
  }
  const title = (await settled).title;
  const askedAnyway = helper.reports.slice(from).some((report) => 'PermissionAsked' in Object(report) && report.PermissionAsked.page === page);
  if (allow === null) assert.ok(!askedAnyway, `page ${page} in ${partition || 'the shared profile'} was asked again`);
  return title;
}

/**
 * Closes a page.
 * @param {number} page The page's number.
 * @returns {void}
 */
function close(page) {
  helper.send({ Close: { page } });
}

test('permission answers are kept per profile and origin, and the private profile forgets them when it closes', async () => {
  assert.equal(await ask(11, '', true), 'granted');
  close(11);
  assert.equal(await ask(12, '', null), 'granted');
  assert.equal(await ask(13, 'private', true), 'granted');
  assert.equal(await ask(14, 'persist:project-e2e', false), 'denied');
  assert.equal(await ask(15, 'private', null), 'granted');
  close(13);
  close(15);
  await new Promise((resolve) => setTimeout(resolve, 300));
  assert.equal(await ask(16, 'private', false), 'denied');
  assert.equal(await ask(17, 'persist:project-e2e', null), 'denied');
  assert.equal(await ask(18, '', null), 'granted');
  for (const page of [12, 14, 16, 17, 18]) close(page);
});
