'use strict';

/**
 * End-to-end tests of what agents read of a page's engine and of recordings: the console with times, sources, stacks
 * and a cursor, the network with kept bodies, type filters and waiting for a request, and `browser_record`.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { after, before, test } = require('node:test');
const { Helper } = require('./harness');

/** The page the tests work on: it logs, throws from a named function, and posts to the API when its button is pressed. */
const APP_PAGE = `<!doctype html><title>App</title>
<button id="send" style="width:200px;height:60px" onclick="fetch('/api/items', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: 'pen' }) })">send</button>
<script>
console.log('hello', 42);
function explode() { throw new Error('boom'); }
setTimeout(explode, 50);
fetch('/api/items?first=1').then((r) => r.json());
</script>`;

/** @type {Helper} */
let helper;
/** @type {http.Server} */
let server;
/** @type {string} */
let origin = '';

/**
 * Serves the page, a second page and the API, which answers after a short wait.
 * @param {http.IncomingMessage} request The request.
 * @param {http.ServerResponse} response The response.
 * @returns {void}
 */
function serve(request, response) {
  const url = new URL(request.url || '/', 'http://fixture');
  if (url.pathname === '/app') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(APP_PAGE);
  } else if (url.pathname === '/other') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end('<!doctype html><title>Other</title><p>other</p>');
  } else if (url.pathname === '/api/items') {
    let sent = '';
    request.on('data', (chunk) => {
      sent += chunk;
    });
    request.on('end', () =>
      setTimeout(() => {
        response.writeHead(200, { 'content-type': 'application/json' });
        response.end(JSON.stringify({ items: ['a', 'b'], method: request.method, sent }));
      }, 300),
    );
  } else {
    response.writeHead(404);
    response.end();
  }
}

before(async () => {
  server = http.createServer(serve);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(undefined)));
  origin = `http://127.0.0.1:${/** @type {any} */ (server.address()).port}`;
  helper = await Helper.start();
  await helper.open(1, `${origin}/app`, { width: 600, height: 400, scale: 1 });
});

after(async () => {
  await helper?.stop();
  server?.closeAllConnections();
  await new Promise((resolve) => server.close(() => resolve(undefined)));
});

test('the console has times, sources, uncaught stacks and a cursor', async () => {
  await helper.until(1, 'document.title', 'App');
  await new Promise((resolve) => setTimeout(resolve, 300));
  const all = await helper.tool(1, 'browser_console');
  assert.ok(all.ok, all.text);
  assert.match(all.text, /#\d+ \d\d:\d\d:\d\d\.\d{3} \[log\] hello 42 \(http:\/\/127\.0\.0\.1:\d+\/app:\d+:\d+\)/);
  assert.match(all.text, /\[error\] Uncaught Error: boom \(http:\/\/127\.0\.0\.1:\d+\/app:\d+:\d+\)\n {4}at explode \(/);
  const cursor = Number(/\[cursor: (\d+);/.exec(all.text)?.[1]);
  assert.ok(cursor >= 2, all.text);
  const none = await helper.tool(1, 'browser_console', { since: cursor });
  assert.match(none.text, /^No new messages\./);
  await helper.tool(1, 'browser_evaluate', { expression: "console.warn('later'), 1" });
  const newer = await helper.tool(1, 'browser_console', { since: cursor });
  assert.match(newer.text, /\[warning\] later/);
  assert.doesNotMatch(newer.text, /hello/);
});

test('bodies stay after navigation, types filter, and wait_for finds the request a click caused', async () => {
  const fetches = await helper.tool(1, 'browser_network', { type: 'fetch,xhr' });
  assert.match(fetches.text, /#(\d+) GET .*\/api\/items\?first=1 -> 200 \(fetch\)/);
  assert.doesNotMatch(fetches.text, /\(document\)/);
  const id = Number(/#(\d+) GET .*first=1/.exec(fetches.text)?.[1]);
  assert.ok(!(await helper.tool(1, 'browser_network', { type: 'pictures' })).ok);
  const clicked = await helper.tool(1, 'browser_click', { selector: '#send' });
  assert.ok(clicked.ok, clicked.text);
  const waited = await helper.tool(1, 'browser_network', { wait_for: '*/api/items', type: 'fetch' });
  assert.ok(waited.ok, waited.text);
  assert.match(waited.text, /POST .*\/api\/items -> 200/);
  assert.match(waited.text, /Request body:\n\{"name":"pen"\}/);
  assert.match(waited.text, /"method":"POST"/);
  const missed = await helper.tool(1, 'browser_network', { wait_for: '/never', timeout_ms: 300 });
  assert.ok(!missed.ok);
  await helper.load(1, `${origin}/other`);
  const kept = await helper.tool(1, 'browser_network', { request: id });
  assert.match(kept.text, /Response body:\n\{"items":\["a","b"\],"method":"GET"/);
});

test('a recording keeps each call with a picture and reports its steps', async () => {
  const folder = fs.mkdtempSync(path.join(process.env.PANDEMONIUM_E2E_SCRATCH || path.join(os.homedir(), '.cache', 'scratch', 'pandemonium'), 'record-'));
  const started = await helper.tool(1, 'browser_record', { action: 'start', folder });
  assert.ok(started.ok, started.text);
  assert.equal(/** @type {any} */ (started).step, undefined);
  const snapshot = /** @type {any} */ (await helper.tool(1, 'browser_snapshot'));
  assert.equal(snapshot.step.index, 1);
  assert.equal(snapshot.step.tool, 'browser_snapshot');
  assert.ok(fs.statSync(snapshot.step.picture).size > 100);
  const evaluated = /** @type {any} */ (await helper.tool(1, 'browser_evaluate', { expression: '1 + 1' }));
  assert.equal(evaluated.step.index, 2);
  assert.equal(evaluated.step.arguments, '{"expression":"1 + 1"}');
  const stopped = await helper.tool(1, 'browser_record', { action: 'stop' });
  assert.match(stopped.text, /1\. .*`browser_snapshot`/);
  assert.match(stopped.text, /2\. .*`browser_evaluate` \{"expression":"1 \+ 1"\} — ok: 2/);
  assert.ok(fs.existsSync(path.join(folder, 'summary.md')));
  assert.equal(JSON.parse(fs.readFileSync(path.join(folder, 'steps.json'), 'utf8')).length, 2);
  const after = /** @type {any} */ (await helper.tool(1, 'browser_snapshot'));
  assert.equal(after.step, undefined);
  fs.rmSync(folder, { recursive: true, force: true });
});
