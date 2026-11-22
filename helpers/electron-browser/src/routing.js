'use strict';

/**
 * Pointer events for a page with cross-origin frames. In an offscreen window the engine sends every pointer event to
 * the page's top frame, Electron's own events and the DevTools protocol's alike, so a frame that runs in a process of
 * its own never sees the pointer move and never shows its hover state or tooltips. Each such frame is reached as a
 * target of the page's DevTools protocol: the router finds the frame under the pointer, then sends the event to that
 * frame's target at the point inside it.
 */

/** @typedef {import('./cdp').Cdp} Cdp */
/** @typedef {import('./types').Json} Json */

/** The most frames deep the frame under the pointer is looked for, so a broken tree cannot loop. */
const DEEPEST = 8;

/**
 * Where a frame's viewport starts in the page's window, and the target to reach it by.
 * @typedef {{ session: string | undefined, x: number, y: number }} Frame
 */

/**
 * A pointer event, with its point in the window's pixels.
 * @typedef {{ type: 'mouseMoved' | 'mousePressed' | 'mouseReleased', x: number, y: number, modifiers: number, button: string, buttons: number, clickCount: number }} PointerEvent
 */

/**
 * The routers by the contents of their page.
 * @type {WeakMap<import('electron').WebContents, Router>}
 */
const ROUTERS = new WeakMap();

/**
 * Sends a page's pointer events to the frame under the pointer, through its DevTools protocol.
 */
class Router {
  /**
   * Starts following the cross-origin frames of the page.
   * @param {Cdp} cdp The page's protocol.
   * @param {import('electron').WebContents} contents The page's contents.
   * @param {() => number} ratio The window's pixels per CSS pixel of the page's top frame now: the pane's display
   *   scale times the reader's zoom.
   */
  constructor(cdp, contents, ratio) {
    /** @type {Cdp} The page's protocol. */
    this.cdp = cdp;
    /** @type {import('electron').WebContents} The page's contents. */
    this.contents = contents;
    /** @type {() => number} The window's pixels per CSS pixel of the page's top frame now. */
    this.cssRatio = ratio;
    /** @type {Map<string, string>} The session of each cross-origin frame, by the frame's id. */
    this.sessions = new Map();
    /** @type {Promise<void>} The events under way, so each is sent after the one before it. */
    this.queue = Promise.resolve();
    /** @type {Frame | null} The frame a pressed button holds the pointer in, as the engine captures it. */
    this.captured = null;
    /** @type {Frame | null} The cross-origin frame the pointer was last over. */
    this.hovered = null;
    /** @type {{ event: PointerEvent, done: Promise<void> } | null} The move waiting its turn, replaced by newer moves so moves never pile up. */
    this.waitingMove = null;
    cdp.listen((method, params, session) => this.hear(method, params, session));
    this.follow(undefined);
  }

  /**
   * Asks a target to report the cross-origin frames inside it as targets of their own.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @returns {void}
   */
  follow(session) {
    this.cdp
      .call('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: false, flatten: true }, session)
      .catch(() => {});
  }

  /**
   * Keeps the sessions of the cross-origin frames up to date as they come and go.
   * @param {string} method The event's name.
   * @param {Json} params What it carries.
   * @param {string | undefined} _session The session it came from.
   * @returns {void}
   */
  hear(method, params, _session) {
    if (method === 'Target.attachedToTarget' && params.targetInfo?.type === 'iframe') {
      this.sessions.set(params.targetInfo.targetId, params.sessionId);
      this.follow(params.sessionId);
    } else if (method === 'Target.detachedFromTarget') {
      for (const [frame, session] of this.sessions) if (session === params.sessionId) this.sessions.delete(frame);
      if (this.captured?.session === params.sessionId) this.captured = null;
      if (this.hovered?.session === params.sessionId) this.hovered = null;
    }
  }

  /**
   * Sends a pointer event to the frame under it, after the events sent before it. A move that is still waiting its
   * turn is replaced by a newer one, so a slow page gets the latest point rather than a backlog.
   * @param {PointerEvent} event The event, its point in the window's pixels.
   * @returns {Promise<void>} Settles once it is sent, or failed to be.
   */
  send(event) {
    if (event.type === 'mouseMoved' && this.waitingMove) {
      this.waitingMove.event = event;
      return this.waitingMove.done;
    }
    if (event.type !== 'mouseMoved') {
      this.queue = this.queue.then(() => this.route(event)).catch(() => {});
      return this.queue;
    }
    const waiting = { event, done: Promise.resolve() };
    this.waitingMove = waiting;
    this.queue = this.queue
      .then(() => {
        this.waitingMove = null;
        return this.route(waiting.event);
      })
      .catch(() => {});
    waiting.done = this.queue;
    return this.queue;
  }

  /**
   * The page's own target and the targets of its cross-origin frames, by session.
   * @returns {(string | undefined)[]} Nothing for the page, then each frame's session.
   */
  targets() {
    return [undefined, ...this.sessions.values()];
  }

  /**
   * Sends an HTML drag and drop event to the frame under its point, after the events sent before it.
   * @param {string} type `dragEnter`, `dragOver`, `drop` or `dragCancel`.
   * @param {{ x: number, y: number }} at Where, in the window's pixels.
   * @param {Json} data The drag's data, as the engine gave it when it took the drag over.
   * @returns {Promise<void>} Settles once it is sent, or failed to be.
   */
  dragEvent(type, at, data) {
    this.queue = this.queue
      .then(async () => {
        const frame = await this.frameAt(at.x, at.y);
        await this.cdp.call('Input.dispatchDragEvent', { type, ...this.inside(at, frame), data, modifiers: 0 }, frame.session);
      })
      .catch(() => {});
    return this.queue;
  }

  /**
   * Tells the cross-origin frame the pointer was over that it left, as when the pointer leaves the pane.
   * @returns {Promise<void>} Settles once told.
   */
  leave() {
    this.queue = this.queue
      .then(async () => {
        const left = this.hovered;
        this.hovered = null;
        if (left) await this.dispatch({ type: 'mouseMoved', x: 0, y: 0, modifiers: 0, button: 'none', buttons: 0, clickCount: 0 }, left, -1);
      })
      .catch(() => {});
    return this.queue;
  }

  /**
   * Finds where `event` goes and sends it there: to the frame a pressed button holds, or else to the frame under it,
   * telling the cross-origin frame it was over before that the pointer left.
   * @param {PointerEvent} event The event.
   * @returns {Promise<void>} Settles once it is sent.
   */
  async route(event) {
    const frame = this.captured || (await this.frameAt(event.x, event.y));
    if (this.hovered && this.hovered.session !== frame.session) {
      await this.dispatch({ ...event, type: 'mouseMoved', button: 'none', buttons: 0, clickCount: 0 }, this.hovered, -1);
    }
    this.hovered = frame.session ? frame : null;
    await this.dispatch(event, frame, 0);
    if (event.type === 'mousePressed') this.captured = frame;
    if (event.type === 'mouseReleased' && event.buttons === 0) this.captured = null;
  }

  /**
   * Sends `event` to `frame`, its point moved into the frame's viewport ({@link Router#inside}). A move is not waited
   * on: the protocol keeps a session's messages in order, and waiting for the page to handle each move holds up the
   * events behind it.
   * @param {PointerEvent} event The event.
   * @param {Frame} frame The frame.
   * @param {number} outside 0 to send it where it is, -1 to send it just outside the frame, which tells it the pointer left.
   * @returns {Promise<void>} Settles once the frame has it, or at once for a move.
   */
  async dispatch(event, frame, outside) {
    const at = outside ? { x: -1, y: -1 } : this.inside(event, frame);
    const sent = this.cdp.call('Input.dispatchMouseEvent', { ...event, ...at }, frame.session).catch(() => {});
    if (event.type !== 'mouseMoved') await sent;
  }

  /**
   * A point of the window as `frame`'s target takes it: the page's own target in the window's pixels over the
   * contents' zoom, a cross-origin frame's target in its own CSS pixels.
   * @param {{ x: number, y: number }} at The point, in the window's pixels.
   * @param {Frame} frame The frame.
   * @returns {{ x: number, y: number }} The point in the frame.
   */
  inside(at, frame) {
    const units = frame.session ? this.ratio() : this.contents.getZoomFactor();
    return { x: (at.x - frame.x) / units, y: (at.y - frame.y) / units };
  }

  /**
   * The window's pixels per CSS pixel of the page.
   * @returns {number} The ratio.
   */
  ratio() {
    return this.cssRatio();
  }

  /**
   * The innermost cross-origin frame under a point of the window, or the page itself when there is none.
   * @param {number} x Across, in the window's pixels.
   * @param {number} y Down, in the window's pixels.
   * @returns {Promise<Frame>} The frame and where its viewport starts in the window.
   */
  async frameAt(x, y) {
    /** @type {Frame} */
    let frame = { session: undefined, x: 0, y: 0 };
    if (this.sessions.size === 0) return frame;
    const ratio = this.ratio();
    for (let depth = 0; depth < DEEPEST; depth++) {
      const inner = await this.childAt(frame, (x - frame.x) / ratio, (y - frame.y) / ratio).catch(() => null);
      if (!inner) return frame;
      frame = { session: inner.session, x: frame.x + inner.x * ratio, y: frame.y + inner.y * ratio };
    }
    return frame;
  }

  /**
   * The cross-origin frame whose element is under a point of `frame`.
   * @param {Frame} frame The frame looked in.
   * @param {number} x Across, in CSS pixels of the frame's viewport.
   * @param {number} y Down, in CSS pixels of the frame's viewport.
   * @returns {Promise<{ session: string, x: number, y: number } | null>} The inner frame's session and where its
   *   viewport starts in `frame`, in CSS pixels; nothing when no cross-origin frame is there.
   */
  async childAt(frame, x, y) {
    const hit = await this.cdp.call('DOM.getNodeForLocation', { x: Math.round(x), y: Math.round(y) }, frame.session);
    const node = await this.cdp.call('DOM.describeNode', { backendNodeId: hit.backendNodeId }, frame.session);
    const session = node.node?.frameId ? this.sessions.get(node.node.frameId) : undefined;
    if (!session) return null;
    const box = await this.cdp.call('DOM.getBoxModel', { backendNodeId: hit.backendNodeId }, frame.session);
    return { session, x: box.model.content[0], y: box.model.content[1] };
  }
}

/**
 * Starts routing the pointer events of a page to its cross-origin frames.
 * @param {Cdp} cdp The page's protocol.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {() => number} ratio The window's pixels per CSS pixel of the page's top frame now.
 * @returns {void}
 */
function install(cdp, contents, ratio) {
  ROUTERS.set(contents, new Router(cdp, contents, ratio));
}

/**
 * The router of a page, when one was installed.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {Router | undefined} The router.
 */
function of(contents) {
  return ROUTERS.get(contents);
}

exports.install = install;
exports.of = of;
exports.Router = Router;
