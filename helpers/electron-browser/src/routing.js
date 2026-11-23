'use strict';

/**
 * Pointer events for a page with cross-origin frames. In an offscreen window the engine sends every pointer event to
 * the page's top frame, Electron's own events and the DevTools protocol's alike, so a frame that runs in a process of
 * its own never sees the pointer move and never shows its hover state or tooltips. Each such frame is reached as a
 * target of the page's DevTools protocol: the router finds the frame under the pointer, then sends the event to that
 * frame's target at the point inside it.
 *
 * Finding the frame asks nothing of the page in the common case: the router keeps the box of each cross-origin frame's
 * element in the viewport of the target that holds it, measured once when the frame attaches, and a small script in an
 * isolated world of each target tells it when the layout may have moved (a scroll, a resize, a change to the document,
 * an animation, a load). Such a report marks the boxes of that target stale and has them measured again; an event that
 * arrives while they are stale waits for the new measure, and one whose boxes cannot be measured is found the slow way,
 * by asking the target what is under the point. The script reports the first change at once, before it is painted, so
 * a pointer event the reader sends after seeing the change finds the boxes stale and waits for the new measure.
 *
 * A point inside a cross-origin frame's box is checked exactly before it goes to the frame: the router asks the target
 * that holds the frame, in one round trip to the layout script's isolated world, what `document.elementFromPoint` finds
 * there. An element laid over the frame, such as a menu, a dialog or one a `:hover` rule shows, takes the point, and the
 * event goes to the target that holds the frame. A point outside every frame's box takes no round trip.
 */

/** @typedef {import('./cdp').Cdp} Cdp */
/** @typedef {import('./types').Json} Json */

/** The most frames deep the frame under the pointer is looked for, so a broken tree cannot loop. */
const DEEPEST = 8;

/** The binding the layout script calls, and the isolated world it runs in. */
const BINDING = '__pmRouterLayout';
const WORLD = 'pandemonium-router';

/** The function the layout script defines in its isolated world to say whether a point of the viewport is on a frame. */
const HIT = '__pmRouterHit';

/**
 * The most milliseconds {@link HIT} waits for the move sent to its document before it looks, when the move is not seen,
 * such as when the document drops it.
 */
const MOVE_WAIT = 60;

/**
 * The script each target runs in its isolated world, in every frame it holds. It calls {@link BINDING} at once, then
 * whenever the layout may have moved: at once for the first change, so the router hears of it before the change is
 * painted, then at most once per animation frame (or 100 milliseconds when frames do not run) while changes go on, and
 * on every animation frame while the document runs an animation.
 *
 * In the frame the target shows at its top, each call carries a report, as JSON: `frames`, the border box and the
 * start of the content box of each cross-origin frame's element, in CSS pixels of the viewport. That frame also defines
 * {@link HIT}, which gives the border box of the cross-origin frame's element `document.elementFromPoint` finds at a
 * point of the viewport, or null when what is there is not such an element. Told that a move to the point was sent to
 * the document, it first waits for the document to handle it, at most {@link MOVE_WAIT} milliseconds, so the answer
 * shows what the move's `:hover` rules show. A frame inside the target's top frame sends an empty call, a hint alone.
 */
const WATCH = `(() => {
  if (window.__pmRouterWatching) return;
  window.__pmRouterWatching = true;
  const notify = globalThis[${JSON.stringify(BINDING)}];
  if (typeof notify !== 'function') return;
  let top = false;
  try { top = window.frameElement === null; } catch (_) { top = true; }
  const foreign = (element) => {
    try { return !element.contentDocument; } catch (_) { return true; }
  };
  const framed = (element) => !!element && (element.localName === 'iframe' || element.localName === 'frame') && foreign(element);
  if (top) {
    let last = null;
    addEventListener('mousemove', (event) => { last = [event.clientX, event.clientY]; }, { capture: true, passive: true });
    const at = (point, x, y) => !!point && Math.abs(point[0] - x) < 1 && Math.abs(point[1] - y) < 1;
    const look = (x, y) => {
      const hit = document.elementFromPoint(x, y);
      if (!framed(hit)) return null;
      const box = hit.getBoundingClientRect();
      return [box.left, box.top, box.right, box.bottom];
    };
    globalThis[${JSON.stringify(HIT)}] = (x, y, moved) => {
      if (!moved || at(last, x, y)) return look(x, y);
      return new Promise((done) => {
        const finish = () => { removeEventListener('mousemove', seen, true); clearTimeout(timer); done(look(x, y)); };
        const seen = (event) => { if (at([event.clientX, event.clientY], x, y)) finish(); };
        addEventListener('mousemove', seen, true);
        const timer = setTimeout(finish, ${MOVE_WAIT});
      });
    };
  }
  const report = () => {
    if (!top) return '';
    const frames = [];
    for (const element of document.querySelectorAll('iframe, frame')) {
      if (!foreign(element)) continue;
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      frames.push([box.left, box.top, box.right, box.bottom,
        box.left + element.clientLeft + (parseFloat(style.paddingLeft) || 0),
        box.top + element.clientTop + (parseFloat(style.paddingTop) || 0)]);
    }
    return JSON.stringify({ frames });
  };
  let waiting = null;
  let again = false;
  let animating = false;
  const send = () => { try { notify(report()); } catch (_) {} };
  const settle = (turn) => {
    if (waiting !== turn) return;
    waiting = null;
    if (again) { again = false; tell(); }
  };
  const tell = () => {
    if (waiting) { again = true; return; }
    const turn = {};
    waiting = turn;
    send();
    requestAnimationFrame(() => settle(turn));
    setTimeout(() => settle(turn), 100);
  };
  const follow = () => {
    tell();
    if (animating) return;
    animating = true;
    const step = () => {
      tell();
      if (document.getAnimations().length > 0) requestAnimationFrame(step);
      else animating = false;
    };
    requestAnimationFrame(step);
  };
  const quiet = { capture: true, passive: true };
  for (const type of ['scroll', 'resize', 'load', 'error', 'transitionend', 'animationend', 'toggle', 'mouseover', 'mouseout']) addEventListener(type, tell, quiet);
  for (const type of ['transitionrun', 'animationstart']) addEventListener(type, follow, quiet);
  new MutationObserver(tell).observe(document, { attributes: true, childList: true, subtree: true, characterData: true });
  if (document.fonts) document.fonts.addEventListener('loadingdone', tell);
  if (window.visualViewport) { visualViewport.addEventListener('resize', tell); visualViewport.addEventListener('scroll', tell); }
  tell();
})();`;

/**
 * Where a frame's viewport starts in the page's window, and the target to reach it by.
 * @typedef {{ session: string | undefined, x: number, y: number }} Frame
 */

/**
 * A cross-origin frame as the target that holds it lays it out: the box of its element and where its viewport starts,
 * in CSS pixels of that target's viewport, both null until measured.
 * @typedef {{ session: string, frameId: string, owner: number | undefined, box: { left: number, top: number, right: number, bottom: number } | null, x: number, y: number }} Child
 */

/**
 * The cross-origin frames a target holds, and how far their boxes can be trusted: `watching` once its layout script
 * was asked for, `watched` once it runs, `dirty` when the layout may have moved since the last measure, `running` while one is under way, `broken` when
 * the last one failed.
 * `frames` holds the frames' places the layout script last reported, as JSON, to tell a report that moved no frame from
 * one that did, and `context` the id of the isolated world that sent it, where {@link HIT} is asked, or null when none
 * is known.
 * @typedef {{ children: Child[], watching: boolean, watched: boolean, dirty: boolean, running: Promise<void> | null, broken: boolean, frames: string | null, context: number | null }} Layout
 */

/**
 * A rectangle in CSS pixels of a target's viewport.
 * @typedef {{ left: number, top: number, right: number, bottom: number }} Rect
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
   * @param {() => number} [topUnits] The window's pixels per unit of a point the page's own target takes; the contents'
   *   zoom when not given.
   */
  constructor(cdp, contents, ratio, topUnits = () => contents.getZoomFactor()) {
    /** @type {Cdp} The page's protocol. */
    this.cdp = cdp;
    /** @type {import('electron').WebContents} The page's contents. */
    this.contents = contents;
    /** @type {() => number} The window's pixels per CSS pixel of the page's top frame now. */
    this.cssRatio = ratio;
    /** @type {() => number} The window's pixels per unit of a point the page's own target takes now. */
    this.topUnits = topUnits;
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
    /** @type {string[]} The sessions of the cross-origin frames on the way to where the last pointer event went, outermost first. */
    this.path = [];
    /** @type {Map<string, Layout>} The cross-origin frames each target holds, by its session, the page's as ''. */
    this.layouts = new Map([['', Router.layout()]]);
    cdp.listen((method, params, session) => this.hear(method, params, session));
    this.follow(undefined);
  }

  /**
   * An empty layout, of a target that holds no cross-origin frame yet.
   * @returns {Layout} The layout.
   */
  static layout() {
    return { children: [], watching: false, watched: false, dirty: false, running: null, broken: false, frames: null, context: null };
  }

  /**
   * Has a target run the layout script ({@link WATCH}) in every frame it holds, now and after each navigation, so it
   * tells the router when the boxes of its cross-origin frames may have moved. Only a target that holds such a frame is
   * watched, since the binding needs the target's runtime events on. A target that cannot run it stays unwatched, and
   * its boxes are measured again before each event that needs them.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @returns {Promise<void>} Settles once the script runs, or failed to.
   */
  async watch(session) {
    try {
      await this.cdp.call('Runtime.enable', {}, session);
      await this.cdp.call('Runtime.addBinding', { name: BINDING, executionContextName: WORLD }, session);
      await this.cdp.call('Page.addScriptToEvaluateOnNewDocument', { source: WATCH, worldName: WORLD, runImmediately: true }, session);
    } catch (_) {
      return;
    }
    const layout = this.layouts.get(session || '');
    if (!layout) return;
    layout.watched = true;
    void this.remeasure(session);
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
   * Keeps the sessions of the cross-origin frames and the layouts that hold them up to date as frames come and go,
   * and marks a target's boxes stale when it says its layout may have moved.
   * @param {string} method The event's name.
   * @param {Json} params What it carries.
   * @param {string | undefined} session The session it came from: the target that holds the frame, for a frame's arrival.
   * @returns {void}
   */
  hear(method, params, session) {
    if (method === 'Target.attachedToTarget' && params.targetInfo?.type === 'iframe') {
      this.attached(params.targetInfo.targetId, params.sessionId, session);
    } else if (method === 'Target.detachedFromTarget') {
      this.detached(params.sessionId);
    } else if (method === 'Runtime.bindingCalled' && params.name === BINDING) {
      this.reported(session, params.payload, params.executionContextId);
    } else if (method === 'Runtime.executionContextDestroyed' || method === 'Runtime.executionContextsCleared') {
      this.forget(session, params.executionContextId);
    } else if (method === 'Page.frameResized') {
      void this.remeasure(session);
    }
  }

  /**
   * Takes in what a target's layout script reported: the isolated world that sent it becomes the one asked what is
   * under a point, and the boxes are measured again unless the report shows every frame where the last one did.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @param {unknown} payload The report, as JSON, or an empty hint from a frame below the target's top frame.
   * @param {unknown} context The id of the isolated world that sent it.
   * @returns {void}
   */
  reported(session, payload, context) {
    const layout = this.layouts.get(session || '');
    if (!layout) return;
    const frames = Router.parseReport(payload);
    if (frames === null) {
      void this.remeasure(session);
      return;
    }
    if (typeof context === 'number') layout.context = context;
    if (frames === layout.frames && Router.trusted(layout)) return;
    layout.frames = frames;
    void this.remeasure(session);
  }

  /**
   * Forgets the isolated world of a target's layout script when it goes away, with its document or with every context
   * of the target.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @param {unknown} context The id of the context that went away, or nothing when every one did.
   * @returns {void}
   */
  forget(session, context) {
    const layout = this.layouts.get(session || '');
    if (layout && (context === undefined || context === layout.context)) layout.context = null;
  }

  /**
   * Reads a layout report.
   * @param {unknown} payload The report as the layout script sent it.
   * @returns {string | null} The frames' places as JSON; nothing for an empty hint or a report that does not parse.
   */
  static parseReport(payload) {
    if (typeof payload !== 'string' || payload === '') return null;
    try {
      const parsed = JSON.parse(payload);
      return Array.isArray(parsed?.frames) ? JSON.stringify(parsed.frames) : null;
    } catch (_) {
      return null;
    }
  }

  /**
   * Starts following a cross-origin frame that attached: its session, its own layout, its place in the layout of the
   * target that holds it, which starts watching its layout with its first such frame, and its own frames. The layout comes first, since asking a target for its frames can report
   * them before the call returns.
   * @param {string} frameId The frame's id, which is its target's.
   * @param {string} session The frame's session.
   * @param {string | undefined} parent The session of the target that holds it, or nothing for the page.
   * @returns {void}
   */
  attached(frameId, session, parent) {
    this.sessions.set(frameId, session);
    this.layouts.set(session, Router.layout());
    const holder = this.layouts.get(parent || '');
    if (holder) {
      holder.children = holder.children.filter((child) => child.frameId !== frameId);
      holder.children.push({ session, frameId, owner: undefined, box: null, x: 0, y: 0 });
      void this.remeasure(parent);
      if (!holder.watching) {
        holder.watching = true;
        void this.watch(parent);
      }
    }
    this.follow(session);
  }

  /**
   * Stops following a cross-origin frame that went away.
   * @param {string} session The frame's session.
   * @returns {void}
   */
  detached(session) {
    for (const [frame, held] of this.sessions) if (held === session) this.sessions.delete(frame);
    for (const layout of this.layouts.values()) layout.children = layout.children.filter((child) => child.session !== session);
    this.layouts.delete(session);
    if (this.captured?.session === session) this.captured = null;
    if (this.hovered?.session === session) this.hovered = null;
  }

  /**
   * Marks a target's boxes stale and has them measured again, unless a measure is under way, which then measures once
   * more when it ends.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @returns {Promise<void>} Settles once the boxes are measured and nothing marked them stale since.
   */
  remeasure(session) {
    const layout = this.layouts.get(session || '');
    if (!layout) return Promise.resolve();
    layout.dirty = true;
    if (!layout.running) layout.running = this.measureUntilStill(session, layout);
    return layout.running;
  }

  /**
   * Measures a target's boxes until nothing marked them stale during the last measure.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @param {Layout} layout Its layout.
   * @returns {Promise<void>} Settles once done.
   */
  async measureUntilStill(session, layout) {
    try {
      await Promise.resolve();
      while (layout.dirty) {
        layout.dirty = false;
        layout.broken = !(await this.measure(session, layout));
      }
    } finally {
      layout.running = null;
    }
  }

  /**
   * Measures the box of each cross-origin frame a target holds, finding each frame's element the first time.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @param {Layout} layout Its layout.
   * @returns {Promise<boolean>} Whether every box was measured; a frame whose box failed has it unset.
   */
  async measure(session, layout) {
    const measured = await Promise.all(
      layout.children.map(async (child) => {
        try {
          if (child.owner === undefined) {
            child.owner = (await this.cdp.call('DOM.getFrameOwner', { frameId: child.frameId }, session)).backendNodeId;
          }
          const box = await this.cdp.call('DOM.getBoxModel', { backendNodeId: child.owner }, session);
          const placed = Router.placed(box.model);
          if (!placed) throw new Error('the frame is not an upright box');
          Object.assign(child, placed);
          return true;
        } catch (_) {
          child.owner = undefined;
          child.box = null;
          return false;
        }
      }),
    );
    return measured.every(Boolean);
  }

  /**
   * Where a frame's element lies, from its box model: the rectangle of its border box and where its content starts.
   * @param {Json} model The box model, quads in CSS pixels of its target's viewport.
   * @returns {{ box: { left: number, top: number, right: number, bottom: number }, x: number, y: number } | null} The
   *   place; nothing when the box is turned, skewed or empty, which a rectangle cannot hold.
   */
  static placed(model) {
    const border = model?.border;
    const content = model?.content;
    if (!Array.isArray(border) || border.length !== 8 || !Array.isArray(content) || content.length < 2) return null;
    const [x1, y1, x2, y2, x3, y3, x4, y4] = border;
    if (y1 !== y2 || x2 !== x3 || y3 !== y4 || x4 !== x1 || x2 <= x1 || y3 <= y1) return null;
    return { box: { left: x1, top: y1, right: x2, bottom: y3 }, x: content[0], y: content[1] };
  }

  /**
   * Whether a layout's boxes can be used as they are, with no measure first.
   * @param {Layout} layout The layout.
   * @returns {boolean} True when it holds no frame, or its target watches its layout and nothing moved it since the last
   *   measure, which succeeded.
   */
  static trusted(layout) {
    return layout.children.length === 0 || (layout.watched && !layout.dirty && !layout.running && !layout.broken);
  }

  /**
   * Whether a rectangle holds a point.
   * @param {Rect} rect The rectangle.
   * @param {number} x Across.
   * @param {number} y Down.
   * @returns {boolean} Whether the point lies inside, its left and top edges included.
   */
  static holds(rect, x, y) {
    return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
  }

  /**
   * The cross-origin frames whose measured boxes hold a point of a target, the first found first.
   * @param {Layout} layout The target's layout.
   * @param {number} x Across, in CSS pixels of the target's viewport.
   * @param {number} y Down, in CSS pixels of the target's viewport.
   * @returns {Child[]} The frames; none when the point is on no frame's box.
   */
  static boxedAt(layout, x, y) {
    return layout.children.filter((each) => each.box && Router.holds(each.box, x, y));
  }

  /**
   * The cross-origin frame that takes the pointer at a point of a target whose box holds it, decided exactly by asking
   * the target's layout script what `document.elementFromPoint` finds there, in one round trip. When the pointer comes
   * into the frames' boxes from elsewhere, the move goes to the target as well, sent beside the question, and the
   * script answers once the target handled it, so an element a `:hover` rule shows over the frame on that move counts.
   * @param {string | undefined} session The target's session, or nothing for the page.
   * @param {Layout} layout The target's layout, whose boxes hold the point.
   * @param {Child[]} boxed The frames whose boxes hold the point ({@link Router.boxedAt}).
   * @param {number} x Across, in CSS pixels of the target's viewport.
   * @param {number} y Down, in CSS pixels of the target's viewport.
   * @param {Json | null} entry The move to send the target first, its point as the target takes it, or nothing.
   * @returns {Promise<Child | null | undefined>} The frame found there, the one whose box matches the element's when
   *   several hold the point; null when something else of the target is there; nothing when the target could not be
   *   asked.
   */
  async onFrame(session, layout, boxed, x, y, entry) {
    if (layout.context === null) return undefined;
    if (entry) this.cdp.call('Input.dispatchMouseEvent', entry, session).catch(() => {});
    try {
      const answer = await this.cdp.call(
        'Runtime.evaluate',
        { expression: `globalThis[${JSON.stringify(HIT)}](${x}, ${y}, ${!!entry})`, contextId: layout.context, returnByValue: true, awaitPromise: true },
        session,
      );
      if (answer.exceptionDetails) return undefined;
      const box = answer.result?.value;
      if (!Array.isArray(box) || box.length !== 4) return null;
      const near = (/** @type {number} */ a, /** @type {number} */ b) => Math.abs(a - b) < 1;
      return boxed.find((each) => each.box && near(each.box.left, box[0]) && near(each.box.top, box[1]) && near(each.box.right, box[2]) && near(each.box.bottom, box[3])) || boxed[0];
    } catch (_) {
      return undefined;
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
        const frame = await this.locate(at.x, at.y);
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
   * Whether the page has cross-origin frames now, whose keys the engine's own key events never reach.
   * @returns {boolean} Whether any is followed.
   */
  hasFrames() {
    return this.sessions.size > 0;
  }

  /**
   * Sends a key event through the page's own target after the pointer events sent before it, so a key pressed right
   * after a click reaches the field the click focused. The protocol hands it to the frame that holds the keyboard, even
   * a cross-origin frame in a process of its own.
   * @param {Json} event The parameters of `Input.dispatchKeyEvent`.
   * @returns {Promise<void>} Settles once it is sent, or failed to be.
   */
  key(event) {
    this.queue = this.queue.then(() => this.cdp.call('Input.dispatchKeyEvent', event)).then(
      () => {},
      () => {},
    );
    return this.queue;
  }

  /**
   * Finds where `event` goes and sends it there: to the frame a pressed button holds, or else to the frame under it,
   * telling the cross-origin frame it was over before that the pointer left.
   * @param {PointerEvent} event The event.
   * @returns {Promise<void>} Settles once it is sent.
   */
  async route(event) {
    const known = this.captured ? null : this.known(event.x, event.y);
    if (known) this.path = [];
    const frame = this.captured || known || (await this.locate(event.x, event.y, event));
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
   * A point of the window as `frame`'s target takes it: the page's own target in the window's pixels over its units
   * (the contents' zoom for a page with a window of its own), a cross-origin frame's target in its own CSS pixels.
   * @param {{ x: number, y: number }} at The point, in the window's pixels.
   * @param {Frame} frame The frame.
   * @returns {{ x: number, y: number }} The point in the frame.
   */
  inside(at, frame) {
    const units = frame.session ? this.ratio() : this.topUnits();
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
   * The page itself when a point of the window lies on no cross-origin frame's box, from the boxes measured already,
   * with no round trip to the page.
   * @param {number} x Across, in the window's pixels.
   * @param {number} y Down, in the window's pixels.
   * @returns {Frame | null} The page; nothing when the point is on a frame's box, which needs asking the page
   *   ({@link Router#locate}), or the page's boxes may have moved since they were measured.
   */
  known(x, y) {
    const layout = this.layouts.get('');
    const ratio = this.ratio();
    if (!layout || !Router.trusted(layout) || Router.boxedAt(layout, x / ratio, y / ratio).length > 0) return null;
    return { session: undefined, x: 0, y: 0 };
  }

  /**
   * The innermost cross-origin frame under a point of the window, measuring the boxes on the way that may have moved
   * first, in one round trip a frame whatever measures run beside it, then asking each target whose frame's box holds
   * the point whether the frame or something laid over it is there ({@link Router#onFrame}), and asking the page what
   * is under the point ({@link Router#frameAt}) when a box cannot be measured or the target cannot be asked. For a
   * pointer event, a target the pointer was not inside one of those frames of before gets the move too, and the frames
   * on the way are kept ({@link Router#path}) to tell the next event.
   * @param {number} x Across, in the window's pixels.
   * @param {number} y Down, in the window's pixels.
   * @param {PointerEvent} [event] The pointer event looked for, or nothing for a drag.
   * @returns {Promise<Frame>} The frame and where its viewport starts in the window.
   */
  async locate(x, y, event) {
    /** @type {Frame} */
    let frame = { session: undefined, x: 0, y: 0 };
    /** @type {string[]} */
    const path = [];
    const ratio = this.ratio();
    for (let depth = 0; depth < DEEPEST; depth++) {
      const layout = this.layouts.get(frame.session || '');
      if (!layout) break;
      if (!Router.trusted(layout)) layout.broken = !(await this.measure(frame.session, layout));
      if (layout.broken) return this.frameAt(x, y);
      const across = (x - frame.x) / ratio;
      const down = (y - frame.y) / ratio;
      const boxed = Router.boxedAt(layout, across, down);
      if (boxed.length === 0) break;
      const entering = event && !boxed.some((each) => this.path.includes(each.session));
      const entry = event && entering
        ? { ...event, type: 'mouseMoved', button: 'none', buttons: event.type === 'mousePressed' ? 0 : event.buttons, clickCount: 0, ...this.inside({ x, y }, frame) }
        : null;
      const inner = await this.onFrame(frame.session, layout, boxed, across, down, entry);
      if (inner === undefined) return this.frameAt(x, y);
      if (inner === null) break;
      frame = { session: inner.session, x: frame.x + inner.x * ratio, y: frame.y + inner.y * ratio };
      path.push(inner.session);
    }
    if (event) this.path = path;
    return frame;
  }

  /**
   * The innermost cross-origin frame under a point of the window, or the page itself when there is none, by asking
   * each target on the way what is under the point: the slow way, three round trips a frame.
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
 * @param {() => number} [topUnits] The window's pixels per unit of a point the page's own target takes; the contents'
 *   zoom when not given.
 * @returns {void}
 */
function install(cdp, contents, ratio, topUnits) {
  ROUTERS.set(contents, new Router(cdp, contents, ratio, topUnits));
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
