'use strict';

/**
 * End-to-end tests of `browser_run`: a whole flow in one call with only its result back, request waits and counts as
 * data, typing and clicking like a person, contexts in fresh profiles with a device, locale and time zone, headless
 * contexts run in parallel under a limit, a context shown as a tab, and recipes saved and run again with a recording.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const { after, before, test } = require('node:test');
const { Helper } = require('./harness');

/** The checkout page: a form whose button posts to the ingest API, a cookie it sets, and a list of the keys typed. */
const SHOP_PAGE = `<!doctype html><title>Shop</title>
<input id="name"><input id="card"><button id="buy" style="margin-top:1500px;width:200px;height:60px">buy</button>
<p id="keys"></p><p id="moves">0</p><p id="done"></p>
<script>
document.cookie = 'seen=yes';
let moves = 0;
addEventListener('mousemove', () => { moves += 1; document.getElementById('moves').textContent = moves; });
document.getElementById('card').addEventListener('keydown', (e) => { document.getElementById('keys').textContent += e.key; });
document.getElementById('buy').addEventListener('click', async () => {
  const r = await fetch('/ingest', { method: 'POST', body: JSON.stringify({ name: document.getElementById('name').value, card: document.getElementById('card').value }) });
  document.getElementById('done').textContent = 'status ' + r.status;
});
</script>`;

/** @type {Helper} */
let helper;
/** @type {http.Server} */
let server;
/** @type {string} */
let origin = '';
/** How many `/slow` requests are being answered at once, and the most there were. */
const slow = { now: 0, most: 0 };

/**
 * Serves the shop, the ingest API that answers 202, and a slow page that counts how many load at once.
 * @param {http.IncomingMessage} request The request.
 * @param {http.ServerResponse} response The response.
 * @returns {void}
 */
function serve(request, response) {
  const url = new URL(request.url || '/', 'http://fixture');
  if (url.pathname === '/shop') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(SHOP_PAGE);
  } else if (url.pathname === '/ingest') {
    request.resume();
    request.on('end', () => {
      response.writeHead(202, { 'content-type': 'application/json' });
      response.end('{"queued":true}');
    });
  } else if (url.pathname === '/slow') {
    slow.now += 1;
    slow.most = Math.max(slow.most, slow.now);
    setTimeout(() => {
      slow.now -= 1;
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      response.end('<!doctype html><title>Slow</title><p>slow</p>');
    }, 400);
  } else {
    response.writeHead(404);
    response.end();
  }
}

/**
 * Runs `browser_run` on page 1 and parses its answer.
 * @param {Record<string, unknown>} args The tool's arguments.
 * @returns {Promise<any>} The answer's JSON, with the tool's `ok` as `toolOk`.
 */
async function run(args) {
  const outcome = await helper.tool(1, 'browser_run', args, 90000);
  let parsed;
  try {
    parsed = JSON.parse(outcome.text);
  } catch (_) {
    assert.fail(outcome.text);
  }
  return { ...parsed, toolOk: outcome.ok };
}

before(async () => {
  server = http.createServer(serve);
  await new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(undefined)));
  origin = `http://127.0.0.1:${/** @type {any} */ (server.address()).port}`;
  helper = await Helper.start();
  await helper.open(1, 'about:blank', { width: 800, height: 600, scale: 1 });
});

after(async () => {
  await helper?.stop();
  server?.closeAllConnections();
  await new Promise((resolve) => server.close(() => resolve(undefined)));
});

test('a whole checkout runs in one call and gives back only its result, with requests as data', async () => {
  const answer = await run({
    base_url: origin,
    script: `
      await page.goto('/shop');
      await page.fill('#name', 'Ada');
      await page.type('#card', '4242', { delay: 40 });
      await page.click('#buy', { human: true });
      const ingest = await page.waitForResponse('*/ingest');
      await page.waitFor({ text: 'status 202' });
      check('ingest returned 202', ingest.status === 202, ingest);
      const counted = page.count('/ingest');
      check('one ingest', counted.total === 1 && counted.statuses['202'] === 1, counted);
      return { keys: await page.text('#keys'), moves: Number(await page.text('#moves')), body: ingest.body, method: ingest.method, scrolled: await page.evaluate('scrollY') };
    `,
  });
  assert.equal(answer.error, null);
  assert.ok(answer.ok && answer.toolOk);
  assert.equal(answer.passed, 2);
  assert.equal(answer.value.keys, '4242');
  assert.equal(answer.value.body, '{"queued":true}');
  assert.equal(answer.value.method, 'POST');
  assert.ok(answer.value.moves >= 10, `the pointer moved ${answer.value.moves} times`);
  assert.ok(answer.value.scrolled > 500, `scrolled to ${answer.value.scrolled}`);
});

test('a failed check or a thrown error fails the run and says why', async () => {
  const failing = await run({ script: "check('never', false, { why: 'test' }); return 1;" });
  assert.ok(!failing.ok && !failing.toolOk);
  assert.deepEqual(failing.checks, [{ name: 'never', ok: false, detail: '{"why":"test"}' }]);
  const thrown = await run({ base_url: origin, script: "await page.goto('/shop'); await page.click('#missing');" });
  assert.match(thrown.error, /^click: no such element/);
  const late = await run({ script: 'await sleep(5000);', timeout_ms: 1000 });
  assert.match(late.error, /ran longer than 1000 ms/);
});

test('contexts have fresh profiles of their own, with a device, locale and time zone', async () => {
  await helper.tool(1, 'browser_navigate', { url: `${origin}/shop` });
  const answer = await run({
    base_url: origin,
    script: `
      const tab = await page.evaluate('document.cookie');
      const one = await browser.newContext({ device: 'iphone-se', locale: 'de-DE', timezone: 'Asia/Tokyo' });
      const a = await one.newPage();
      await a.goto('/shop');
      const seen = await a.evaluate(() => ({ cookie: document.cookie, width: screen.width, mobile: navigator.userAgent.includes('iPhone'), lang: navigator.language, zone: Intl.DateTimeFormat().resolvedOptions().timeZone, number: (1234.5).toLocaleString() }));
      await a.evaluate("document.cookie = 'only=one'");
      const two = await browser.newContext();
      const b = await two.newPage();
      await b.goto('/shop');
      const other = await b.evaluate('document.cookie');
      await one.close();
      await two.close();
      return { tab, seen, other, tabAfter: await page.evaluate('document.cookie') };
    `,
  });
  assert.equal(answer.error, null);
  assert.match(answer.value.tab, /seen=yes/);
  assert.equal(answer.value.seen.cookie, 'seen=yes');
  assert.equal(answer.value.seen.width, 375);
  assert.equal(answer.value.seen.mobile, true);
  assert.equal(answer.value.seen.lang, 'de-DE');
  assert.equal(answer.value.seen.zone, 'Asia/Tokyo');
  assert.equal(answer.value.seen.number, '1.234,5');
  assert.equal(answer.value.other, 'seen=yes');
  assert.doesNotMatch(answer.value.tabAfter, /only=one/);
});

test('headless contexts run in parallel, never more than the limit at once', async () => {
  slow.most = 0;
  const started = Date.now();
  const answer = await run({
    base_url: origin,
    limit: 3,
    script: `
      const results = await parallel([1, 2, 3, 4, 5, 6, 7, 8, 9], async (n) => {
        const context = await browser.newContext();
        const p = await context.newPage();
        await p.goto('/slow?n=' + n);
        const title = p.title();
        await context.close();
        return title;
      });
      return results;
    `,
  });
  assert.equal(answer.error, null);
  assert.equal(answer.value.length, 9);
  assert.ok(answer.value.every((/** @type {any} */ result) => result.ok && result.value === 'Slow'), JSON.stringify(answer.value));
  assert.equal(slow.most, 3);
  assert.ok(Date.now() - started < 9 * 1400, 'the pages loaded side by side');
});

test('a context shown in the pane asks the window for a tab in its profile', async () => {
  const from = helper.mark();
  const running = run({ base_url: origin, script: "const c = await browser.newContext({ headless: false }); const p = await c.newPage(); await p.goto('/shop'); const cookie = await p.evaluate('document.cookie'); await c.close(); return cookie;" });
  const asked = await helper.waitReport('OpenRequested', 1, (body) => typeof body.partition === 'string', { from });
  assert.equal(asked.agent, true);
  await helper.open(2, asked.url, { width: 600, height: 400, scale: 1, partition: asked.partition });
  const closed = helper.waitReport('PageClosed', 2, () => true, { from });
  const answer = await running;
  assert.equal(answer.error, null);
  assert.equal(answer.value, 'seen=yes');
  await closed;
  helper.send({ Close: { page: 2 } });
});

test('a recipe is saved, listed and run again by name with a recording', async () => {
  const saved = await run({ base_url: origin, save: 'Checkout smoke', script: "await page.goto('/shop'); await page.fill('#name', 'Bo'); await page.click('#buy'); const r = await page.waitForResponse('/ingest'); check('ingest 202', r.status === 202); return r.status;" });
  assert.equal(saved.recipe, 'checkout-smoke');
  assert.equal(saved.value, 202);
  const listed = JSON.parse((await helper.tool(1, 'browser_run', { list: true })).text);
  assert.ok(listed.recipes.some((/** @type {any} */ recipe) => recipe.name === 'checkout-smoke'));
  const again = await run({ recipe: 'checkout-smoke', record: true });
  assert.ok(again.ok, JSON.stringify(again));
  assert.deepEqual(again.checks, [{ name: 'ingest 202', ok: true, detail: '' }]);
  assert.ok(again.recording && fs.existsSync(again.recording), again.recording);
  const steps = JSON.parse(fs.readFileSync(`${again.recording}/steps.json`, 'utf8'));
  assert.deepEqual(steps.map((/** @type {any} */ step) => step.tool), ['browser_navigate', 'browser_type', 'browser_click']);
  assert.ok(steps.every((/** @type {any} */ step) => step.picture && fs.existsSync(step.picture)));
  const missing = await helper.tool(1, 'browser_run', { recipe: 'nope' });
  assert.match(missing.text, /no recipe nope; saved recipes: checkout-smoke/);
});
