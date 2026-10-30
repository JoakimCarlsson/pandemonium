'use strict';

/**
 * File pickers of a page (`<input type=file>`), opened by the reader's click.
 *
 * Electron asks a native open dialog for the files, parented to the page's window. The window here is
 * hidden and offscreen, so the dialog never appears or cannot be reached, and the input stays empty.
 * This file asks the protocol to hand the request over instead (`Page.setInterceptFileChooserDialog`),
 * opens the dialog itself with no parent, and gives the chosen files to the input.
 */

const { dialog } = require('electron');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */

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
    /** @type {boolean} Whether a native dialog is open, so that a second request waits for none. */
    this.open = false;
    if (page.cdp) {
      page.cdp.listen((method, params) => this.hear(method, params));
      page.cdp.call('Page.enable').then(
        () => (page.cdp ? page.cdp.call('Page.setInterceptFileChooserDialog', { enabled: true }) : undefined),
        () => {},
      ).catch(() => {});
    }
  }

  /**
   * Takes in one event of the protocol.
   * @param {string} method The event's name.
   * @param {Json} params What it carries.
   * @returns {void}
   */
  hear(method, params) {
    if (method !== 'Page.fileChooserOpened' || this.open) return;
    this.choose(Number(params.backendNodeId), params.mode === 'selectMultiple').catch(() => {});
  }

  /**
   * Asks the reader for files and gives them to the input.
   * @param {number} backendNodeId The input that asked.
   * @param {boolean} multiple Whether it takes more than one file.
   * @returns {Promise<void>} Resolves once the page has the files or the reader cancelled.
   */
  async choose(backendNodeId, multiple) {
    this.open = true;
    try {
      const result = await dialog.showOpenDialog({
        properties: multiple ? ['openFile', 'multiSelections'] : ['openFile'],
      });
      if (result.canceled || result.filePaths.length === 0 || !this.page.cdp) return;
      await this.page.cdp.call('DOM.setFileInputFiles', { files: result.filePaths, backendNodeId });
    } finally {
      this.open = false;
    }
  }
}

exports.FileChooser = FileChooser;
