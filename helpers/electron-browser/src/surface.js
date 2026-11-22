'use strict';

/**
 * The two files a page paints into: the window maps them and reads a whole picture whenever the
 * counter at the start of a file is even and the same before and after the copy.
 */

const fs = require('fs');
const path = require('path');

/** How many bytes of a file come before the pixels. */
const HEADER = 16;

/** The tag at byte 8 of a file that says its pixels are BGRA; a file without it holds RGBA. */
const FORMAT = Buffer.from('BGRA', 'latin1');

/** How long files of an earlier size are kept after newer ones are announced, in milliseconds. */
const RETIRE_MS = 2000;

/**
 * A band of whole rows of a picture.
 * @typedef {object} Rows
 * @property {number} top The first row in the band.
 * @property {number} bottom The row just past the band.
 */

/**
 * The rows of a picture `height` device pixels tall that a paint's dirty rectangle covers. The rectangle may be in
 * logical or in device pixels, so the band runs from its top as device pixels to its bottom scaled by `scale`, which
 * covers it either way.
 * @param {import('electron').Rectangle | undefined} dirty The part that changed, or nothing for the whole picture.
 * @param {number} scale Device pixels per logical pixel.
 * @param {number} height The height of the picture, in device pixels.
 * @returns {Rows} The rows to write.
 */
function dirtyRows(dirty, scale, height) {
  const factor = Number.isFinite(scale) && scale >= 1 ? scale : 1;
  if (!dirty || !Number.isFinite(dirty.y) || !Number.isFinite(dirty.height) || dirty.height <= 0) {
    return { top: 0, bottom: height };
  }
  const top = Math.max(0, Math.min(height, Math.floor(dirty.y)));
  const bottom = Math.max(top, Math.min(height, Math.ceil((dirty.y + dirty.height) * factor)));
  return { top, bottom };
}

/**
 * The smallest band holding both `held` and `added`.
 * @param {Rows | null} held The rows a file still lacks, or nothing.
 * @param {Rows} added The rows that changed since.
 * @returns {Rows} Both bands as one.
 */
function joinRows(held, added) {
  if (!held) return { ...added };
  return { top: Math.min(held.top, added.top), bottom: Math.max(held.bottom, added.bottom) };
}

/**
 * The width of a painted image in logical pixels, at least 1.
 * @param {import('electron').NativeImage} image What the page painted.
 * @returns {number} Its logical width.
 */
function logicalWidth(image) {
  const factor = Math.max(1, ...image.getScaleFactors());
  return Math.max(1, image.getSize(factor).width);
}

/** The highest display scale a page is painted at. */
const MOST_SCALE = 4;

/**
 * The display scale to paint a page at: the window's, kept between 1 and {@link MOST_SCALE}, or 1 when it gave none
 * that is usable.
 * @param {unknown} scale The scale the window gave, device pixels per logical pixel.
 * @returns {number} The scale to paint at.
 */
function displayScale(scale) {
  if (typeof scale !== 'number' || !Number.isFinite(scale) || scale <= 0) return 1;
  return Math.min(MOST_SCALE, Math.max(1, scale));
}

/**
 * The device scale every offscreen window paints at. The engine fixes it when it makes a window and, on some
 * desktops, such as Wayland, ignores any other, so pages are painted at 1 into a window sized in device pixels and
 * render at the pane's display scale through their zoom or emulation (see `Page#carryScale` and `applyViewport`).
 */
const ENGINE_SCALE = 1;

/**
 * The length in device pixels of a side of a pane, the size its window is given.
 * @param {number} side The side, in logical pixels.
 * @param {number} scale The pane's display scale, device pixels per logical pixel.
 * @returns {number} The side in device pixels, rounded and at least 1.
 */
function deviceSide(side, scale) {
  return Math.max(1, Math.round(side * scale));
}

/**
 * The device pixels of a painted image, at the sharpest scale it holds. A page painted at a display scale above 1
 * gives an image whose size is in logical pixels and whose bitmap at that scale holds the device pixels.
 * @param {import('electron').NativeImage} image What the page painted.
 * @returns {{ width: number, height: number, bgra: Buffer }} Its width and height in device pixels and its pixels as BGRA; a size of 0 when it is empty.
 */
function pixels(image) {
  const factor = Math.max(1, ...image.getScaleFactors());
  const logical = image.getSize(factor);
  const bgra = image.toBitmap({ scaleFactor: factor });
  const count = bgra.length / 4;
  const guess = Math.round(logical.width * factor);
  const tall = logical.height * factor;
  const width = [0, 1, -1, 2, -2]
    .map((step) => guess + step)
    .filter((side) => side > 0 && count % side === 0)
    .sort((a, b) => Math.abs(count / a - tall) - Math.abs(count / b - tall))[0];
  if (!width || count === 0) return { width: 0, height: 0, bgra };
  return { width, height: count / width, bgra };
}

/**
 * The pair of files a page paints into, written in turn.
 */
class Surface {
  /**
   * A surface in `folder` for the page `page`.
   * @param {string} folder The folder the window and the helper share.
   * @param {number} page The page the files belong to.
   * @param {(width: number, height: number, paths: [string, string]) => void} announce Tells the window where the files are, each time they are made.
   */
  constructor(folder, page, announce) {
    /** @type {string} The folder the window and the helper share. */
    this.folder = folder;
    /** @type {number} The page the files belong to. */
    this.page = page;
    /** @type {(width: number, height: number, paths: [string, string]) => void} Tells the window where the files are. */
    this.announce = announce;
    /** @type {{ paths: [string, string], descriptors: [number, number], counters: [bigint, bigint], pending: [Rows | null, Rows | null] } | null} The open files, and the rows each still lacks. */
    this.files = null;
    /** @type {{ paths: string[], at: number }[]} Files of earlier sizes, kept a while so the window can still map them, then removed. */
    this.retired = [];
    /** @type {Buffer} The counter of a file as it is written, reused for every paint. */
    this.counter = Buffer.alloc(8);
    /** @type {[number, number]} The size of a picture in the open files, in pixels. */
    this.size = [0, 0];
    /** @type {number} How many pictures were written. */
    this.generation = 0;
    /** @type {number} How many screenshots were saved. */
    this.pictures = 0;
  }

  /**
   * Opens two files for pictures `width` by `height` and tells the window where they are.
   * @param {number} width The width of a picture, in pixels.
   * @param {number} height The height of a picture, in pixels.
   * @returns {void}
   */
  open(width, height) {
    this.retire();
    this.sweep(false);
    const bytes = HEADER + width * height * 4;
    const stamp = `${this.page}-${width}x${height}-${Date.now()}-${this.generation}`;
    /** @type {[string, string]} */
    const paths = [path.join(this.folder, `page-${stamp}-0.rgba`), path.join(this.folder, `page-${stamp}-1.rgba`)];
    const opened = paths.map((file) => {
      const descriptor = fs.openSync(file, 'w+');
      fs.ftruncateSync(descriptor, bytes);
      fs.writeSync(descriptor, FORMAT, 0, FORMAT.length, 8);
      return descriptor;
    });
    this.files = {
      paths,
      descriptors: [opened[0], opened[1]],
      counters: [0n, 0n],
      pending: [{ top: 0, bottom: height }, { top: 0, bottom: height }],
    };
    this.size = [width, height];
    this.announce(width, height, paths);
  }

  /**
   * Writes the painted `image`, as BGRA, into the next file. Only the rows that changed since that file was last
   * written go in: the rows `dirty` names joined with those of the paint before, since the two files take turns.
   * @param {import('electron').NativeImage} image What the page painted.
   * @param {Electron.Rectangle} [dirty] The part that changed, as the paint event reports it; the whole picture when missing.
   * @returns {{ buffer: number, generation: number } | null} Which file holds it and which paint it was, or nothing when the image is empty.
   */
  write(image, dirty) {
    const { width, height, bgra } = pixels(image);
    if (width === 0 || height === 0 || bgra.length < width * height * 4) return null;
    if (!this.files || this.size[0] !== width || this.size[1] !== height) {
      this.open(width, height);
    }
    if (this.retired.length > 0) this.sweep(false);
    const files = /** @type {NonNullable<Surface['files']>} */ (this.files);
    const changed = dirtyRows(dirty, width / logicalWidth(image), height);
    files.pending = [joinRows(files.pending[0], changed), joinRows(files.pending[1], changed)];
    this.generation += 1;
    const buffer = this.generation % 2;
    const descriptor = files.descriptors[buffer];
    const rows = files.pending[buffer];
    const before = files.counters[buffer];
    this.counter.writeBigUInt64LE(before + 1n);
    fs.writeSync(descriptor, this.counter, 0, 8, 0);
    if (rows && rows.bottom > rows.top) {
      const start = rows.top * width * 4;
      fs.writeSync(descriptor, bgra, start, (rows.bottom - rows.top) * width * 4, HEADER + start);
    }
    this.counter.writeBigUInt64LE(before + 2n);
    fs.writeSync(descriptor, this.counter, 0, 8, 0);
    files.counters[buffer] = before + 2n;
    files.pending[buffer] = null;
    return { buffer, generation: this.generation };
  }

  /**
   * Closes the open files and keeps their names to remove later, since the window may still be mapping them for
   * the pictures it was told of before newer files were announced.
   * @returns {void}
   */
  retire() {
    if (!this.files) return;
    for (const descriptor of this.files.descriptors) {
      try {
        fs.closeSync(descriptor);
      } catch (_) {
        /* already closed */
      }
    }
    this.retired.push({ paths: [...this.files.paths], at: Date.now() });
    this.files = null;
  }

  /**
   * Removes the retired files that have waited {@link RETIRE_MS}, or all of them. A file that cannot be removed yet,
   * as on Windows while the window still maps it, is tried again on a later call unless `all` is set.
   * @param {boolean} all Whether to remove every retired file now, however recently it was retired.
   * @returns {void}
   */
  sweep(all) {
    const now = Date.now();
    this.retired = this.retired.flatMap((entry) => {
      if (!all && now - entry.at < RETIRE_MS) return [entry];
      const left = entry.paths.filter((file) => {
        try {
          fs.unlinkSync(file);
          return false;
        } catch (error) {
          return /** @type {NodeJS.ErrnoException} */ (error).code !== 'ENOENT';
        }
      });
      return left.length > 0 && !all ? [{ paths: left, at: entry.at }] : [];
    });
  }

  /**
   * Saves a picture beside the shared files.
   * @param {Buffer} bytes The picture, as the file's contents.
   * @param {string} [extension] The file's extension, without the dot; a PNG file unless said otherwise.
   * @returns {string} Where it was saved.
   */
  savePicture(bytes, extension = 'png') {
    this.pictures += 1;
    const file = path.join(this.folder, `pick-${this.page}-${this.pictures}.${extension}`);
    fs.writeFileSync(file, bytes);
    return file;
  }

  /**
   * Closes the shared files and removes them with every retired one.
   * @returns {void}
   */
  close() {
    this.retire();
    this.sweep(true);
  }
}

exports.Surface = Surface;
exports.pixels = pixels;
exports.displayScale = displayScale;
exports.dirtyRows = dirtyRows;
exports.joinRows = joinRows;
exports.ENGINE_SCALE = ENGINE_SCALE;
exports.deviceSide = deviceSide;
