'use strict';

/**
 * Tests of how pointer events and typed text reach the frames of a page, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { deliver } = require('../src/input');
const routing = require('../src/routing');

/**
 * A page's protocol that answers as a page with one cross-origin frame whose content box starts at 10, 100 CSS pixels
 * and is 400 by 300, and records the pointer events and text sent and the questions asked about the page. Asked in its
 * layout script's world what is under a point, it names the frame's box unless a rectangle laid over the frame holds it.
 * @param {{ ownerFails?: boolean }} [options] `ownerFails` to have finding the frame's element fail, so its box cannot be measured.
 * @returns {{ cdp: any, sent: Array<{ method: string, params: any, session: string | undefined }>, asked: string[], attach: () => void, emit: (method: string, params: any, session?: string) => void, place: (top: number) => void, cover: (rect: number[] | null) => void }} The protocol, what was sent, the questions asked, a way to have the frame attach and the page's layout script report, a way to send an event, a way to move the frame's top and a way to lay a rectangle over the frame or take it away.
 */
function fakeCdp(options = {}) {
  /** @type {Array<(method: string, params: any, session?: string) => void>} */
  const listeners = [];
  /** @type {Array<{ method: string, params: any, session: string | undefined }>} */
  const sent = [];
  /** @type {string[]} */
  const asked = [];
  let top = 100;
  /** @type {number[] | null} */
  let over = null;
  const cdp = {
    /**
     * Records a listener.
     * @param {(method: string, params: any, session?: string) => void} listener The listener.
     * @returns {void}
     */
    listen(listener) {
      listeners.push(listener);
    },
    /**
     * Answers a command as the page would.
     * @param {string} method The command.
     * @param {any} params What it takes.
     * @param {string} [session] The frame target, if any.
     * @returns {Promise<any>} The answer.
     */
    async call(method, params, session) {
      if (method.startsWith('DOM.') || method === 'Runtime.evaluate') asked.push(method);
      if (method === 'Runtime.evaluate') {
        const [x, y] = (params.expression.match(/\(([-\d.]+), ([-\d.]+),/) || []).slice(1).map(Number);
        const covered = over && x >= over[0] && x < over[2] && y >= over[1] && y < over[3];
        const framed = x >= 10 && x < 410 && y >= top && y < top + 300;
        return { result: { value: framed && !covered ? [10, top, 410, top + 300] : null } };
      }
      if (method === 'DOM.getNodeForLocation') return { backendNodeId: !session && params.y >= top ? 7 : 1 };
      if (method === 'DOM.describeNode') return { node: params.backendNodeId === 7 ? { frameId: 'F' } : {} };
      if (method === 'DOM.getFrameOwner') {
        if (options.ownerFails) throw new Error('no such frame');
        return { backendNodeId: 7 };
      }
      if (method === 'DOM.getBoxModel') {
        return { model: { content: [10, top, 410, top, 410, top + 300, 10, top + 300], border: [10, top, 410, top, 410, top + 300, 10, top + 300] } };
      }
      sent.push({ method, params, session });
      return {};
    },
  };
  /**
   * Sends an event to every listener.
   * @param {string} method The event.
   * @param {any} params What it carries.
   * @param {string} [session] The target it came from.
   * @returns {void}
   */
  const emit = (method, params, session) => {
    for (const listener of listeners) listener(method, params, session);
  };
  /**
   * Has the cross-origin frame attach as a target.
   * @returns {void}
   */
  /**
   * Has the cross-origin frame attach as a target, and the page's layout script report it from its world, number 1.
   * @returns {void}
   */
  const attach = () => {
    emit('Target.attachedToTarget', { sessionId: 'S', targetInfo: { type: 'iframe', targetId: 'F' } });
    emit('Runtime.bindingCalled', { name: '__pmRouterLayout', executionContextId: 1, payload: JSON.stringify({ frames: [[10, top, 410, top + 300, 10, top]] }) }, undefined);
  };
  /**
   * Moves the frame's top, as a scroll or a script would.
   * @param {number} at The new top, in CSS pixels of the page's viewport.
   * @returns {void}
   */
  const place = (at) => {
    top = at;
  };
  /**
   * Lays a rectangle of the page over the frame, or takes it away.
   * @param {number[] | null} rect Its left, top, right and bottom in CSS pixels of the page's viewport, or nothing.
   * @returns {void}
   */
  const cover = (rect) => {
    over = rect;
  };
  return { cdp, sent, asked, attach, emit, place, cover };
}

/**
 * Lets the promises and callbacks under way settle.
 * @returns {Promise<void>} Settles once they did.
 */
async function settle() {
  for (let turn = 0; turn < 5; turn++) await new Promise((done) => setImmediate(done));
}

/**
 * Where the pointer events sent went: each one's target and point.
 * @param {Array<{ method: string, params: any, session: string | undefined }>} sent What was sent.
 * @returns {Array<[string | undefined, number, number]>} Each pointer event's session, across and down.
 */
function landed(sent) {
  return sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent').map((entry) => [entry.session, entry.params.x, entry.params.y]);
}

test('a pointer over a cross-origin frame goes to its target, in its CSS pixels', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 2);
  attach();
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 30, y: 120, modifiers: 0, left: false } }, () => {}, 2);
  await routing.of(/** @type {any} */ (contents))?.queue;
  const moves = sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent');
  assert.deepEqual(moves.map((entry) => [entry.session, entry.params.x, entry.params.y]), [[undefined, 60, 240], ['S', 20, 20]]);
});

test('a pointer over the top frame goes to the page in the window pixels, and leaving a frame tells it', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 2);
  attach();
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 30, y: 120, modifiers: 0, left: false } }, () => {}, 2);
  await routing.of(/** @type {any} */ (contents))?.queue;
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 30, y: 20, modifiers: 0, left: false } }, () => {}, 2);
  await routing.of(/** @type {any} */ (contents))?.queue;
  const moves = sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent');
  assert.deepEqual(
    moves.map((entry) => [entry.session, entry.params.x, entry.params.y]),
    [[undefined, 60, 240], ['S', 20, 20], ['S', -1, -1], [undefined, 60, 40]],
  );
});

test('a pressed button keeps the pointer in the frame it went down in', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  deliver(/** @type {any} */ (contents), { PointerButton: { page: 1, x: 20, y: 110, button: 'Left', pressed: true, clicks: 1, modifiers: 0 } });
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 20, y: 50, modifiers: 16, left: false } });
  deliver(/** @type {any} */ (contents), { PointerButton: { page: 1, x: 20, y: 50, button: 'Left', pressed: false, clicks: 1, modifiers: 16 } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  const events = sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent');
  assert.deepEqual(
    events.map((entry) => [entry.params.type, entry.session, entry.params.y, entry.params.buttons]),
    [['mouseMoved', undefined, 110, 0], ['mousePressed', 'S', 10, 1], ['mouseMoved', 'S', -50, 1], ['mouseReleased', 'S', -50, 0]],
  );
});

test('under a zoom the page takes points in the window pixels over the zoom, and a frame in its CSS pixels', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 2.5 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 2.5);
  attach();
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 25, y: 20, modifiers: 0, left: false } }, () => {}, 2);
  await routing.of(/** @type {any} */ (contents))?.queue;
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 75, y: 150, modifiers: 0, left: false } }, () => {}, 2);
  await routing.of(/** @type {any} */ (contents))?.queue;
  const moves = sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent');
  assert.deepEqual(
    moves.map((entry) => [entry.session, entry.params.x, entry.params.y]),
    [[undefined, 20, 16], [undefined, 60, 120], ['S', 50, 20]],
  );
});

test('moves that pile up while the page is busy are sent as the newest one only', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  for (let x = 1; x <= 50; x++) {
    deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x, y: 20, modifiers: 0, left: false } });
  }
  deliver(/** @type {any} */ (contents), { PointerButton: { page: 1, x: 50, y: 20, button: 'Left', pressed: true, clicks: 1, modifiers: 0 } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  const events = sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent');
  assert.deepEqual(
    events.map((entry) => [entry.params.type, entry.params.x]),
    [['mouseMoved', 50], ['mousePressed', 50]],
  );
});

test('the router names the page and each cross-origin frame, and sends a drag event to the frame under it', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 2);
  attach();
  const router = /** @type {import('../src/routing').Router} */ (routing.of(/** @type {any} */ (contents)));
  assert.deepEqual(router.targets(), [undefined, 'S']);
  await router.dragEvent('drop', { x: 60, y: 240 }, { items: [] });
  const drops = sent.filter((entry) => entry.method === 'Input.dispatchDragEvent');
  assert.deepEqual(
    drops.map((entry) => [entry.session, entry.params.type, entry.params.x, entry.params.y]),
    [['S', 'drop', 20, 20]],
  );
});

test('a move off every frame box asks the page nothing, and one on a frame box asks it once', async () => {
  const { cdp, sent, asked, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 2);
  attach();
  await settle();
  asked.length = 0;
  for (const y of [120, 20, 240]) {
    deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 30, y, modifiers: 0, left: false } }, () => {}, 2);
    await routing.of(/** @type {any} */ (contents))?.queue;
  }
  assert.deepEqual(asked, ['Runtime.evaluate', 'Runtime.evaluate']);
  assert.deepEqual(landed(sent), [[undefined, 60, 240], ['S', 20, 20], ['S', -1, -1], [undefined, 60, 40], [undefined, 60, 480], ['S', 20, 140]]);
});

test('a layout hint has the boxes measured again before the next event uses them', async () => {
  const { cdp, sent, attach, emit, place } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  await settle();
  place(200);
  emit('Runtime.bindingCalled', { name: '__pmRouterLayout', payload: '' }, undefined);
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 20, y: 150, modifiers: 0, left: false } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 20, y: 210, modifiers: 0, left: false } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  assert.deepEqual(landed(sent), [[undefined, 20, 150], [undefined, 20, 210], ['S', 10, 10]]);
});

test('an element its parent lays over part of a frame takes the pointer there, however small, and the rest still reaches the frame', async () => {
  const { cdp, sent, asked, attach, cover } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  await settle();
  asked.length = 0;
  cover([20, 110, 22, 112]);
  for (const [x, y] of [[21, 111], [23, 111], [200, 200]]) {
    deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x, y, modifiers: 0, left: false } });
    await routing.of(/** @type {any} */ (contents))?.queue;
  }
  cover(null);
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 21, y: 111, modifiers: 0, left: false } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  assert.ok(asked.every((method) => method === 'Runtime.evaluate'));
  assert.deepEqual(landed(sent), [[undefined, 21, 111], [undefined, 21, 111], [undefined, 23, 111], ['S', 13, 11], ['S', 190, 100], ['S', 11, 11]]);
});

test('a report that moves a frame has its box measured again before the next event', async () => {
  const { cdp, sent, asked, attach, emit, place } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  emit('Runtime.bindingCalled', { name: '__pmRouterLayout', payload: JSON.stringify({ frames: [[10, 100, 410, 400, 10, 100]], covers: [] }) }, undefined);
  await settle();
  asked.length = 0;
  place(200);
  emit('Runtime.bindingCalled', { name: '__pmRouterLayout', payload: JSON.stringify({ frames: [[10, 200, 410, 500, 10, 200]], covers: [] }) }, undefined);
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 20, y: 150, modifiers: 0, left: false } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  assert.ok(asked.includes('DOM.getBoxModel'));
  assert.deepEqual(landed(sent), [[undefined, 20, 150]]);
});

test('a frame whose box cannot be measured is found by asking the page what is under the pointer', async () => {
  const { cdp, sent, asked, attach } = fakeCdp({ ownerFails: true });
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  await settle();
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 20, y: 110, modifiers: 0, left: false } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  assert.ok(asked.includes('DOM.getNodeForLocation'));
  assert.deepEqual(landed(sent), [['S', 10, 10]]);
});

test('a frame that leaves takes its box with it', async () => {
  const { cdp, sent, attach, emit } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 1);
  attach();
  await settle();
  emit('Target.detachedFromTarget', { sessionId: 'S' }, undefined);
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 20, y: 110, modifiers: 0, left: false } });
  await routing.of(/** @type {any} */ (contents))?.queue;
  assert.deepEqual(landed(sent), [[undefined, 20, 110]]);
});

test('committed and pasted text go through the protocol, which reaches a cross-origin frame without a crash', () => {
  /** @type {Array<[string, any]>} */
  const commands = [];
  let inserted = '';
  const contents = {
    debugger: {
      isAttached: () => true,
      /**
       * Records a command.
       * @param {string} method The command.
       * @param {any} params What it takes.
       * @returns {Promise<void>} Settles at once.
       */
      sendCommand: (method, params) => {
        commands.push([method, params]);
        return Promise.resolve();
      },
    },
    /**
     * Records text inserted the way that crashes a cross-origin frame.
     * @param {string} text The text.
     * @returns {void}
     */
    insertText: (text) => {
      inserted += text;
    },
  };
  deliver(/** @type {any} */ (contents), { CommitText: { page: 1, text: 'abc' } });
  deliver(/** @type {any} */ (contents), { PasteText: { page: 1, text: 'PP' } });
  assert.deepEqual(commands, [['Input.insertText', { text: 'abc' }], ['Input.insertText', { text: 'PP' }]]);
  assert.equal(inserted, '');
});
