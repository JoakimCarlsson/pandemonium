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
 * - `prompt` is cancelled at once, without a box: `prompt()` returns `null`.
 * - `beforeunload`: Electron emits `will-prevent-unload` and, when nobody calls `preventDefault`,
 *   cancels the navigation without a word (`did-fail-load` with `ERR_ABORTED`).
 * - The DevTools protocol sees `alert` and `confirm` too: `Page.javascriptDialogOpening` while the
 *   page is stopped, and `Page.handleJavaScriptDialog` answers it.
 *
 * So this file answers through the protocol, keeps the native box from opening, makes every tool
 * return instead of waiting on a stopped page, and gives up on a dialog nobody answers.
 */

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */

/** How long a dialog waits for an agent to answer before it is dismissed, in milliseconds. */
const PATIENCE = 60000;

/** How long past `PATIENCE` the native box is held back before it answers by itself, in milliseconds. */
const NATIVE_GRACE = 5000;

/**
 * Electron's own `dialog.showMessageBox`, kept before {@link holdNativeBoxes} replaces it, so the reader can be asked.
 * @type {((options: import('electron').MessageBoxOptions) => Promise<import('electron').MessageBoxReturnValue>) | null}
 */
let nativeBox = null;

/** How many boxes may be shown to the reader within `BOX_WINDOW` before the page's further dialogs are dismissed unseen. */
const BOX_LIMIT = 3;

/** The span, in milliseconds, in which `BOX_LIMIT` boxes may be shown. */
const BOX_WINDOW = 30000;

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
          ? `A prompt "${shown.message}" was cancelled by the browser at once: Electron does not show prompts, so the page got null.`
          : `A ${shown.type} "${shown.message}" closed without an answer from here.`,
      );
    }
  }

  /**
   * Shows a dialog the reader's own click made a page open, as a native box with no parent window, and gives the page the reader's answer. The box is withdrawn when the dialog closes some other way, such as an agent answering it or the timer giving up. Dialogs opened while an agent's tool runs are the agent's to answer.
   * @param {OpenDialog} shown The dialog that opened.
   * @returns {void}
   */
  askReader(shown) {
    if (!nativeBox || shown.type === 'prompt') return;
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
    const ask = new AbortController();
    this.withdraw = () => ask.abort();
    let host = 'A page';
    try {
      const { protocol, host: name } = new URL(shown.url);
      if ((protocol === 'http:' || protocol === 'https:') && name) host = name;
    } catch (_) {
      /* a page without a normal address keeps the plain title */
    }
    nativeBox({
      type: 'none',
      title: host,
      message: shown.message,
      buttons: shown.type === 'confirm' ? ['OK', 'Cancel'] : ['OK'],
      defaultId: 0,
      cancelId: shown.type === 'confirm' ? 1 : 0,
      noLink: true,
      signal: ask.signal,
    }).then(
      ({ response }) => {
        if (this.pending === shown) this.answer(response === 0, '').catch(() => {});
      },
      () => {},
    );
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
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.withdraw) this.withdraw();
    this.withdraw = null;
  }
}

/**
 * Keeps Electron from opening native message boxes for the dialogs of pages: the answer comes from {@link Dialogs}, or, if nobody gives one, from a timer here, so a page never waits on a box nobody sees.
 * The original is kept for asking the reader about dialogs their own clicks opened. A request this does not recognise as a page's dialog goes to Electron's own implementation.
 * @param {import('electron').Dialog} dialog Electron's `dialog` module.
 * @returns {boolean} Whether the module could be changed; when it could not, native boxes open as Electron meant, and the protocol can still answer them.
 */
function holdNativeBoxes(dialog) {
  try {
    const target = /** @type {any} */ (dialog);
    const original = target.showMessageBox.bind(dialog);
    nativeBox = original;
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
