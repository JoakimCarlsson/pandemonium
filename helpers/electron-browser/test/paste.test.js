'use strict';

/**
 * Tests of what a rich paste and a drop of files carry into a page: which pastes are rich, the files read for them
 * and their types, and the files a drop keeps.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const { isRich, mediaType, pasteScript, readFiles } = require('../src/paste');
const { existingFiles } = require('../src/drop');

/**
 * A new folder for the scratch files of a test, under `~/.cache/scratch`, never a RAM-backed temporary folder.
 * @returns {string} The folder.
 */
function scratch() {
  const root = path.join(os.homedir(), '.cache', 'scratch', 'pandemonium');
  fs.mkdirSync(root, { recursive: true });
  return fs.mkdtempSync(path.join(root, 'paste-'));
}

test('a paste is rich when it carries HTML or files', () => {
  assert.equal(isRich({ html: null, files: [] }), false);
  assert.equal(isRich({}), false);
  assert.equal(isRich({ html: '<b>a</b>' }), true);
  assert.equal(isRich({ files: ['/a.png'] }), true);
});

test('files are read with their media types, and what is not a readable absolute file is left out', () => {
  const folder = scratch();
  try {
    const picture = path.join(folder, 'shot.PNG');
    fs.writeFileSync(picture, Buffer.from([1, 2, 3]));
    const files = readFiles([picture, 'relative.png', path.join(folder, 'missing.txt'), folder, 7]);
    assert.deepEqual(files, [{ name: 'shot.PNG', type: 'image/png', data: Buffer.from([1, 2, 3]).toString('base64') }]);
    assert.equal(mediaType('/x/archive.unknown'), 'application/octet-stream');
    assert.deepEqual(existingFiles([picture, 'shot.png', folder, null]), [picture]);
  } finally {
    fs.rmSync(folder, { recursive: true, force: true });
  }
});

test('the paste script carries what is pasted as data, not as code', () => {
  const script = pasteScript({ text: "'); alert(1); ('", html: '</script>', files: [] });
  assert.ok(script.includes(JSON.stringify("'); alert(1); ('")));
  assert.ok(script.startsWith('(() => {'));
});
