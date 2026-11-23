'use strict';

/**
 * What a page reports beyond painting, for the pane to draw around it, and what the reader does with it from there:
 * the menu of a right click, full screen, sound, the address of the link under the pointer, tooltips, prompts,
 * printing, saving the page, reader view, files dropped on it and the zoom of a site. Every report is a `Chrome` event holding a `PageReport` of
 * `crates/pm-browser/src/chrome.rs`; every action a `Chrome` request holding a `PageAction`.
 */

const { app, ipcMain, shell } = require('electron');
const fs = require('fs');
const safety = require('./safety');
const blocker = require('./blocker');
const passwords = require('./passwords');
const reader = require('./reader');
const { dropFiles } = require('./drop');
const { offsetOf } = require('./frames');
const { toWindow } = require('./input');

/**
 * How many tooltip messages each page's contents sent, so a tooltip of a frame, which is placed only after its frame's
 * offset is found, is dropped when a later message came meanwhile.
 * @type {WeakMap<import('electron').WebContents, number>}
 */
const TOOLTIP_TURNS = new WeakMap();

/**
 * A coordinate as a whole number of pixels.
 * @param {unknown} value The coordinate as the page sent it.
 * @returns {number} It rounded, or 0 when it is not a number.
 */
function whole(value) {
  return Number.isFinite(Number(value)) ? Math.round(Number(value)) : 0;
}

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */

/**
 * Tells the window something about a page.
 * @param {Page} page The page.
 * @param {Json} report The report, a `PageReport`.
 * @returns {void}
 */
function report(page, report) {
  page.send({ Chrome: { page: page.id, report } });
}

/**
 * Starts telling the window what a page reports for the pane around it: right clicks, full screen, sound and the link under the pointer.
 * @param {Page} page The page.
 * @returns {void}
 */
function watch(page) {
  const contents = page.contents;
  contents.on('context-menu', (_event, params) => {
    const flags = params.editFlags || {};
    const history = contents.navigationHistory;
    report(page, {
      ContextMenu: {
        x: Math.round(params.x / page.inputScale()),
        y: Math.round(params.y / page.inputScale()),
        link_url: String(params.linkURL || ''),
        src_url: String(params.srcURL || ''),
        is_image: params.mediaType === 'image',
        selection_text: String(params.selectionText || '').slice(0, 10000),
        editable: params.isEditable === true,
        can_cut: flags.canCut === true,
        can_copy: flags.canCopy === true,
        can_paste: flags.canPaste === true,
        misspelled: String(params.misspelledWord || ''),
        suggestions: (params.dictionarySuggestions || []).slice(0, 8).map(String),
        can_back: history.canGoBack(),
        can_forward: history.canGoForward(),
      },
    });
  });
  contents.on('enter-html-full-screen', () => report(page, { Fullscreen: { on: true } }));
  contents.on('leave-html-full-screen', () => report(page, { Fullscreen: { on: false } }));
  contents.on('audio-state-changed', (event) => {
    const audible = /** @type {{ audible?: boolean }} */ (event).audible === true;
    report(page, { Audio: { audible, muted: contents.isAudioMuted() } });
  });
  contents.on('update-target-url', (_event, url) => report(page, { HoveredLink: { url: String(url || '') } }));
  contents.on('did-start-navigation', (details) => {
    if (details.isMainFrame && !details.isSameDocument) report(page, { Tooltip: { text: '', x: 0, y: 0 } });
  });
  if (!page.sealed) reader.watch(page, (made) => report(page, made));
}

/**
 * Prints the page to a PDF in the reader's Downloads folder, under a name nothing else has, opens it, and tells the window where it went.
 * @param {Page} page The page.
 * @returns {Promise<void>} Resolves once the window has been told.
 */
async function print(page) {
  try {
    const data = await page.contents.printToPDF({ printBackground: true });
    const title = page.contents.getTitle() || 'page';
    const target = safety.reserve(app.getPath('downloads'), safety.safeName(`${title}.pdf`));
    fs.writeFileSync(target, data);
    report(page, { Printed: { path: target, error: '' } });
    shell.openPath(target).catch(() => {});
  } catch (error) {
    report(page, { Printed: { path: '', error: String(error instanceof Error ? error.message : error) } });
  }
}

/**
 * Carries out what the reader asked of the page from the pane around it.
 * @param {Page} page The page.
 * @param {Json | string} action The action, a `PageAction`.
 * @returns {void}
 */
function act(page, action) {
  const contents = page.contents;
  if (action === 'LeaveFullscreen') {
    contents.executeJavaScript('document.fullscreenElement ? document.exitFullscreen() : undefined', true).catch(() => {});
    return;
  }
  if (action === 'Print') {
    print(page).catch(() => {});
    return;
  }
  if (action === 'SavePage') {
    safety.savePage(page).catch(() => {});
    return;
  }
  if (action === 'ReadArticle') {
    reader.read(page, (made) => report(page, made)).catch(() => {});
    return;
  }
  if (!action || typeof action !== 'object') return;
  if (action.AnswerDialog) {
    const { id, accept, text } = action.AnswerDialog;
    if (page.dialogs) page.dialogs.answerFromReader(Number(id), accept === true, String(text || ''));
  } else if (action.ReplaceMisspelling) {
    contents.replaceMisspelling(String(action.ReplaceMisspelling.word || ''));
  } else if (action.InspectAt) {
    const at = toWindow(Number(action.InspectAt.x), Number(action.InspectAt.y), page.inputScale());
    contents.inspectElement(at.x, at.y);
  } else if (action.CopyImageAt) {
    const at = toWindow(Number(action.CopyImageAt.x), Number(action.CopyImageAt.y), page.inputScale());
    contents.copyImageAt(at.x, at.y);
  } else if (action.Save) {
    const url = String(action.Save.url || '');
    if (/^(https?|data|blob):/i.test(url)) contents.downloadURL(url);
  } else if (action.CancelDownload) {
    safety.cancelDownload(Number(action.CancelDownload.id));
  } else if (action.ShowDownload) {
    safety.showDownload(Number(action.ShowDownload.id), false);
  } else if (action.OpenDownload) {
    safety.showDownload(Number(action.OpenDownload.id), true);
  } else if (action.Mute) {
    contents.setAudioMuted(action.Mute.muted === true);
    report(page, { Audio: { audible: contents.isCurrentlyAudible(), muted: contents.isAudioMuted() } });
  } else if (action.ChooseScreen) {
    safety.chooseScreen(Number(action.ChooseScreen.id), action.ChooseScreen.source ?? null);
  } else if (action.DropFiles) {
    const { x, y, paths } = action.DropFiles;
    dropFiles(page, Number(x), Number(y), paths).catch(() => {});
  } else if (action.AnswerPassword) {
    const { id, save, never } = action.AnswerPassword;
    passwords.answer(Number(id), save === true, never === true).catch(() => {});
  } else if (action.PrimaryPassword) {
    const typed = action.PrimaryPassword.password;
    passwords.primaryPassword(page, typeof typed === 'string' ? typed : null).catch(() => {});
  } else if (action.FillLogin) {
    passwords.fill(page, String(action.FillLogin.username ?? '')).catch(() => {});
  } else if (action.SiteBlocking) {
    blocker.setSite(page, action.SiteBlocking.on === true);
  } else if (action.SetZoom) {
    const factor = Number(action.SetZoom.factor);
    if (Number.isFinite(factor) && factor >= 0.25 && factor <= 5) {
      page.setReaderZoom(factor);
      page.reportZoom();
    }
  }
}

/**
 * Listens for what the preload of every page sends: a `prompt` the page waits on, and the title of what the pointer rests on.
 * @param {(contents: import('electron').WebContents | null) => Page | undefined} pageOf Finds the page of a window's contents.
 * @returns {void}
 */
function install(pageOf) {
  ipcMain.on('pandemonium:prompt', (event, message, value) => {
    const page = pageOf(event.sender);
    let answered = false;
    /**
     * Gives the waiting page its answer, once.
     * @param {string | null} answer The text, or `null` when the prompt was cancelled.
     * @returns {void}
     */
    const reply = (answer) => {
      if (answered) return;
      answered = true;
      event.returnValue = answer;
    };
    if (!page || !page.dialogs) {
      reply(null);
      return;
    }
    const frame = event.senderFrame;
    page.dialogs.askPrompt(String(message ?? ''), String(value ?? ''), reply, frame && frame.parent ? frame.url : undefined);
  });
  ipcMain.on('pandemonium:tooltip', (event, text, x, y) => {
    const page = pageOf(event.sender);
    if (!page) return;
    const shown = String(text || '').slice(0, 1000);
    const turn = (TOOLTIP_TURNS.get(event.sender) || 0) + 1;
    TOOLTIP_TURNS.set(event.sender, turn);
    const frame = event.senderFrame;
    if (!shown || !frame || !frame.parent) {
      report(page, { Tooltip: { text: shown, x: whole(x), y: whole(y) } });
      return;
    }
    offsetOf(frame).then((offset) => {
      if (!offset || TOOLTIP_TURNS.get(event.sender) !== turn) return;
      report(page, { Tooltip: { text: shown, x: whole(Number(x) + offset.x), y: whole(Number(y) + offset.y) } });
    });
  });
}

module.exports = { act, install, watch };
