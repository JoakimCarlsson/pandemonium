'use strict';

/**
 * End-to-end tests of a page's life and what it reports around it: loading, failed loads, history, stopping, crashes,
 * popups, dialogs, the menu of a right click, downloads, finding, zooming and an agent's screenshot.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { after, before, test } = require('node:test');
const { Helper, sleep } = require('./harness');
const { startFixtures } = require('./fixtures');

/** @type {Helper} */
let helper;
/** @type {import('./fixtures').Fixtures} */
let fixtures;

before(async () => {
  fixtures = await startFixtures();
  helper = await Helper.start();
  await helper.open(1, `${fixtures.main}/one`, { width: 600, height: 400, scale: 1 });
});

after(async () => {
  await helper?.stop();
  await fixtures?.close();
});

/**
 * Presses and releases `button` at `x`, `y` of `page`.
 * @param {number} page The page.
 * @param {number} x Logical pixels from the left.
 * @param {number} y Logical pixels from the top.
 * @param {'Left' | 'Right'} [button] The button.
 * @returns {void}
 */
function click(page, x, y, button = 'Left') {
  const held = button === 'Left' ? 16 : 64;
  helper.send({ PointerMoved: { page, x, y, modifiers: 0, left: false } });
  helper.send({ PointerButton: { page, x, y, button, pressed: true, clicks: 1, modifiers: held } });
  helper.send({ PointerButton: { page, x, y, button, pressed: false, clicks: 1, modifiers: 0 } });
}

/**
 * Waits for a `Chrome` report of `page` whose report is named `name`.
 * @param {number} page The page.
 * @param {string} name The report inside, such as `Dialog`.
 * @param {number} from The first report to look at.
 * @param {(body: any) => boolean} [match] What else it must hold.
 * @param {number} [timeout] How long to wait, in milliseconds.
 * @returns {Promise<any>} The report inside.
 */
async function chromeReport(page, name, from, match = () => true, timeout = undefined) {
  const body = await helper.waitReport('Chrome', page, (b) => name in b.report && match(b.report[name]), { from, timeout });
  return body.report[name];
}

test('a load reports its progress, its address and its history', async () => {
  const from = helper.mark();
  await helper.load(1, `${fixtures.main}/two`);
  const loading = helper.reports.slice(from).filter((r) => r.Loading?.page === 1).map((r) => r.Loading);
  assert.ok(loading.some((l) => l.loading && l.progress > 0 && l.progress < 1), JSON.stringify(loading));
  assert.equal(loading[loading.length - 1].loading, false);
  await helper.waitReport('Title', 1, (b) => b.title === 'two', { from });
  await helper.waitReport('History', 1, (b) => b.back && !b.forward, { from });
  const back = helper.mark();
  helper.send({ Back: { page: 1 } });
  await helper.waitReport('Address', 1, (b) => b.url === `${fixtures.main}/one`, { from: back });
  await helper.waitReport('History', 1, (b) => b.forward, { from: back });
  const forward = helper.mark();
  helper.send({ Forward: { page: 1 } });
  await helper.waitReport('Address', 1, (b) => b.url === `${fixtures.main}/two`, { from: forward });
});

test('a refused connection reports LoadFailed for the address', async () => {
  const from = helper.mark();
  const url = 'http://127.0.0.1:9/';
  helper.send({ Load: { page: 1, url } });
  const failed = await helper.waitReport('LoadFailed', 1, () => true, { from });
  assert.equal(failed.url, url);
  assert.ok(failed.code < 0 && failed.code !== -3, `code ${failed.code}`);
  assert.ok(failed.name.startsWith('ERR_'), failed.name);
});

test('Stop ends a load that hangs without reporting a failure', async () => {
  const from = helper.mark();
  helper.send({ Load: { page: 1, url: `${fixtures.main}/hang` } });
  await helper.waitReport('Loading', 1, (b) => b.loading, { from });
  await sleep(300);
  const stop = helper.mark();
  helper.send({ Stop: { page: 1 } });
  await helper.waitReport('Loading', 1, (b) => !b.loading, { from: stop });
  await sleep(300);
  assert.equal(helper.reports.slice(from).some((r) => r.LoadFailed?.page === 1), false);
});

test('a renderer that crashes reports Crashed and comes back on Reload', async () => {
  await helper.load(1, `${fixtures.main}/one`);
  const from = helper.mark();
  assert.ok(helper.killRenderers() > 0, 'the page has a renderer to kill');
  const crashed = await helper.waitReport('Crashed', 1, () => true, { from });
  assert.ok(crashed.reason, 'a crash names its reason');
  const reload = helper.mark();
  helper.send({ Load: { page: 1, url: `${fixtures.main}/one` } });
  await helper.waitReport('Loading', 1, (b) => !b.loading, { from: reload });
  await helper.until(1, 'document.title', 'one');
});

test('window.open makes a popup the window adopts, which closes as a page', async () => {
  await helper.load(1, `${fixtures.main}/popup`);
  await helper.readerTurn();
  const from = helper.mark();
  click(1, 100, 40);
  const opened = await helper.waitReport('PopupOpened', 1, () => true, { from });
  assert.equal(opened.agent, false);
  const adopt = helper.mark();
  helper.send({ AdoptPopup: { page: 7, popup: opened.popup, width: 400, height: 300, scale: 1 } });
  await helper.waitReport('Address', 7, (b) => b.url === `${fixtures.main}/popup-child`, { from: adopt });
  await helper.until(7, 'window.hasOpener', true);
  const close = helper.mark();
  await helper.evaluate(1, 'window.child.close(), true');
  await helper.waitReport('PageClosed', 7, () => true, { from: close });
});

test(
  'an adopted popup paints into its surface at the pane\'s size',
  async () => {
    await helper.load(1, `${fixtures.main}/popup`);
    await helper.readerTurn();
    const from = helper.mark();
    click(1, 100, 40);
    const opened = await helper.waitReport('PopupOpened', 1, () => true, { from });
    const adopt = helper.mark();
    helper.send({ AdoptPopup: { page: 8, popup: opened.popup, width: 400, height: 300, scale: 1 } });
    try {
      await helper.waitReport('Painted', 8, () => true, { from: adopt, timeout: 5000 });
      await helper.waitReport('Surface', 8, (b) => b.width === 400 && b.height === 300, { from: adopt });
      await helper.until(8, 'window.innerWidth', 400);
    } finally {
      helper.send({ Close: { page: 8 } });
    }
  },
);

test('an adopted popup takes clicks, keys, committed and pasted text, and an agent screenshot', async () => {
  await helper.load(1, `${fixtures.main}/popup`);
  await helper.readerTurn();
  const from = helper.mark();
  click(1, 100, 40);
  const opened = await helper.waitReport('PopupOpened', 1, () => true, { from });
  const adopt = helper.mark();
  helper.send({ AdoptPopup: { page: 9, popup: opened.popup, width: 400, height: 300, scale: 1 } });
  try {
    await helper.waitReport('Painted', 9, () => true, { from: adopt });
    helper.send({ Focus: { page: 9, focused: true } });
    click(9, 100, 30);
    await helper.until(9, 'document.activeElement.id', 'field');
    for (const letter of 'ab') {
      const code = letter.toUpperCase().charCodeAt(0);
      helper.send({ Key: { page: 9, key: { pressed: true, code, text: letter, modifiers: 0 } } });
      helper.send({ Key: { page: 9, key: { pressed: false, code, text: null, modifiers: 0 } } });
    }
    await helper.until(9, "document.getElementById('field').value", 'ab');
    helper.send({ CommitText: { page: 9, text: 'é日' } });
    await helper.until(9, "document.getElementById('field').value", 'abé日');
    helper.send({ PasteText: { page: 9, text: '-pasted' } });
    await helper.until(9, "document.getElementById('field').value", 'abé日-pasted');
    const shot = await helper.tool(9, 'browser_screenshot', {}, 20000);
    assert.equal(shot.ok, true, shot.text);
    assert.ok(shot.image, 'the screenshot carries an image');
    const png = require('node:fs').readFileSync(/** @type {string} */ (shot.image));
    assert.deepEqual([png.readUInt32BE(16), png.readUInt32BE(20)], [400, 300]);
  } finally {
    helper.send({ Close: { page: 9 } });
  }
});

test('alert, confirm and prompt are reported and take the reader\'s answers', async () => {
  await helper.load(1, `${fixtures.main}/dialogs`);
  await helper.readerTurn();
  for (const [kind, y, answer, title] of [
    ['Alert', 25, { accept: true, text: '' }, 'alert=true'],
    ['Confirm', 75, { accept: false, text: '' }, 'confirm=false'],
    ['Prompt', 125, { accept: true, text: 'typed' }, 'prompt="typed"'],
  ]) {
    const from = helper.mark();
    click(1, 100, /** @type {number} */ (y));
    const dialog = await chromeReport(1, 'Dialog', from);
    assert.equal(dialog.kind, kind);
    helper.send({ Chrome: { page: 1, action: { AnswerDialog: { id: dialog.id, ...answer } } } });
    await helper.waitReport('Title', 1, (b) => b.title === title, { from });
  }
});

test('a right click reports the menu with what is under the pointer', async () => {
  await helper.load(1, `${fixtures.main}/menu`);
  let from = helper.mark();
  click(1, 50, 20, 'Right');
  const link = await chromeReport(1, 'ContextMenu', from);
  assert.equal(link.link_url, `${fixtures.main}/target`);
  assert.equal(link.editable, false);
  from = helper.mark();
  click(1, 50, 115, 'Right');
  const field = await chromeReport(1, 'ContextMenu', from);
  assert.equal(field.editable, true);
  assert.equal(field.link_url, '');
});

test('a download reports its progress and is saved whole', async () => {
  await helper.load(1, `${fixtures.main}/one`);
  const from = helper.mark();
  helper.send({ Load: { page: 1, url: `${fixtures.main}/download` } });
  const finished = await chromeReport(1, 'Download', from, (d) => d.progress === 'Finished', 20000);
  assert.equal(finished.received, fixtures.downloadBytes);
  assert.match(finished.name, /^data( \(\d+\))?\.bin$/);
  const steps = helper.reports
    .slice(from)
    .filter((r) => r.Chrome?.report?.Download)
    .map((r) => r.Chrome.report.Download);
  assert.ok(steps.some((d) => d.progress === 'Running' && d.received < fixtures.downloadBytes), 'progress is reported while it runs');
  const saved = [path.join(helper.home, 'Downloads'), helper.home, path.join(os.homedir(), 'Downloads')]
    .map((folder) => path.join(folder, finished.name))
    .find((file) => fs.existsSync(file));
  assert.ok(saved, 'the file is saved in the Downloads folder the system names');
  const size = fs.statSync(saved).size;
  if (!saved.startsWith(helper.folder)) fs.rmSync(saved);
  assert.equal(size, fixtures.downloadBytes);
});

test('find counts the matches and moves through them', async () => {
  await helper.load(1, `${fixtures.main}/paint`);
  let from = helper.mark();
  helper.send({ Find: { page: 1, text: 'needle', forward: true, again: false } });
  const found = await helper.waitReport('Found', 1, (b) => b.total === 3, { from });
  assert.equal(found.active, 1);
  from = helper.mark();
  helper.send({ Find: { page: 1, text: 'needle', forward: true, again: true } });
  await helper.waitReport('Found', 1, (b) => b.total === 3 && b.active === 2, { from });
  helper.send({ StopFind: { page: 1 } });
});

test('zoom steps in and back to 100%', async () => {
  let from = helper.mark();
  helper.send({ Zoom: { page: 1, action: 'In' } });
  const zoomed = await helper.waitReport('Zoomed', 1, (b) => b.factor > 1, { from });
  assert.ok(zoomed.factor <= 1.25 + 1e-6, `zoomed to ${zoomed.factor}`);
  from = helper.mark();
  helper.send({ Zoom: { page: 1, action: 'Reset' } });
  await helper.waitReport('Zoomed', 1, (b) => Math.abs(b.factor - 1) < 1e-6, { from });
});

test('an agent screenshot returns a picture of the page', async () => {
  const result = await helper.tool(1, 'browser_screenshot', {}, 20000);
  assert.ok(result.ok, result.text);
  assert.ok(result.image, 'the screenshot carries an image');
  const image = String(result.image);
  const bytes = fs.existsSync(image) ? fs.readFileSync(image) : Buffer.from(image.replace(/^data:[^,]*,/, ''), 'base64');
  assert.ok(bytes.length > 1000, `the picture has ${bytes.length} bytes`);
  assert.ok(bytes.subarray(0, 4).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47])) || bytes[0] === 0xff, 'a PNG or JPEG');
});
