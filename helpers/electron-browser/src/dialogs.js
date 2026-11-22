'use strict';

/**
 * JavaScript dialogs (`alert`, `confirm`, `prompt`) and `beforeunload` prompts of a page, kept from
 * hanging the page or an agent's call.
 *
 * What Electron does with them in an offscreen page, read from its source (`shell/browser/api/
 * electron_api_web_contents.cc` and `lib/browser/api/web-contents.ts`, `main` branch):
 *
 * - `alert` and `confirm` stop the page's renderer and ask `dialog.showMessageBox` for a native
 *   message box. Offscreen pages have no parent window for it, so it would open as a free-standing
 *   box that nothing in the pane shows or answers, and the page stays stopped until someone does.
 *   While the renderer is stopped, `Runtime.evaluate` and friends never answer either.
 * - `prompt` is cancelled at once, without a box: `prompt()` returns `null`. The preload therefore puts its own
 *   `window.prompt` in the page, which waits on a synchronous message to the helper; {@link Dialogs.askPrompt} answers it.
 * - `beforeunload`: Electron emits `will-prevent-unload` and, when nobody calls `preventDefault`,
 *   cancels the navigation without a word (`did-fail-load` with `ERR_ABORTED`).
 * - The DevTools protocol sees `alert` and `confirm` too: `Page.javascriptDialogOpening` while the
 *   page is stopped, and `Page.handleJavaScriptDialog` answers it.
 *
 * So this file answers through the protocol, keeps the native box from opening, asks the reader with a sheet the
 * pane draws, makes every tool return instead of waiting on a stopped page, and gives up on a dialog nobody answers.
 */

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */

/** How long a dialog waits for an agent to answer before it is dismissed, in milliseconds. */
const PATIENCE = 60000;

/** How long past `PATIENCE` the native box is held back before it answers by itself, in milliseconds. */
const NATIVE_GRACE = 5000;

/** How many boxes may be shown to the reader within `BOX_WINDOW` before the page's further dialogs are dismissed unseen. */
const BOX_LIMIT = 3;

/** The span, in milliseconds, in which `BOX_LIMIT` boxes may be shown. */
const BOX_WINDOW = 30000;

/** The number the next dialog shown to the reader gets. */
let nextDialog = 1;

/**
 * The host of a page's address, for the sheet's title.
 * @param {string} url The address.
 * @returns {string} Its host, or nothing for an address without a web host.
 */
function hostOf(url) {
  try {
    const { protocol, host } = new URL(url);
    return (protocol === 'http:' || protocol === 'https:') && host ? host : '';
  } catch (_) {
    return '';
  }
}

/** The most dialogs kept in the list of ones that closed since an agent was last told. */
const KEPT = 20;

/**
 * A dialog that is open.
 * @typedef {object} OpenDialog
 * @property {string} type `alert`, `confirm` or `prompt`.
 * @property {string} message What it says.
 * @property {string} defaultPrompt The text a prompt starts with.
 * @property {string} url The address of the page that opened it.
 */

/**
 * What a page's dialogs are doing: which one is open, which closed unseen, and when to give up.
 */
class Dialogs {
  /**
   * Starts listening to a page.
   * @param {Page} page The page, which must have a protocol.
   */
  constructor(page) {
    /** @type {Page} The page. */
    this.page = page;
    /** @type {OpenDialog | null} The dialog that is open, if one is. */
    this.pending = null;
    /** @type {string[]} Sentences about dialogs that closed since an agent was last told. */
    this.closed = [];
    /** @type {boolean} Whether this module itself is answering the open dialog, so that its closing is told as the agent's doing. */
    this.answering = false;
    /** @type {number} How many tool calls are running; a leaving page is let go only then. */
    this.busy = 0;
    /** @type {ReturnType<typeof setTimeout> | null} The timer that dismisses an open dialog nobody answered. */
    this.timer = null;
    /** @type {number[]} When boxes were last shown to the reader, for limiting a page that keeps opening dialogs. */
    this.boxes = [];
    /** @type {(() => void) | null} Takes back the native box that asks the reader, while one is open. */
    this.withdraw = null;
    /** @type {Array<() => void>} Who wants to know when a dialog opens. */
    this.watchers = [];
    /** @type {number} The number of the dialog the pane shows a sheet for, or 0 when it shows none. */
    this.shownId = 0;
    /** @type {((answer: string | null) => void) | null} Gives the open prompt its answer, while a prompt is open. */
    this.promptReply = null;
    if (page.cdp) page.cdp.listen((method, params) => this.hear(method, params));
    page.contents.on('will-prevent-unload', (event) => this.unloading(event));
  }

  /**
   * Takes in one event of the protocol.
   * @param {string} method The event's name.
   * @param {Json} params What it carries.
   * @returns {void}
   */
  hear(method, params) {
    if (method === 'Page.javascriptDialogOpening') {
      if (params.type === 'beforeunload') return;
      this.pending = {
        type: String(params.type || 'alert'),
        message: String(params.message || ''),
        defaultPrompt: String(params.defaultPrompt || ''),
        url: String(params.url || ''),
      };
      if (this.timer) clearTimeout(this.timer);
      this.timer = setTimeout(() => this.giveUp(), PATIENCE);
      if (!this.page.agentBusy()) this.askReader(this.pending);
      for (const watcher of this.watchers.splice(0)) watcher();
    } else if (method === 'Page.javascriptDialogClosed') {
      const shown = this.pending;
      this.pending = null;
      if (this.timer) clearTimeout(this.timer);
      this.timer = null;
      if (this.withdraw) this.withdraw();
      this.withdraw = null;
      if (!shown || this.answering) return;
      this.remember(
        shown.type === 'prompt'
          ? `A prompt "${shown.message}" closed without an answer from here.`
          : `A ${shown.type} "${shown.message}" closed without an answer from here.`,
      );
    }
  }

  /**
   * Shows a dialog the reader's own click made a page open as a sheet the pane draws over the page, through a `Dialog` report, and gives the page the answer the window sends back with {@link Dialogs.answerFromReader}. The sheet is withdrawn with a `DialogClosed` report when the dialog closes some other way, such as an agent answering it or the timer giving up. Dialogs opened while an agent's tool runs are the agent's to answer.
   * @param {OpenDialog} shown The dialog that opened.
   * @returns {void}
   */
  askReader(shown) {
    const now = Date.now();
    this.boxes = this.boxes.filter((at) => now - at < BOX_WINDOW);
    if (this.boxes.length >= BOX_LIMIT) {
      this.answer(false, '').then(
        () => this.remember(`A ${shown.type} "${shown.message}" was dismissed unseen: the page opened more than ${BOX_LIMIT} dialogs in ${BOX_WINDOW / 1000} seconds.`),
        () => {},
      );
      return;
    }
    this.boxes.push(now);
    const id = nextDialog;
    nextDialog += 1;
    this.shownId = id;
    const page = this.page;
    const kind = shown.type === 'confirm' ? 'Confirm' : shown.type === 'prompt' ? 'Prompt' : 'Alert';
    page.send({
      Chrome: {
        page: page.id,
        report: { Dialog: { id, kind, message: shown.message, default_text: shown.defaultPrompt, host: hostOf(shown.url) } },
      },
    });
    this.withdraw = () => {
      if (this.shownId === id) this.shownId = 0;
      page.send({ Chrome: { page: page.id, report: { DialogClosed: { id } } } });
    };
  }

  /**
   * Takes the reader's answer from the sheet in the pane: it answers the dialog only when that is still the one the sheet showed.
   * @param {number} id The number the sheet's dialog had.
   * @param {boolean} accept Whether OK was pressed.
   * @param {string} text What was typed into a prompt.
   * @returns {void}
   */
  answerFromReader(id, accept, text) {
    if (!this.pending || this.shownId !== id) return;
    this.answer(accept === true, typeof text === 'string' ? text : '').catch(() => {});
  }

  /**
   * Opens a `prompt` of the page, which the preload hands over while the page waits on a synchronous message: it is answered as the other dialogs are, by the reader's sheet, by an agent or by the timer, and `reply` gives the page the text, or `null` when it was cancelled.
   * @param {string} message What it says.
   * @param {string} defaultPrompt The text it starts with.
   * @param {(answer: string | null) => void} reply Gives the page its answer; called once.
   * @param {string} [url] The address of the frame that asked, which may be a frame inside the page; the page's when not given.
   * @returns {void}
   */
  askPrompt(message, defaultPrompt, reply, url) {
    if (this.pending) {
      reply(null);
      return;
    }
    /** @type {OpenDialog} */
    const shown = { type: 'prompt', message: String(message), defaultPrompt: String(defaultPrompt), url: url || this.page.contents.getURL() };
    this.pending = shown;
    this.promptReply = reply;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.giveUp(), PATIENCE);
    if (!this.page.agentBusy()) this.askReader(shown);
    for (const watcher of this.watchers.splice(0)) watcher();
  }

  /**
   * Notes a dialog that closed unseen, for the next tool's answer.
   * @param {string} sentence What happened.
   * @returns {void}
   */
  remember(sentence) {
    this.closed.push(sentence);
    while (this.closed.length > KEPT) this.closed.shift();
  }

  /**
   * Decides what a page that wants to stop a navigation with a `beforeunload` prompt gets: while an agent's tool runs, it may leave, since a prompt nobody can answer would only stall the tool; otherwise Electron's own answer, which keeps the page, stands.
   * @param {import('electron').Event} event The event; its default is to keep the page.
   * @returns {void}
   */
  unloading(event) {
    if (this.busy > 0) {
      event.preventDefault();
      this.remember('The page asked to stay with a beforeunload prompt; the tool left it anyway.');
    } else {
      this.remember('The page asked to stay with a beforeunload prompt, and the browser kept it, so the move did not happen.');
    }
  }

  /**
   * Dismisses the open dialog because nobody answered it in time.
   * @returns {void}
   */
  giveUp() {
    const shown = this.pending;
    if (!shown) return;
    this.answer(false, '').then(
      () => this.remember(`A ${shown.type} "${shown.message}" was dismissed after ${PATIENCE / 1000} seconds without an answer.`),
      () => {},
    );
  }

  /**
   * Answers the open dialog.
   * @param {boolean} accept Whether to accept it (OK) or dismiss it (Cancel).
   * @param {string} text What to type into a prompt.
   * @returns {Promise<void>} Resolves once the page has the answer; rejects when the protocol refuses.
   */
  async answer(accept, text) {
    const reply = this.promptReply;
    if (reply) {
      this.promptReply = null;
      this.pending = null;
      if (this.timer) clearTimeout(this.timer);
      this.timer = null;
      if (this.withdraw) this.withdraw();
      this.withdraw = null;
      reply(accept ? text : null);
      return;
    }
    if (!this.page.cdp) throw new Error('the page has no protocol');
    this.answering = true;
    try {
      /** @type {Json} */
      const params = { accept };
      if (text !== '') params.promptText = text;
      await this.page.cdp.call('Page.handleJavaScriptDialog', params);
      this.pending = null;
      if (this.timer) clearTimeout(this.timer);
      this.timer = null;
      this.page.contents.emit('-cancel-dialogs');
    } finally {
      this.answering = false;
    }
  }

  /**
   * Has `wake` called when the next dialog opens.
   * @param {() => void} wake What to call.
   * @returns {() => void} A function that stops waiting.
   */
  watch(wake) {
    this.watchers.push(wake);
    return () => {
      this.watchers = this.watchers.filter((held) => held !== wake);
    };
  }

  /**
   * What an agent should know about dialogs, to add to a tool's answer: the open one, and those that closed since it was last told.
   * @returns {string} The text, or nothing when there is nothing to say.
   */
  report() {
    const lines = this.closed.splice(0);
    if (this.pending) lines.push(this.describe());
    return lines.length ? `\n\nDialogs:\n${lines.map((line) => `- ${line}`).join('\n')}` : '';
  }

  /**
   * The open dialog, in words.
   * @returns {string} A sentence that says what it is and how to answer it.
   */
  describe() {
    const shown = this.pending;
    if (!shown) return 'No dialog is open.';
    const asked = shown.type === 'prompt' ? ` (it starts with "${shown.defaultPrompt}")` : '';
    return `A ${shown.type} dialog is open on ${shown.url}: "${shown.message}"${asked}. The page is stopped until it is answered; answer it with browser_handle_dialog (it is dismissed by itself after ${PATIENCE / 1000} seconds).`;
  }

  /**
   * Lets go of the timers.
   * @returns {void}
   */
  dispose() {
    if (this.promptReply) this.promptReply(null);
    this.promptReply = null;
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.withdraw) this.withdraw();
    this.withdraw = null;
  }
}

/**
 * Keeps Electron from opening native message boxes for the dialogs of pages: the answer comes from {@link Dialogs}, or, if nobody gives one, from a timer here, so a page never waits on a box nobody sees.
 * A request this does not recognise as a page's dialog goes to Electron's own implementation.
 * @param {import('electron').Dialog} dialog Electron's `dialog` module.
 * @returns {boolean} Whether the module could be changed; when it could not, native boxes open as Electron meant, and the protocol can still answer them.
 */
function holdNativeBoxes(dialog) {
  try {
    const target = /** @type {any} */ (dialog);
    const original = target.showMessageBox.bind(dialog);
    target.showMessageBox = (/** @type {any[]} */ ...args) => {
      const options = args.find((held) => held && typeof held === 'object' && 'message' in held && held.signal);
      if (!options) return original(...args);
      return new Promise((resolve) => {
        const timer = setTimeout(
          () => resolve({ response: typeof options.cancelId === 'number' ? options.cancelId : 0, checkboxChecked: false }),
          PATIENCE + NATIVE_GRACE,
        );
        options.signal.addEventListener(
          'abort',
          () => {
            clearTimeout(timer);
            resolve({ response: 0, checkboxChecked: false });
          },
          { once: true },
        );
      });
    };
    return true;
  } catch (_) {
    return false;
  }
}

exports.Dialogs = Dialogs;
exports.holdNativeBoxes = holdNativeBoxes;
exports.PATIENCE = PATIENCE;
