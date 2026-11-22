'use strict';

/**
 * Tests of how pointer events and typed text reach the frames of a page, run with `node --test test/` in the helper's folder.
 */

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { deliver } = require('../src/input');
const routing = require('../src/routing');

/**
 * A page's protocol that answers as a page with one cross-origin frame whose content box starts at 10, 100 CSS pixels,
 * and records the pointer events and text sent.
 * @returns {{ cdp: any, sent: Array<{ method: string, params: any, session: string | undefined }>, attach: () => void }} The protocol, what was sent and a way to have the frame attach.
 */
function fakeCdp() {
  /** @type {Array<(method: string, params: any, session?: string) => void>} */
  const listeners = [];
  /** @type {Array<{ method: string, params: any, session: string | undefined }>} */
  const sent = [];
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
      if (method === 'DOM.getNodeForLocation') return { backendNodeId: !session && params.y >= 100 ? 7 : 1 };
      if (method === 'DOM.describeNode') return { node: params.backendNodeId === 7 ? { frameId: 'F' } : {} };
      if (method === 'DOM.getBoxModel') return { model: { content: [10, 100, 0, 0, 0, 0, 0, 0] } };
      sent.push({ method, params, session });
      return {};
    },
  };
  /**
   * Has the cross-origin frame attach as a target.
   * @returns {void}
   */
  const attach = () => {
    for (const listener of listeners) listener('Target.attachedToTarget', { sessionId: 'S', targetInfo: { type: 'iframe', targetId: 'F' } });
  };
  return { cdp, sent, attach };
}

test('a pointer over a cross-origin frame goes to its target, in its CSS pixels', async () => {
  const { cdp, sent, attach } = fakeCdp();
  const contents = { getZoomFactor: () => 1 };
  routing.install(/** @type {any} */ (cdp), /** @type {any} */ (contents), () => 2);
  attach();
  deliver(/** @type {any} */ (contents), { PointerMoved: { page: 1, x: 30, y: 120, modifiers: 0, left: false } }, () => {}, 2);
  await routing.of(/** @type {any} */ (contents))?.queue;
  const moves = sent.filter((entry) => entry.method === 'Input.dispatchMouseEvent');
  assert.deepEqual(moves.map((entry) => [entry.session, entry.params.x, entry.params.y]), [['S', 20, 20]]);
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
    [['S', 20, 20], ['S', -1, -1], [undefined, 60, 40]],
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
    [['mousePressed', 'S', 10, 1], ['mouseMoved', 'S', -50, 1], ['mouseReleased', 'S', -50, 0]],
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
    [[undefined, 20, 16], ['S', 50, 20]],
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
