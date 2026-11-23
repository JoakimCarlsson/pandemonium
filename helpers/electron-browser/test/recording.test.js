'use strict';

/**
 * Tests of recordings of what agents do with the browser, run with `node --test test/` in the helper's folder. Scratch
 * files go in the repository's ignored `build/` folder.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const recording = require('../src/recording');

/** The ignored folder the tests write their files under. */
const SCRATCH = path.join(__dirname, '..', '..', '..', 'build', 'helper-tests');

/** A one-pixel PNG, as base64. */
const PIXEL = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';

/**
 * A stand-in for a page whose screenshots are one pixel and whose scripts in the tools' world answer as the masking of
 * secret fields expects; `calls` gets the name of every protocol method called, and `readable` false makes the page
 * unreadable, so that its secret fields cannot be blurred.
 * @param {string[]} [calls] Where the methods called go.
 * @param {boolean} [readable] Whether scripts can run in the page.
 * @returns {any} The page.
 */
function page(calls = [], readable = true) {
  return {
    id: 7,
    dialogs: null,
    cdp: {
      call: async (/** @type {string} */ method) => {
        calls.push(method);
        if (method === 'Page.getFrameTree') return readable ? { frameTree: { frame: { id: 'main' } } } : null;
        if (method === 'Page.createIsolatedWorld') return { executionContextId: 3 };
        if (method === 'Runtime.evaluate') return { result: { value: '1' } };
        return { data: PIXEL };
      },
    },
    contents: { isDestroyed: () => false, getURL: () => 'http://localhost:5173/' },
  };
}

test('a recording keeps each tool call with a picture and writes a summary', async () => {
  fs.mkdirSync(SCRATCH, { recursive: true });
  const folder = fs.mkdtempSync(path.join(SCRATCH, 'recording-'));
  assert.equal(await recording.afterTool(page(), 'browser_click', '{}', { ok: true, text: 'Clicked.', image: null }), null);
  assert.ok(recording.recordTool({ action: 'start', folder }).ok);
  assert.ok(!recording.recordTool({ action: 'start', folder }).ok);
  const first = await recording.afterTool(page(), 'browser_click', '{"ref":3}', { ok: true, text: 'Clicked.', image: null });
  const second = await recording.afterTool(page(), 'browser_type', '{"text":"hi"}', { ok: false, text: 'no such element', image: null });
  assert.equal(first && first.index, 1);
  assert.equal(second && second.index, 2);
  assert.ok(first && fs.existsSync(first.picture));
  assert.equal(await recording.afterTool(page(), 'browser_record', '{}', { ok: true, text: '', image: null }), null);
  const stopped = recording.recordTool({ action: 'stop' });
  assert.ok(stopped.ok);
  assert.match(stopped.text, /1\. .*`browser_click` \{"ref":3\} — ok: Clicked\./);
  assert.match(stopped.text, /2\. .*`browser_type` .* — failed: no such element/);
  const summary = fs.readFileSync(path.join(folder, 'summary.md'), 'utf8');
  assert.match(summary, /2 steps\./);
  assert.equal(JSON.parse(fs.readFileSync(path.join(folder, 'steps.json'), 'utf8')).length, 2);
  assert.ok(!recording.recordTool({ action: 'stop' }).ok);
  fs.rmSync(folder, { recursive: true, force: true });
});

test('a recording refuses a folder that is not absolute', () => {
  assert.ok(!recording.recordTool({ action: 'start', folder: 'relative/place' }).ok);
  assert.ok(!recording.recordTool({ action: 'pause' }).ok);
});

test('each session has its own recording, in a folder of its own', async () => {
  fs.mkdirSync(SCRATCH, { recursive: true });
  const base = fs.mkdtempSync(path.join(SCRATCH, 'sessions-'));
  const ada = { session: 'session-1', name: 'Fix the sign-in form' };
  const bob = { session: 'session-2', name: 'Check console' };
  assert.ok(recording.recordTool({ action: 'start' }, base, ada).ok);
  assert.ok(recording.recordTool({ action: 'start' }, base, bob).ok);
  assert.ok(!recording.recordTool({ action: 'start' }, base, ada).ok);
  assert.ok(recording.recording(ada) && recording.recording(bob));
  assert.ok(!recording.recording({ session: 'session-3' }));
  const first = await recording.afterTool(page(), 'browser_click', '{"ref":1}', { ok: true, text: 'Clicked.', image: null }, ada);
  const other = await recording.afterTool(page(), 'browser_click', '{"ref":2}', { ok: true, text: 'Clicked.', image: null }, bob);
  assert.equal(await recording.afterTool(page(), 'browser_click', '{}', { ok: true, text: '', image: null }, { session: 'session-3' }), null);
  assert.ok(first && other);
  assert.notEqual(first.folder, other.folder);
  assert.match(path.basename(first.folder), /fix-the-sign-in-form-session-1$/);
  assert.equal(first.index, 1);
  assert.equal(other.index, 1);
  assert.equal(first.session, 'session-1');
  assert.equal(first.session_name, 'Fix the sign-in form');
  const stopped = recording.recordTool({ action: 'stop' }, base, ada);
  assert.ok(stopped.ok);
  assert.match(stopped.text, /Session: Fix the sign-in form/);
  assert.ok(recording.recording(bob));
  assert.ok(!recording.recordTool({ action: 'stop' }, base, ada).ok);
  assert.ok(recording.recordTool({ action: 'stop' }, base, bob).ok);
  fs.rmSync(base, { recursive: true, force: true });
});

test('secrets are never kept: typed into secret fields, or named like one', () => {
  assert.equal(JSON.parse(recording.redacted('browser_type', '{"ref":4,"text":"hunter2"}', [true])).text, recording.REDACTED);
  assert.equal(JSON.parse(recording.redacted('browser_type', '{"ref":4,"text":"ada"}', [false])).text, 'ada');
  const form = JSON.parse(
    recording.redacted('browser_fill_form', '{"fields":[{"ref":1,"value":"ada"},{"ref":2,"value":"hunter2"},{"ref":3,"value":true}]}', [false, true, true]),
  );
  assert.deepEqual(
    form.fields.map((/** @type {any} */ field) => field.value),
    ['ada', recording.REDACTED, true],
  );
  const named = JSON.parse(recording.redacted('browser_evaluate', '{"expression":"1","password":"x","nested":{"api_key":"k","Token":"t","url":"u"}}', []));
  assert.equal(named.password, recording.REDACTED);
  assert.equal(named.nested.api_key, recording.REDACTED);
  assert.equal(named.nested.Token, recording.REDACTED);
  assert.equal(named.nested.url, 'u');
  assert.equal(named.expression, '1');
  assert.doesNotMatch(recording.redacted('browser_type', 'not json "hunter2"', []), /hunter2/);
});

test('screenshots blur secret fields first, and are dropped when they cannot be', async () => {
  fs.mkdirSync(SCRATCH, { recursive: true });
  const folder = fs.mkdtempSync(path.join(SCRATCH, 'masked-'));
  const caller = { session: 'session-9', name: 'Log in' };
  assert.ok(recording.recordTool({ action: 'start', folder }, undefined, caller).ok);
  /** @type {string[]} */
  const calls = [];
  const typed = await recording.afterTool(page(calls), 'browser_type', '{"ref":4,"text":"hunter2"}', { ok: true, text: 'Typed.', image: null }, caller, [true]);
  assert.ok(typed && typed.picture);
  assert.doesNotMatch(typed.arguments, /hunter2/);
  const shot = calls.indexOf('Page.captureScreenshot');
  assert.ok(calls.indexOf('Runtime.evaluate') < shot && calls.lastIndexOf('Runtime.evaluate') > shot, 'masked before and restored after the capture');
  const blind = await recording.afterTool(page([], false), 'browser_click', '{}', { ok: true, text: 'Clicked.', image: null }, caller);
  assert.equal(blind && blind.picture, '');
  const tabs = await recording.afterTool(null, 'browser_tabs', '{"action":"list"}', { ok: true, text: 'Tab 1', image: null }, caller);
  assert.equal(tabs && tabs.tool, 'browser_tabs');
  assert.equal(tabs && tabs.picture, '');
  const kept = fs.readFileSync(path.join(folder, 'steps.json'), 'utf8');
  assert.doesNotMatch(kept, /hunter2/);
  assert.ok(recording.recordTool({ action: 'stop' }, undefined, caller).ok);
  fs.rmSync(folder, { recursive: true, force: true });
});
