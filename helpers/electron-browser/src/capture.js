'use strict';

/**
 * What the reader adds to the prompt from a page: a picture of what is shown, a picture of the
 * whole page, a picture of a rectangle they drag, and the lines of the page's console. A picture is
 * saved as a file the window attaches; everything is reported as one `Captured`.
 *
 * Everything the page said is untrusted data. It is passed on as data and the window sets it off
 * when it writes the prompt.
 */

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Capture} Capture */
/** @typedef {import('./types').CaptureKind} CaptureKind */
/** @typedef {import('./types').CapturedPicture} CapturedPicture */
/** @typedef {import('./types').Json} Json */

/** The most bytes of one picture that are passed on; a model refuses larger pictures. */
const LARGEST_PICTURE = 3500000;

/** The most pixels, in CSS pixels, an area may cover before it counts as too large. */
const LARGEST_AREA = 16000000;

/** The tallest part of a page the full page picture covers, in CSS pixels. */
const TALLEST_PAGE = 16000;

/** The most lines of the console that are passed on. */
const MOST_LINES = 200;

/** The most characters of a line of the console that are passed on. */
const LONGEST_LINE = 1000;

/** The smallest side of an area worth taking, in CSS pixels. */
const SMALLEST_SIDE = 4;

/** How long the page is given to paint again once the rectangle is gone, in milliseconds. */
const REPAINT = 80;

/** The name of the scripting world the area script runs in. */
const WORLD = 'pm-area';

/** The name of the function that world reports through. */
const BINDING = '__pmArea';

/**
 * The script that, run in a page, lets the reader drag a rectangle and reports it through the
 * binding: in viewport pixels, with how far the page is scrolled. It takes its own elements off
 * the page before it reports, so they are not in the picture. Only events the reader made count.
 */
const SCRIPT = `(() => {
  if (window.__pmArea) window.__pmArea.stop();
  const veil = document.createElement('div');
  veil.style.cssText = 'position:fixed;left:0;top:0;right:0;bottom:0;z-index:2147483647;cursor:crosshair;';
  const box = document.createElement('div');
  box.style.cssText = 'position:fixed;box-sizing:border-box;display:none;pointer-events:none;border:2px solid #4c8bf5;background:rgba(76,139,245,.15);box-shadow:0 0 0 9999px rgba(0,0,0,.35);';
  const hint = document.createElement('div');
  hint.textContent = 'Drag over the area to add. Esc cancels.';
  hint.style.cssText = 'position:fixed;top:8px;left:50%;transform:translateX(-50%);padding:4px 10px;border-radius:6px;font:12px sans-serif;color:#fff;background:#222;pointer-events:none;';
  veil.appendChild(box);
  veil.appendChild(hint);
  document.documentElement.appendChild(veil);
  let start = null;
  const rectOf = (e) => {
    const x = Math.min(start.x, e.clientX);
    const y = Math.min(start.y, e.clientY);
    return { x, y, width: Math.abs(e.clientX - start.x), height: Math.abs(e.clientY - start.y) };
  };
  const swallow = (e) => { e.preventDefault(); e.stopPropagation(); e.stopImmediatePropagation(); };
  const down = (e) => {
    swallow(e);
    if (!e.isTrusted || e.button !== 0) return;
    start = { x: e.clientX, y: e.clientY };
    hint.style.display = 'none';
  };
  const move = (e) => {
    swallow(e);
    if (!start) return;
    const r = rectOf(e);
    box.style.display = 'block';
    box.style.left = r.x + 'px';
    box.style.top = r.y + 'px';
    box.style.width = r.width + 'px';
    box.style.height = r.height + 'px';
  };
  const up = (e) => {
    swallow(e);
    if (!e.isTrusted || !start) return;
    const r = rectOf(e);
    start = null;
    box.style.display = 'none';
    if (r.width < ${SMALLEST_SIDE} || r.height < ${SMALLEST_SIDE}) { hint.style.display = 'block'; return; }
    stop();
    __pmArea(JSON.stringify({ rect: { x: r.x, y: r.y, width: r.width, height: r.height, scrollX: window.scrollX, scrollY: window.scrollY } }));
  };
  const key = (e) => {
    if (e.isTrusted && e.key === 'Escape') { swallow(e); stop(); __pmArea(JSON.stringify({})); }
  };
  const wheel = (e) => e.preventDefault();
  function stop() {
    document.removeEventListener('keydown', key, true);
    veil.remove();
    delete window.__pmArea;
  }
  veil.addEventListener('mousedown', down, true);
  veil.addEventListener('mousemove', move, true);
  veil.addEventListener('mouseup', up, true);
  veil.addEventListener('wheel', wheel, { passive: false });
  for (const name of ['click', 'contextmenu', 'dblclick', 'auxclick', 'pointerdown', 'pointerup']) veil.addEventListener(name, swallow, true);
  document.addEventListener('keydown', key, true);
  window.__pmArea = { stop };
})();`;

/**
 * Per page, whether it already listens for what the area script reports, and the context the script runs in while the mode is on.
 * @type {WeakMap<Page, { listening: boolean, context: number | null }>}
 */
const modes = new WeakMap();

/**
 * Text as one line cut to `most` characters.
 * @param {unknown} value What a page said.
 * @param {number} most The most characters to keep.
 * @returns {string} The text, or nothing when `value` is not text.
 */
function line(value, most) {
  if (typeof value !== 'string') return '';
  const text = value.replace(/\s+/g, ' ').trim();
  return text.length > most ? `${text.slice(0, most)}…` : text;
}

/**
 * An empty capture of `kind` for `page`, to fill in.
 * @param {Page} page The page.
 * @param {CaptureKind} kind What was asked.
 * @returns {Capture} The capture, with the page's address and title.
 */
function begin(page, kind) {
  return {
    kind,
    pictures: [],
    lines: [],
    url: line(page.contents.getURL(), 500),
    title: line(page.contents.getTitle(), 200),
    note: '',
    error: '',
  };
}

/**
 * A picture of the page, in PNG, or in JPEG when the PNG is too large to pass on.
 * @param {Page} page The page, which must have a protocol to speak.
 * @param {{ x: number, y: number, width: number, height: number } | null} clip The part of the page to take, in CSS pixels from its top left corner, or nothing for what is shown.
 * @param {boolean} beyond Whether the part may reach past what is shown.
 * @returns {Promise<{ bytes: Buffer, extension: string } | 'large' | null>} The picture; `'large'` when even the JPEG is too large; nothing when the engine gave none.
 */
async function take(page, clip, beyond) {
  const cdp = page.cdp;
  if (!cdp) return null;
  for (const format of /** @type {const} */ (['png', 'jpeg'])) {
    /** @type {Json} */
    const params = format === 'png' ? { format } : { format, quality: 80 };
    if (clip) params.clip = { ...clip, scale: 1 };
    if (beyond) params.captureBeyondViewport = true;
    const reply = await cdp.call('Page.captureScreenshot', params).catch(() => null);
    if (!reply || !reply.data) return null;
    const bytes = Buffer.from(reply.data, 'base64');
    if (bytes.length <= LARGEST_PICTURE) return { bytes, extension: format === 'png' ? 'png' : 'jpg' };
  }
  return 'large';
}

/**
 * Takes a picture and adds it to `capture`.
 * @param {Page} page The page.
 * @param {Capture} capture What is being taken.
 * @param {string} label What the picture shows.
 * @param {{ x: number, y: number, width: number, height: number } | null} clip The part to take, or nothing for what is shown.
 * @param {boolean} beyond Whether the part may reach past what is shown.
 * @returns {Promise<boolean>} Whether the picture was added.
 */
async function add(page, capture, label, clip, beyond) {
  const picture = await take(page, clip, beyond);
  if (picture === 'large') {
    capture.note = `${capture.note} The ${label.toLowerCase()} picture was too large to attach.`.trim();
    return false;
  }
  if (!picture) return false;
  capture.pictures.push({ label, path: page.surface.savePicture(picture.bytes, picture.extension) });
  return true;
}

/**
 * Takes the whole page, as far down as `TALLEST_PAGE` allows, and adds it to `capture`.
 * @param {Page} page The page.
 * @param {Capture} capture What is being taken.
 * @returns {Promise<boolean>} Whether the picture was added.
 */
async function addFullPage(page, capture) {
  const cdp = page.cdp;
  if (!cdp) return false;
  const metrics = await cdp.call('Page.getLayoutMetrics').catch(() => null);
  const size = metrics && (metrics.cssContentSize || metrics.contentSize);
  if (!size) return false;
  const height = Math.min(Math.ceil(size.height), TALLEST_PAGE);
  if (size.height > TALLEST_PAGE) {
    capture.note = `${capture.note} The page is taller than ${TALLEST_PAGE} px; only its top is in the picture.`.trim();
  }
  return add(page, capture, 'Full page', { x: 0, y: 0, width: Math.ceil(size.width), height }, true);
}

/**
 * Takes what the reader asked for and reports it.
 * @param {Page} page The page.
 * @param {CaptureKind} kind What to take.
 * @returns {Promise<void>} Resolves once the window has been told.
 */
async function run(page, kind) {
  const capture = begin(page, kind);
  try {
    if (!page.cdp || !page.recorder) {
      capture.error = 'Nothing can be taken from developer tools.';
    } else if (kind === 'Console') {
      capture.lines = page.recorder.console.slice(-MOST_LINES).map((text) => line(text, LONGEST_LINE));
    } else if (kind === 'Screenshot') {
      if (!(await add(page, capture, 'Visible', null, false))) capture.error = capture.note || 'No picture could be taken.';
    } else if (kind === 'FullPage') {
      if (!(await addFullPage(page, capture))) capture.error = capture.note || 'No picture could be taken.';
    } else {
      await pickArea(page, true);
      return;
    }
  } catch (error) {
    capture.error = String(error instanceof Error ? error.message : error);
  }
  page.send({ Captured: { page: page.id, capture } });
}

/**
 * Takes the area the reader chose, or, when it is too large to pass on, what is shown and the whole page in its place, and says so.
 * @param {Page} page The page.
 * @param {{ x: number, y: number, width: number, height: number, scrollX: number, scrollY: number }} rect The area, in viewport pixels, and how far the page is scrolled.
 * @returns {Promise<Capture>} What was taken.
 */
async function takeArea(page, rect) {
  const capture = begin(page, 'Area');
  const clip = { x: rect.x + rect.scrollX, y: rect.y + rect.scrollY, width: rect.width, height: rect.height };
  const large = rect.width * rect.height > LARGEST_AREA;
  if (!large && (await add(page, capture, 'Area', clip, false))) return capture;
  const why = large || capture.note ? 'it was too large to attach' : 'it could not be taken';
  capture.note = `Area skipped: ${why}. The visible part and the whole page are attached instead.`;
  capture.pictures = [];
  const shown = await add(page, capture, 'Visible', null, false);
  const whole = await addFullPage(page, capture);
  if (!shown && !whole) capture.error = 'No picture could be taken.';
  return capture;
}

/**
 * Turns on or off the mode in which the reader drags a rectangle over the page; the area is reported as a capture, and ending the mode without one as a capture of nothing.
 * @param {Page} page The page, which must have a protocol to speak.
 * @param {boolean} on Whether the mode is on.
 * @returns {Promise<void>} Resolves once the mode has changed.
 */
async function pickArea(page, on) {
  const cdp = page.cdp;
  if (!cdp) return;
  let mode = modes.get(page);
  if (!mode) {
    mode = { listening: false, context: null };
    modes.set(page, mode);
  }
  const held = mode;
  if (!on) {
    const contextId = held.context;
    held.context = null;
    if (contextId !== null) {
      await cdp.call('Runtime.evaluate', { contextId, expression: 'window.__pmArea && window.__pmArea.stop();' }).catch(() => {});
    }
    return;
  }
  try {
    if (!held.listening) {
      held.listening = true;
      cdp.listen((method, params) => {
        if (method !== 'Runtime.bindingCalled' || params.name !== BINDING) return;
        if (params.executionContextId === undefined || params.executionContextId !== held.context) return;
        held.context = null;
        /** @type {Record<string, any> | null} */
        let found = null;
        try {
          found = JSON.parse(params.payload);
        } catch (_) {
          found = null;
        }
        const rect = found && found.rect;
        if (!rect || ![rect.x, rect.y, rect.width, rect.height, rect.scrollX, rect.scrollY].every(Number.isFinite)) {
          page.send({ Captured: { page: page.id, capture: begin(page, 'Area') } });
          return;
        }
        new Promise((done) => setTimeout(done, REPAINT))
          .then(() => takeArea(page, rect))
          .catch((error) => {
            const failed = begin(page, 'Area');
            failed.error = String(error instanceof Error ? error.message : error);
            return failed;
          })
          .then((capture) => page.send({ Captured: { page: page.id, capture } }));
      });
    }
    await cdp.call('Runtime.enable');
    await cdp.call('Page.enable');
    await cdp.call('Runtime.addBinding', { name: BINDING, executionContextName: WORLD });
    const tree = await cdp.call('Page.getFrameTree');
    const created = await cdp.call('Page.createIsolatedWorld', {
      frameId: tree.frameTree.frame.id,
      worldName: WORLD,
      grantUniveralAccess: false,
    });
    held.context = created.executionContextId;
    await cdp.call('Runtime.evaluate', { contextId: created.executionContextId, expression: SCRIPT });
  } catch (_) {
    const failed = begin(page, 'Area');
    failed.error = 'The page could not be read to choose an area.';
    page.send({ Captured: { page: page.id, capture: failed } });
  }
}

module.exports = { run, pickArea };
