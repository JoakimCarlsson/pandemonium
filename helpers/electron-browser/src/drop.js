'use strict';

/**
 * Files the reader drags from the desktop and lets go over a page: the window hands their paths and the point, and
 * they are dropped on the page through its DevTools protocol as a drag from another program is, entering, moving over
 * and let go at that point, so drop zones that upload what is dropped on them get the files. A page that does not take
 * them would open the first file, which the page's guard against leaving the web stops.
 */

const fs = require('fs');
const path = require('path');
const { toWindow } = require('./input');

/** The operations a drop of files allows, as the protocol counts them: copying. */
const COPY = 1;

/**
 * The paths among `paths` that are absolute and name files that exist.
 * @param {unknown} paths The paths the window sent.
 * @returns {string[]} The files.
 */
function existingFiles(paths) {
  return (Array.isArray(paths) ? paths : []).filter((file) => {
    if (typeof file !== 'string' || !path.isAbsolute(file)) return false;
    try {
      return fs.statSync(file).isFile();
    } catch {
      return false;
    }
  });
}

/**
 * Drops files on a page at a point, as letting go of a drag of them from a file manager there does. Pages without a
 * protocol, such as sealed pages and the developer tools, take no files.
 * @param {import('./page').Page} page The page.
 * @param {number} x Logical pixels from the left of the page.
 * @param {number} y Logical pixels from the top of the page.
 * @param {unknown} paths The absolute paths of the files.
 * @returns {Promise<void>} Settles once the drop was handed to the page, or it was clear it could not be.
 */
async function dropFiles(page, x, y, paths) {
  const files = existingFiles(paths);
  if (page.sealed || !page.cdp || files.length === 0 || page.contents.isDestroyed()) return;
  const at = toWindow(Number(x) || 0, Number(y) || 0, page.paneScale);
  const units = page.topUnits();
  const point = { x: at.x / units, y: at.y / units };
  const data = { items: [], files, dragOperationsMask: COPY };
  try {
    for (const type of ['dragEnter', 'dragOver', 'drop']) {
      await page.cdp.call('Input.dispatchDragEvent', { type, ...point, data });
    }
  } catch {
    await page.cdp.call('Input.dispatchDragEvent', { type: 'dragCancel', ...point, data }).catch(() => {});
  }
}

module.exports = { dropFiles, existingFiles };
