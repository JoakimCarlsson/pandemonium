'use strict';

/**
 * File pickers of a page (`<input type=file>`), opened by the reader's click.
 *
 * Electron asks a native open dialog for the files, parented to the page's window. The window here is
 * hidden and offscreen, so the dialog never appears or cannot be reached, and the input stays empty.
 * This file asks the protocol to hand the request over instead (`Page.setInterceptFileChooserDialog`),
 * opens the dialog itself with no parent, and gives the chosen files to the input.
 *
 * On macOS the helper hides its Dock icon, which makes it a background app, and a background app's dialog
 * opens behind every other window or not at all. The app is brought to the front before the dialog opens.
 */

const { app, dialog } = require('electron');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */

/**
 * Brings the helper to the front on macOS, where a background app's open dialog would not show.
 * @returns {void}
 */
function raiseApp() {
  if (process.platform !== 'darwin') return;
  try {
    app.focus({ steal: true });
  } catch (_) {}
}

/** The file name extensions a few common MIME types and families stand for, for the filters of the dialog. */
const MIME_EXTENSIONS = {
  'image/*': ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp', 'ico', 'avif', 'heic', 'tif', 'tiff'],
  'video/*': ['mp4', 'webm', 'mov', 'mkv', 'avi', 'm4v', 'ogv'],
  'audio/*': ['mp3', 'wav', 'ogg', 'oga', 'flac', 'm4a', 'aac', 'opus', 'weba'],
  'text/*': ['txt', 'csv', 'md', 'html', 'htm', 'css', 'js', 'json', 'xml'],
  'image/png': ['png'],
  'image/jpeg': ['jpg', 'jpeg'],
  'image/gif': ['gif'],
  'image/webp': ['webp'],
  'image/svg+xml': ['svg'],
  'application/pdf': ['pdf'],
  'application/json': ['json'],
  'application/zip': ['zip'],
  'text/plain': ['txt'],
  'text/csv': ['csv'],
  'text/html': ['html', 'htm'],
  'text/markdown': ['md'],
};

/**
 * The filters of the open dialog for an input's `accept` attribute: one filter of every extension it names, then all files, since the attribute is only a hint.
 * @param {string} accept The attribute, such as `image/*,.pdf`.
 * @returns {import('electron').FileFilter[]} The filters; none when the attribute names nothing the dialog can filter by.
 */
function filtersOf(accept) {
  /** @type {Set<string>} */
  const extensions = new Set();
  for (const raw of String(accept || '').split(',')) {
    const token = raw.trim().toLowerCase();
    if (!token) continue;
    if (token.startsWith('.')) {
      if (/^\.[a-z0-9_+-]+$/.test(token)) extensions.add(token.slice(1));
    } else if (Object.prototype.hasOwnProperty.call(MIME_EXTENSIONS, token)) {
      for (const extension of MIME_EXTENSIONS[/** @type {keyof typeof MIME_EXTENSIONS} */ (token)]) extensions.add(extension);
    }
  }
  if (extensions.size === 0) return [];
  return [
    { name: 'Accepted files', extensions: [...extensions] },
    { name: 'All files', extensions: ['*'] },
  ];
}

/**
 * A request of the page for files, waiting for its turn.
 * @typedef {object} FileRequest
 * @property {number} backendNodeId The input that asked.
 * @property {boolean} multiple Whether it takes more than one file.
 */

/**
 * Takes over the file pickers of a page.
 */
class FileChooser {
  /**
   * Starts listening to a page.
   * @param {Page} page The page, which must have a protocol.
   */
  constructor(page) {
    /** @type {Page} The page. */
    this.page = page;
    /** @type {boolean} Whether a native dialog is open, so that a second request waits its turn. */
    this.open = false;
    /** @type {FileRequest[]} The requests that came while a dialog was open, oldest first. */
    this.waiting = [];
    if (page.cdp) {
      page.cdp.listen((method, params) => this.hear(method, params));
      page.cdp.call('Page.enable').then(
        () => (page.cdp ? page.cdp.call('Page.setInterceptFileChooserDialog', { enabled: true }) : undefined),
        () => {},
      ).catch(() => {});
    }
  }

  /**
   * Takes in one event of the protocol: a request for files is asked at once, or after the dialog open now, never dropped.
   * @param {string} method The event's name.
   * @param {Json} params What it carries.
   * @returns {void}
   */
  hear(method, params) {
    if (method !== 'Page.fileChooserOpened') return;
    const request = { backendNodeId: Number(params.backendNodeId), multiple: params.mode === 'selectMultiple' };
    if (this.waiting.some((held) => held.backendNodeId === request.backendNodeId)) return;
    this.waiting.push(request);
    this.next();
  }

  /**
   * Opens the dialog of the oldest waiting request, unless one is open.
   * @returns {void}
   */
  next() {
    if (this.open) return;
    const request = this.waiting.shift();
    if (!request) return;
    this.choose(request.backendNodeId, request.multiple)
      .catch(() => {})
      .finally(() => this.next());
  }

  /**
   * What the input that asked takes: the filters of its `accept` attribute, and whether it takes a folder.
   * @param {number} backendNodeId The input.
   * @returns {Promise<{ accept: string, folder: boolean }>} What it takes; nothing special when it cannot be read.
   */
  async inputOf(backendNodeId) {
    const cdp = this.page.cdp;
    if (!cdp) return { accept: '', folder: false };
    try {
      const { object } = await cdp.call('DOM.resolveNode', { backendNodeId });
      const { result } = await cdp.call('Runtime.callFunctionOn', {
        objectId: object.objectId,
        functionDeclaration: 'function () { return { accept: String(this.accept || ""), folder: Boolean(this.webkitdirectory) }; }',
        returnByValue: true,
      });
      cdp.call('Runtime.releaseObject', { objectId: object.objectId }).catch(() => {});
      const value = result && result.value ? result.value : {};
      return { accept: String(value.accept || ''), folder: value.folder === true };
    } catch (_) {
      return { accept: '', folder: false };
    }
  }

  /**
   * Asks the reader for files, or a folder when the input takes one, filtered by what the input accepts, and gives them to the input.
   * @param {number} backendNodeId The input that asked.
   * @param {boolean} multiple Whether it takes more than one file.
   * @returns {Promise<void>} Resolves once the page has the files or the reader cancelled.
   */
  async choose(backendNodeId, multiple) {
    this.open = true;
    try {
      const { accept, folder } = await this.inputOf(backendNodeId);
      raiseApp();
      /** @type {import('electron').OpenDialogOptions['properties']} */
      const properties = folder ? ['openDirectory'] : multiple ? ['openFile', 'multiSelections'] : ['openFile'];
      const result = await dialog.showOpenDialog({ properties, filters: folder ? [] : filtersOf(accept) });
      if (result.canceled || result.filePaths.length === 0 || !this.page.cdp) return;
      await this.page.cdp.call('DOM.setFileInputFiles', { files: result.filePaths, backendNodeId });
    } finally {
      this.open = false;
    }
  }
}

exports.FileChooser = FileChooser;
exports.filtersOf = filtersOf;
