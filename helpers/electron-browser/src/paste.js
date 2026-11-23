'use strict';

/**
 * Pasting what the window's clipboard held beyond plain text: its rich text as HTML and files, such as a picture the
 * window saved from the clipboard. The hidden engine cannot read the system clipboard, so the page is handed a paste
 * event of its own carrying all of it, in the frame that holds the keyboard, as rich editors and upload fields read
 * a paste; when the page does not take it, an editable region gets the HTML and a plain field the text.
 */

const fs = require('fs');
const path = require('path');

/** The most bytes of files one paste carries into a page; files past it are left out. */
const LARGEST_PASTE = 25 * 1024 * 1024;

/**
 * The media types of the files a paste most often carries, by extension in lower case.
 * @type {Readonly<Record<string, string>>}
 */
const MEDIA_TYPES = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.bmp': 'image/bmp',
  '.svg': 'image/svg+xml',
  '.pdf': 'application/pdf',
  '.txt': 'text/plain',
  '.md': 'text/markdown',
  '.csv': 'text/csv',
  '.json': 'application/json',
  '.html': 'text/html',
  '.zip': 'application/zip',
};

/**
 * A file read for a paste, as the script in the page takes it.
 * @typedef {object} PastedFile
 * @property {string} name Its name, without folders.
 * @property {string} type Its media type.
 * @property {string} data Its bytes in base64.
 */

/**
 * The media type of a file, from its extension.
 * @param {string} file The file's path.
 * @returns {string} The type, or `application/octet-stream` when the extension says nothing known.
 */
function mediaType(file) {
  return MEDIA_TYPES[path.extname(file).toLowerCase()] || 'application/octet-stream';
}

/**
 * Reads the files a paste carries, in order, up to {@link LARGEST_PASTE} bytes in all; a path that is not a file that
 * can be read is left out.
 * @param {unknown} paths The absolute paths the window sent.
 * @returns {PastedFile[]} The files read.
 */
function readFiles(paths) {
  /** @type {PastedFile[]} */
  const files = [];
  let total = 0;
  for (const file of Array.isArray(paths) ? paths : []) {
    if (typeof file !== 'string' || !path.isAbsolute(file)) continue;
    try {
      const stat = fs.statSync(file);
      if (!stat.isFile() || total + stat.size > LARGEST_PASTE) continue;
      total += stat.size;
      files.push({ name: path.basename(file), type: mediaType(file), data: fs.readFileSync(file).toString('base64') });
    } catch {
      continue;
    }
  }
  return files;
}

/**
 * The script that offers the page a paste of `given` in the frame it runs in and says what became of it: `taken` when
 * the page handled the event itself, `html` when the HTML went into the editable region that holds the keyboard, and
 * `text` when the plain text is still to be typed.
 * @param {{ text: string, html: string, files: PastedFile[] }} given What the paste carries.
 * @returns {string} The script.
 */
function pasteScript(given) {
  return `(() => {
  const given = ${JSON.stringify(given)};
  let target = document.activeElement || document.body;
  while (target && target.shadowRoot && target.shadowRoot.activeElement) target = target.shadowRoot.activeElement;
  if (!target) return 'text';
  const data = new DataTransfer();
  if (given.text) data.setData('text/plain', given.text);
  if (given.html) data.setData('text/html', given.html);
  for (const file of given.files) {
    const bytes = Uint8Array.from(atob(file.data), (c) => c.charCodeAt(0));
    data.items.add(new File([bytes], file.name, { type: file.type }));
  }
  const event = new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true, composed: true });
  if (!target.dispatchEvent(event)) return 'taken';
  if (given.html && target.isContentEditable && document.execCommand('insertHTML', false, given.html)) return 'html';
  return 'text';
})()`;
}

/**
 * Pastes text with its HTML and files into the frame of `contents` that holds the keyboard, as described above; the
 * text is typed with `typeText` when the page did not take the paste and no HTML went in, or when the frame cannot be
 * reached.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {{ text: string, html?: string | null, files?: string[] }} body What the window's clipboard held.
 * @param {(contents: import('electron').WebContents, text: string) => void} typeText Types plain text into the page.
 * @returns {Promise<void>} Settles once the paste is done.
 */
async function pasteRich(contents, body, typeText) {
  const text = typeof body.text === 'string' ? body.text : '';
  const html = typeof body.html === 'string' ? body.html : '';
  const files = readFiles(body.files);
  let outcome = 'text';
  try {
    const frame = contents.focusedFrame || contents.mainFrame;
    outcome = String(await frame.executeJavaScript(pasteScript({ text, html, files }), true));
  } catch {
    outcome = 'text';
  }
  if (outcome === 'text' && text && !contents.isDestroyed()) typeText(contents, text);
}

/**
 * Whether a paste carries more than plain text, and so is offered to the page as a paste event of its own.
 * @param {{ html?: string | null, files?: string[] }} body The paste.
 * @returns {boolean} Whether it has HTML or files.
 */
function isRich(body) {
  return (typeof body.html === 'string' && body.html.length > 0) || (Array.isArray(body.files) && body.files.length > 0);
}

module.exports = { isRich, mediaType, pasteRich, pasteScript, readFiles };
