'use strict';

/**
 * Splits a stream of bytes into lines of text, decoding UTF-8 only once a line is whole, so a character whose bytes
 * arrive in two chunks is never broken.
 */

/** The byte that ends a line. */
const NEWLINE = 0x0a;

/**
 * Gathers chunks of bytes and gives back each whole line they hold.
 */
class LineReader {
  /**
   * A reader holding nothing yet.
   */
  constructor() {
    /** @type {Buffer[]} The chunks that arrived since the last whole line, not yet ending one. */
    this.held = [];
  }

  /**
   * Takes in `chunk` and gives back the lines it finishes, without their newline, decoded as UTF-8.
   * @param {Buffer} chunk Bytes as they arrived.
   * @returns {string[]} The whole lines, in order; the bytes after the last newline are kept for the next chunk.
   */
  push(chunk) {
    /** @type {string[]} */
    const lines = [];
    let start = 0;
    let end = chunk.indexOf(NEWLINE, start);
    while (end >= 0) {
      const tail = chunk.subarray(start, end);
      const whole = this.held.length > 0 ? Buffer.concat([...this.held, tail]) : tail;
      this.held = [];
      lines.push(whole.toString('utf8'));
      start = end + 1;
      end = chunk.indexOf(NEWLINE, start);
    }
    if (start < chunk.length) this.held.push(chunk.subarray(start));
    return lines;
  }
}

exports.LineReader = LineReader;
