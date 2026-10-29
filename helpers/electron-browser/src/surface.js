'use strict';

/**
 * The two files a page paints into: the window maps them and reads a whole picture whenever the
 * counter at the start of a file is even and the same before and after the copy.
 */

const fs = require('fs');
const path = require('path');

/** How many bytes of a file come before the pixels. */
const HEADER = 16;

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
    /** @type {{ paths: [string, string], descriptors: [number, number], counters: [bigint, bigint], scratch: Buffer } | null} The open files, and what is needed to fill them. */
    this.files = null;
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
    this.close();
    const bytes = HEADER + width * height * 4;
    const stamp = `${this.page}-${width}x${height}-${Date.now()}`;
    /** @type {[string, string]} */
    const paths = [path.join(this.folder, `page-${stamp}-0.rgba`), path.join(this.folder, `page-${stamp}-1.rgba`)];
    const opened = paths.map((file) => {
      const descriptor = fs.openSync(file, 'w+');
      fs.ftruncateSync(descriptor, bytes);
      return descriptor;
    });
    this.files = {
      paths,
      descriptors: [opened[0], opened[1]],
      counters: [0n, 0n],
      scratch: Buffer.alloc(width * height * 4),
    };
    this.size = [width, height];
    this.announce(width, height, paths);
  }

  /**
   * Writes the painted `image`, as RGBA, into the next file.
   * @param {import('electron').NativeImage} image What the page painted.
   * @returns {{ buffer: number, generation: number } | null} Which file holds it and which paint it was, or nothing when the image is empty.
   */
  write(image) {
    const { width, height } = image.getSize();
    if (width === 0 || height === 0) return null;
    if (!this.files || this.size[0] !== width || this.size[1] !== height) {
      this.open(width, height);
    }
    const files = /** @type {NonNullable<Surface['files']>} */ (this.files);
    const bgra = image.toBitmap();
    const out = files.scratch;
    const from = new Uint32Array(bgra.buffer, bgra.byteOffset, bgra.length >> 2);
    const to = new Uint32Array(out.buffer, out.byteOffset, out.length >> 2);
    for (let i = 0; i < from.length; i += 1) {
      const pixel = from[i];
      to[i] = (pixel & 0xff00ff00) | ((pixel & 0xff) << 16) | ((pixel >>> 16) & 0xff);
    }
    this.generation += 1;
    const buffer = this.generation % 2;
    const descriptor = files.descriptors[buffer];
    const counter = Buffer.alloc(8);
    const before = files.counters[buffer];
    counter.writeBigUInt64LE(before + 1n);
    fs.writeSync(descriptor, counter, 0, 8, 0);
    fs.writeSync(descriptor, out, 0, out.length, HEADER);
    counter.writeBigUInt64LE(before + 2n);
    fs.writeSync(descriptor, counter, 0, 8, 0);
    files.counters[buffer] = before + 2n;
    return { buffer, generation: this.generation };
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
   * Closes and removes the shared files.
   * @returns {void}
   */
  close() {
    if (!this.files) return;
    for (const descriptor of this.files.descriptors) {
      try {
        fs.closeSync(descriptor);
      } catch (_) {
        /* already closed */
      }
    }
    for (const file of this.files.paths) {
      try {
        fs.unlinkSync(file);
      } catch (_) {
        /* already gone */
      }
    }
    this.files = null;
  }
}

exports.Surface = Surface;
