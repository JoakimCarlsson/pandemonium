'use strict';

/**
 * Where a frame inside a page sits in the page's top frame, so that a point a frame reports in its own viewport, such
 * as the pointer under a tooltip or the caret, can be given in the coordinates of the page as the pane shows it.
 *
 * Each step asks two frames: the child finds its own place among its parent's frames (`window.parent[i] === window`,
 * which is allowed across origins), and the parent finds the `<iframe>` or `<frame>` whose `contentWindow` is that
 * frame, which is also allowed across origins, and gives where its content box starts in its own viewport. The steps
 * add up to the top frame.
 */

/** The script that finds a frame's place among the frames of its parent, or -1 when it has none or is not found. */
const PLACE_SCRIPT = `(() => {
  if (window.parent === window) return -1;
  for (let i = 0; i < window.parent.length; i++) if (window.parent[i] === window) return i;
  return -1;
})()`;

/**
 * The script that finds, in a parent frame, where the content box of the element holding its child at `place` starts
 * in its viewport, in CSS pixels.
 * @param {number} place The child's place among the parent's frames.
 * @returns {string} The script; it gives `{ x, y }`, or `null` when no element holds that child.
 */
function boxScript(place) {
  return `(() => {
    const child = window[${place}];
    if (!child) return null;
    for (const element of document.querySelectorAll('iframe, frame')) {
      if (element.contentWindow !== child) continue;
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return {
        x: box.left + element.clientLeft + (parseFloat(style.paddingLeft) || 0),
        y: box.top + element.clientTop + (parseFloat(style.paddingTop) || 0),
      };
    }
    return null;
  })()`;
}

/** The most frames deep a frame is followed up, so a broken tree cannot loop. */
const DEEPEST = 32;

/**
 * Where the viewport of `frame` starts in the viewport of the page's top frame, in CSS pixels of the page.
 * @param {import('electron').WebFrameMain | null | undefined} frame The frame; the top frame or nothing gives no offset.
 * @returns {Promise<{ x: number, y: number } | null>} The offset, `{ x: 0, y: 0 }` for the top frame, or `null` when a
 *   frame on the way went away or refused the scripts.
 */
async function offsetOf(frame) {
  let x = 0;
  let y = 0;
  let current = frame;
  try {
    for (let depth = 0; current && current.parent && depth < DEEPEST; depth++) {
      const place = Number(await current.executeJavaScript(PLACE_SCRIPT));
      if (!Number.isInteger(place) || place < 0) return null;
      const box = /** @type {{ x: number, y: number } | null} */ (await current.parent.executeJavaScript(boxScript(place)));
      if (!box || !Number.isFinite(box.x) || !Number.isFinite(box.y)) return null;
      x += box.x;
      y += box.y;
      current = current.parent;
    }
  } catch {
    return null;
  }
  return { x, y };
}

module.exports = { offsetOf };
