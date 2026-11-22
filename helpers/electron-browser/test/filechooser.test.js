'use strict';

/**
 * Tests of the filters the open dialog gets from an input's `accept` attribute.
 */

const test = require('node:test');
const assert = require('node:assert');
const { filtersOf } = require('../src/filechooser');

test('extensions and known MIME types become one filter, then all files', () => {
  const filters = filtersOf('image/png, .PDF,application/json');
  assert.deepStrictEqual(filters, [
    { name: 'Accepted files', extensions: ['png', 'pdf', 'json'] },
    { name: 'All files', extensions: ['*'] },
  ]);
});

test('families of MIME types stand for their common extensions', () => {
  const [accepted] = filtersOf('image/*');
  assert.ok(accepted.extensions.includes('jpg'));
  assert.ok(accepted.extensions.includes('webp'));
});

test('an attribute that names nothing the dialog can filter by gives no filters', () => {
  assert.deepStrictEqual(filtersOf(''), []);
  assert.deepStrictEqual(filtersOf('application/x-unknown'), []);
  assert.deepStrictEqual(filtersOf('.not valid'), []);
});
