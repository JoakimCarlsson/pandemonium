'use strict';

/**
 * The pages the end-to-end tests load, served by two local web servers: one on `127.0.0.1` and one on `localhost`,
 * each on a port of its own, so a frame of the second inside a page of the first is cross-origin and cross-site and
 * runs in a renderer process of its own.
 */

const http = require('node:http');

/**
 * A frame with a text field that tells its parent what is typed into it and when it is clicked.
 * @param {string} name What the frame calls itself in its messages.
 * @returns {string} The document.
 */
function framePage(name) {
  return `<!doctype html><html><body style="margin:0;background:#cde">
<input id="field" style="position:absolute;left:10px;top:10px;width:150px;height:30px;font-size:16px">
<script>
const field = document.getElementById('field');
const tell = (kind) => parent.postMessage({ frame: ${JSON.stringify(name)}, kind, value: field.value }, '*');
field.addEventListener('input', () => tell('input'));
field.addEventListener('mousedown', () => tell('click'));
document.documentElement.addEventListener('mouseover', () => tell('over'));
</script></body></html>`;
}

/**
 * The page with a field of its own, a same-origin frame and a cross-origin frame, which records what each reports.
 * @param {string} other The origin of the cross-origin frame.
 * @returns {string} The document.
 */
function inputPage(other) {
  return `<!doctype html><html><head><title>input</title></head><body style="margin:0;background:#fff">
<input id="top" style="position:absolute;left:10px;top:10px;width:200px;height:30px;font-size:16px">
<iframe id="same" src="/frame?name=same" style="position:absolute;left:10px;top:100px;width:300px;height:80px;border:0"></iframe>
<iframe id="cross" src="${other}/frame?name=cross" style="position:absolute;left:10px;top:250px;width:300px;height:80px;border:0"></iframe>
<script>
window.frames_seen = { same: { clicks: 0, value: '' }, cross: { clicks: 0, value: '' } };
window.keys = [];
document.getElementById('top').addEventListener('keydown', (e) => keys.push({ key: e.key, ctrl: e.ctrlKey, code: e.code }));
addEventListener('message', (event) => {
  const seen = frames_seen[event.data.frame];
  if (event.data.kind === 'click') seen.clicks += 1;
  seen.value = event.data.value;
});
</script></body></html>`;
}

/**
 * A page with a cross-origin frame whose top left quarter a box of the page covers, with a box 2 pixels square over the
 * frame further in, and below it a second cross-origin frame in a box whose `:hover` rule shows a box over that frame's
 * top left corner. It counts the times the pointer comes over each box and over each frame.
 * @param {string} other The origin of the cross-origin frames.
 * @returns {string} The document.
 */
function coveredPage(other) {
  return `<!doctype html><html><head><title>covered</title>
<style>#reveal { display: none } #held:hover #reveal { display: block }</style></head><body style="margin:0;background:#fff">
<iframe id="cross" src="${other}/frame?name=cross" style="position:absolute;left:10px;top:50px;width:300px;height:200px;border:0"></iframe>
<div id="cover" style="position:absolute;left:10px;top:50px;width:150px;height:100px;background:#fc0"></div>
<div id="tiny" style="position:absolute;left:280px;top:220px;width:2px;height:2px;background:#f00"></div>
<div id="held" style="position:absolute;left:10px;top:270px;width:300px;height:100px">
<iframe src="${other}/frame?name=held" style="position:absolute;left:0;top:0;width:300px;height:100px;border:0"></iframe>
<div id="reveal" style="position:absolute;left:0;top:0;width:100px;height:50px;background:#0c6"></div>
</div>
<script>
window.overs = { cover: 0, frame: 0, tiny: 0, reveal: 0, held: 0 };
for (const name of ['cover', 'tiny', 'reveal']) document.getElementById(name).addEventListener('mouseover', () => { overs[name] += 1; });
addEventListener('message', (event) => {
  if (event.data.kind !== 'over') return;
  if (event.data.frame === 'held') overs.held += 1;
  else overs.frame += 1;
});
</script></body></html>`;
}

/** A page that counts the pointer moves it hears and puts where the last one was in its title. */
const HOVER_PAGE = `<!doctype html><html><head><title>hover</title></head><body style="margin:0;height:100vh;background:#eee">
<script>
window.moves = 0;
addEventListener('mousemove', (e) => { moves += 1; document.title = 'at ' + e.clientX + ',' + e.clientY; });
</script></body></html>`;

/** A page of coloured blocks and text, to check that what is painted is not empty and for finding text. */
const PAINT_PAGE = `<!doctype html><html><head><title>paint</title></head><body style="margin:0;background:#fff">
<div style="width:200px;height:100px;background:#c03"></div><div style="width:100px;height:50px;background:#03c"></div>
<p>needle one, needle two, needle three</p>
<script>window.dpr = devicePixelRatio; window.inner = [innerWidth, innerHeight];</script></body></html>`;

/** A frame that tells its parent, over and over, its pixel ratio, its width and where it was last pressed. */
const RATIO_FRAME = `<!doctype html><html><body style="margin:0;background:#cfe">
<p style="margin:0;font:12px sans-serif">cross-origin frame text</p>
<script>
let pressed = null;
addEventListener('mousedown', (event) => { pressed = [event.clientX, event.clientY]; });
setInterval(() => parent.postMessage({ ratio: devicePixelRatio, width: innerWidth, pressed }, '*'), 100);
</script></body></html>`;

/**
 * A page for the display scale and emulation tests: text to look at, a cross-origin frame 200 by 60 at its top left whose
 * reports it keeps in `window.cross`, and where it was last pressed in `window.pressed`.
 * @param {string} other The origin of the cross-origin frame.
 * @returns {string} The page.
 */
function ratioPage(other) {
  return `<!doctype html><html><head><title>ratio</title><meta name="viewport" content="width=device-width"></head>
<body style="margin:0;background:#fff">
<iframe src="${other}/ratio-frame" style="display:block;width:200px;height:60px;border:0"></iframe>
<p style="margin:0;height:400px;font:14px sans-serif">The quick brown fox jumps over the lazy dog.</p>
<script>
window.cross = {};
window.pressed = null;
addEventListener('message', (event) => { window.cross = event.data; });
addEventListener('mousedown', (event) => { window.pressed = [event.clientX, event.clientY]; });
</script></body></html>`;
}

/** A page whose button opens the ratio page as a popup. */
const RATIO_OPENER = `<!doctype html><html><head><title>ratio opener</title></head><body style="margin:0">
<button style="width:300px;height:100px" onclick="window.child = window.open('/ratio', 'ratio', 'width=300,height=200')">open</button>
</body></html>`;

/** A page whose three buttons, one under the other, open the three dialogs and put each answer in its title. */
const DIALOG_PAGE = `<!doctype html><html><head><title>dialogs</title></head><body style="margin:0">
<button style="display:block;width:200px;height:50px" onclick="alert('hello'); tell('alert', true)">alert</button>
<button style="display:block;width:200px;height:50px" onclick="tell('confirm', confirm('sure?'))">confirm</button>
<button style="display:block;width:200px;height:50px" onclick="tell('prompt', prompt('name?', 'x'))">prompt</button>
<script>window.tell = (kind, answer) => { document.title = kind + '=' + JSON.stringify(answer); };</script></body></html>`;

/** A page that opens a popup that keeps `window.opener`, on a click. */
const POPUP_PAGE = `<!doctype html><html><head><title>opener</title></head><body style="margin:0">
<button id="open" style="width:300px;height:100px" onclick="window.child = window.open('/popup-child', 'child', 'width=400,height=300')">open</button>
</body></html>`;

/** The popup's page, which closes itself when asked through its opener. */
const POPUP_CHILD = `<!doctype html><html><head><title>child</title></head><body>
<input id="field" style="position:absolute;left:10px;top:10px;width:300px;height:40px">
<script>window.hasOpener = !!window.opener;</script>child</body></html>`;

/** A page with a link and a field, for the menu of a right click. */
const MENU_PAGE = `<!doctype html><html><head><title>menu</title></head><body style="margin:0">
<a id="link" href="/target" style="display:block;width:200px;height:40px;background:#ddd">a link</a>
<input id="field" style="position:absolute;left:10px;top:100px;width:200px;height:30px">
</body></html>`;

/** How many bytes the download fixture sends. */
const DOWNLOAD_BYTES = 256 * 1024;

/**
 * Serves one request of either server.
 * @param {http.IncomingMessage} request The request.
 * @param {http.ServerResponse} response The response.
 * @param {() => string} other The origin of the other server.
 * @returns {void}
 */
function serve(request, response, other) {
  const url = new URL(request.url || '/', 'http://fixture');
  const html = (body) => {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(body);
  };
  switch (url.pathname) {
    case '/frame':
      return html(framePage(url.searchParams.get('name') || 'frame'));
    case '/input':
      return html(inputPage(other()));
    case '/covered':
      return html(coveredPage(other()));
    case '/hover':
      return html(HOVER_PAGE);
    case '/paint':
      return html(PAINT_PAGE);
    case '/dialogs':
      return html(DIALOG_PAGE);
    case '/popup':
      return html(POPUP_PAGE);
    case '/ratio':
      return html(ratioPage(other()));
    case '/ratio-frame':
      return html(RATIO_FRAME);
    case '/ratio-opener':
      return html(RATIO_OPENER);
    case '/popup-child':
      return html(POPUP_CHILD);
    case '/menu':
      return html(MENU_PAGE);
    case '/one':
      return html('<!doctype html><title>one</title><p>one</p>');
    case '/two':
      return html('<!doctype html><title>two</title><p>two</p>');
    case '/hang':
      response.writeHead(200, { 'content-type': 'text/html' });
      response.write('<!doctype html><title>hang</title><p>loading');
      return;
    case '/download': {
      response.writeHead(200, {
        'content-type': 'application/octet-stream',
        'content-disposition': 'attachment; filename="data.bin"',
        'content-length': String(DOWNLOAD_BYTES),
      });
      let sent = 0;
      const chunk = Buffer.alloc(16 * 1024, 7);
      const timer = setInterval(() => {
        response.write(chunk);
        sent += chunk.length;
        if (sent >= DOWNLOAD_BYTES) {
          clearInterval(timer);
          response.end();
        }
      }, 30);
      response.on('close', () => clearInterval(timer));
      return;
    }
    default:
      response.writeHead(404, { 'content-type': 'text/html' });
      response.end('<!doctype html><title>missing</title>');
  }
}

/**
 * The two fixture servers.
 * @typedef {object} Fixtures
 * @property {string} main The origin of the first, such as `http://127.0.0.1:41234`.
 * @property {string} other The origin of the second, on `localhost`, cross-site to the first.
 * @property {number} downloadBytes How many bytes `/download` sends.
 * @property {() => Promise<void>} close Stops both.
 */

/**
 * Starts both fixture servers on free ports.
 * @returns {Promise<Fixtures>} Their origins and how to stop them.
 */
async function startFixtures() {
  /** @type {{ main: string, other: string }} */
  const origins = { main: '', other: '' };
  const sockets = new Set();
  const first = http.createServer((q, r) => serve(q, r, () => origins.other));
  const second = http.createServer((q, r) => serve(q, r, () => origins.main));
  for (const server of [first, second]) server.on('connection', (socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
  });
  const listen = (server, host) =>
    new Promise((resolve) => server.listen(0, host, () => resolve(/** @type {any} */ (server.address()).port)));
  origins.main = `http://127.0.0.1:${await listen(first, '127.0.0.1')}`;
  origins.other = `http://localhost:${await listen(second, '127.0.0.1')}`;
  return {
    main: origins.main,
    other: origins.other,
    downloadBytes: DOWNLOAD_BYTES,
    close: async () => {
      for (const socket of sockets) socket.destroy();
      await Promise.all([first, second].map((s) => new Promise((resolve) => s.close(() => resolve(undefined)))));
    },
  };
}

module.exports = { startFixtures };
