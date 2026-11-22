'use strict';

/**
 * A page's DevTools protocol: commands that answer, and the events the page sends.
 */

/** @typedef {import('./types').Json} Json */

/**
 * Called with the name and the parameters of an event the page sent.
 * @callback EventListener
 * @param {string} method The event's name, such as `Runtime.consoleAPICalled`.
 * @param {Json} params What the event carries.
 * @param {string} [session] The session of the frame target that sent it, or nothing for the page itself.
 * @returns {void}
 */

/**
 * The protocol of one page, over Electron's debugger.
 */
class Cdp {
  /**
   * Attaches to the page shown by `contents`.
   * @param {import('electron').WebContents} contents The page's contents.
   */
  constructor(contents) {
    /** @type {import('electron').Debugger} The page's debugger. */
    this.debugger = contents.debugger;
    /** @type {EventListener[]} Who hears the events. */
    this.listeners = [];
    this.debugger.attach('1.3');
    this.debugger.on('message', (_event, method, params, session) => {
      for (const listener of this.listeners) listener(method, params, session || undefined);
    });
  }

  /**
   * Sends a command to the page.
   * @param {string} method The command, such as `Page.navigate`.
   * @param {Json} [params] What it takes.
   * @param {string} [session] The session of a frame target to send it to, such as a cross-origin frame; the page itself when not given.
   * @returns {Promise<Json>} The result, or a rejection with the protocol's error.
   */
  call(method, params = {}, session = undefined) {
    return session ? this.debugger.sendCommand(method, params, session) : this.debugger.sendCommand(method, params);
  }

  /**
   * Has `listener` called with every event the page sends.
   * @param {EventListener} listener Who wants to hear them.
   * @returns {void}
   */
  listen(listener) {
    this.listeners.push(listener);
  }

  /**
   * Stops calling `listener`.
   * @param {EventListener} listener What was given to `listen`.
   * @returns {void}
   */
  unlisten(listener) {
    this.listeners = this.listeners.filter((held) => held !== listener);
  }

  /**
   * Lets go of the page.
   * @returns {void}
   */
  detach() {
    try {
      this.debugger.detach();
    } catch (_) {
      /* already detached */
    }
  }
}

exports.Cdp = Cdp;
