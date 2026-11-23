'use strict';

/**
 * End-to-end tests of pasting rich text and files, dropping files from the desktop on a drop zone, saving a page into
 * the Downloads folder and reader view, against the real helper.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { after, before, test } = require('node:test');
const { Helper, unquote } = require('./harness');

/** The page with the fields pasted and dropped into. */
const EDITOR = `<!doctype html><html><head><title>Editor</title></head><body style="margin:0">
<div id="rich" contenteditable="true" style="height:40px;border:1px solid #888"></div>
<input id="plain" style="width:300px">
<div id="taker" contenteditable="true" style="height:40px;border:1px solid #888"></div>
<div id="zone" style="position:absolute;left:0;top:200px;width:300px;height:150px;background:#cde">Drop here</div>
<script>
  window.taken = null;
  window.dropped = null;
  document.getElementById('taker').addEventListener('paste', (event) => {
    event.preventDefault();
    const data = event.clipboardData;
    window.taken = { types: [...data.types], html: data.getData('text/html'), files: [...data.files].map((f) => [f.name, f.type, f.size]) };
  });
  const zone = document.getElementById('zone');
  zone.addEventListener('dragover', (event) => event.preventDefault());
  zone.addEventListener('drop', async (event) => {
    event.preventDefault();
    const files = [...event.dataTransfer.files];
    window.dropped = await Promise.all(files.map(async (f) => [f.name, await f.text()]));
  });
</script></body></html>`;

/** A page that reads as an article. */
const ARTICLE = `<!doctype html><html lang="en"><head><title>The Long Walk</title></head><body>
<nav><a href="/">Home</a> <a href="/about">About</a></nav>
<article><h1>The Long Walk</h1><p class="byline">By Ann Writer</p>
${Array.from({ length: 12 }, (_, at) => `<p>Paragraph ${at} of the article, which goes on about walking in the hills for a long while, with enough words in it that the readability check counts it as part of a real article and not a caption or a menu.</p>`).join('\n')}
</article><aside>Advertising that is not part of it.</aside></body></html>`;

/** @type {Helper} */
let helper;
/** @type {http.Server} */
let server;
/** @type {string} */
let base = '';

before(async () => {
  server = http.createServer((request, response) => {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(request.url === '/article' ? ARTICLE : EDITOR);
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(undefined)));
  base = `http://127.0.0.1:${/** @type {import('node:net').AddressInfo} */ (server.address()).port}`;
  helper = await Helper.start();
  await helper.open(1, `${base}/editor`, { width: 600, height: 400, scale: 1 });
});

after(async () => {
  await helper?.stop();
  await new Promise((resolve) => server?.close(() => resolve(undefined)));
});

/**
 * Writes a scratch file for a paste or a drop.
 * @param {string} name Its name.
 * @param {string | Buffer} content What it holds.
 * @returns {string} Its absolute path.
 */
function scratchFile(name, content) {
  const file = path.join(helper.folder, name);
  fs.writeFileSync(file, content);
  return file;
}

test('HTML pasted into an editable region keeps its formatting, and a plain field gets the text', async () => {
  await helper.evaluate(1, "document.getElementById('rich').focus()");
  helper.send({ PasteText: { page: 1, text: 'bold text', html: '<b>bold</b> text', files: [] } });
  await helper.until(1, "document.getElementById('rich').innerHTML.includes('<b>bold</b>')", true);
  await helper.evaluate(1, "document.getElementById('plain').focus()");
  helper.send({ PasteText: { page: 1, text: 'bold text', html: '<b>bold</b> text', files: [] } });
  await helper.until(1, "document.getElementById('plain').value", 'bold text');
});

test('a page that handles paste itself gets the HTML and pasted files', async () => {
  const picture = scratchFile('pasted.png', Buffer.from('89504e470d0a1a0a', 'hex'));
  await helper.evaluate(1, "document.getElementById('taker').focus()");
  helper.send({ PasteText: { page: 1, text: 'x', html: '<i>x</i>', files: [picture] } });
  await helper.until(1, 'window.taken && window.taken.files', [['pasted.png', 'image/png', 8]]);
  const html = unquote(await helper.evaluate(1, 'window.taken.html'));
  assert.equal(html, '<i>x</i>');
  assert.equal(unquote(await helper.evaluate(1, "document.getElementById('taker').innerHTML")), '');
});

test('files dropped on a drop zone reach it', async () => {
  const note = scratchFile('note.txt', 'dropped words');
  helper.send({ Chrome: { page: 1, action: { DropFiles: { x: 100, y: 260, paths: [note] } } } });
  await helper.until(1, 'window.dropped', [['note.txt', 'dropped words']]);
});

test('saving a page writes it with its files into Downloads and reports it as a download', async () => {
  const from = helper.mark();
  helper.send({ Chrome: { page: 1, action: 'SavePage' } });
  const done = await helper.waitReport('Chrome', 1, (b) => b.report.Download && b.report.Download.progress === 'Finished', { from });
  const name = done.report.Download.name;
  assert.equal(name, 'Editor.html');
  const saved = [path.join(helper.home, 'Downloads', name), path.join(helper.home, name)].find((file) => fs.existsSync(file)) || '';
  assert.ok(fs.readFileSync(saved, 'utf8').includes('Drop here'));
  assert.ok(fs.statSync(saved).size > 0);
});

test('an article is offered for reader view and read without its menus and advertising', async () => {
  let from = helper.mark();
  await helper.load(1, `${base}/article`);
  await helper.waitReport('Chrome', 1, (b) => b.report.Readable && b.report.Readable.readable === true, { from });
  from = helper.mark();
  helper.send({ Chrome: { page: 1, action: 'ReadArticle' } });
  const read = await helper.waitReport('Chrome', 1, (b) => Boolean(b.report.Article), { from });
  const article = read.report.Article;
  assert.equal(article.error, '');
  assert.equal(article.title, 'The Long Walk');
  assert.ok(article.html.includes('Paragraph 11 of the article'));
  assert.ok(!article.html.includes('Advertising'));
  assert.equal(article.url, `${base}/article`);
  from = helper.mark();
  await helper.load(1, `${base}/editor`);
  await helper.waitReport('Chrome', 1, (b) => b.report.Readable && b.report.Readable.readable === false, { from });
});
