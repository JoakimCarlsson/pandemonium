'use strict';

/**
 * What the window says about the pointer and the keyboard, put into the terms Electron uses.
 */

/** @typedef {import('./types').Cursor} Cursor */
/** @typedef {import('./types').InputRequest} InputRequest */
/** @typedef {import('./types').Key} Key */

/**
 * The modifier names Electron uses, by the bit the window sets for each.
 * @type {ReadonlyArray<readonly [number, 'shift' | 'control' | 'alt' | 'leftbuttondown' | 'middlebuttondown' | 'rightbuttondown' | 'meta']>}
 */
const MODIFIERS = [
  [2, 'shift'],
  [4, 'control'],
  [8, 'alt'],
  [16, 'leftbuttondown'],
  [32, 'middlebuttondown'],
  [64, 'rightbuttondown'],
  [128, 'meta'],
];

/** The bit the window sets while control is held. */
const CONTROL = 4;

/** The bit the window sets while the command key is held. */
const COMMAND = 128;

/**
 * The keys that have names rather than a character, by Windows virtual-key code.
 * @type {Readonly<Record<number, string>>}
 */
const NAMED_KEYS = {
  8: 'Backspace',
  9: 'Tab',
  13: 'Enter',
  27: 'Escape',
  32: 'Space',
  33: 'PageUp',
  34: 'PageDown',
  35: 'End',
  36: 'Home',
  37: 'Left',
  38: 'Up',
  39: 'Right',
  40: 'Down',
  45: 'Insert',
  46: 'Delete',
};

/**
 * How the pointer looks for each cursor type the engine reports. The engine names cursors as Chromium does, not as CSS
 * does: its `pointer` is the ordinary arrow and its `hand` is the hand over a link. Types not listed, such as `pointer`
 * and `null`, are the arrow.
 * @type {Readonly<Record<string, Cursor>>}
 */
const CURSORS = {
  hand: 'Hand',
  text: 'Text',
  'vertical-text': 'Text',
  crosshair: 'Cross',
  wait: 'Wait',
  progress: 'Wait',
  help: 'Help',
  'col-resize': 'ResizeHorizontal',
  'ew-resize': 'ResizeHorizontal',
  'e-resize': 'ResizeHorizontal',
  'w-resize': 'ResizeHorizontal',
  'row-resize': 'ResizeVertical',
  'ns-resize': 'ResizeVertical',
  'n-resize': 'ResizeVertical',
  's-resize': 'ResizeVertical',
};

/**
 * The Electron modifier names for the bits in `bits`.
 * @param {number} bits The modifiers and buttons held, as the window counts them.
 * @returns {Array<'shift' | 'control' | 'alt' | 'leftbuttondown' | 'middlebuttondown' | 'rightbuttondown' | 'meta'>} Their names.
 */
function modifiersOf(bits) {
  return MODIFIERS.filter(([bit]) => (bits & bit) !== 0).map(([, name]) => name);
}

/**
 * The name of the key with Windows virtual-key code `code`, when it has one an input event can carry.
 * @param {number} code The key's virtual-key code.
 * @returns {string | null} Its name, such as `Enter`, `A` or `F5`, or nothing when it has none.
 */
function keyName(code) {
  if (NAMED_KEYS[code]) return NAMED_KEYS[code];
  if (code >= 65 && code <= 90) return String.fromCharCode(code);
  if (code >= 48 && code <= 57) return String.fromCharCode(code);
  if (code >= 112 && code <= 123) return `F${code - 111}`;
  return null;
}

/**
 * The Electron key events for one press or release of a key the window reports.
 * @param {Key} key The key.
 * @returns {Array<import('electron').KeyboardInputEvent>} The events to send, in order.
 */
function keyEvents(key) {
  const type = key.pressed ? 'keyDown' : 'keyUp';
  const modifiers = modifiersOf(key.modifiers);
  const name = keyName(key.code);
  /** @type {Array<import('electron').KeyboardInputEvent>} */
  const events = [];
  if (name) {
    events.push({ type, keyCode: name, modifiers });
  } else if (key.text) {
    events.push({ type, keyCode: key.text, modifiers });
  }
  if (key.pressed && key.text && !(key.modifiers & (CONTROL | COMMAND))) {
    events.push({ type: 'char', keyCode: key.text, modifiers });
  }
  return events;
}

/**
 * How the window should draw the pointer for the engine's cursor `type`.
 * @param {string} type The engine's name for the cursor, such as `hand`.
 * @returns {Cursor} The look the window knows.
 */
function cursorOf(type) {
  return CURSORS[type] || 'Arrow';
}

/**
 * Sends the input of `request` to `contents`.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {{ [name: string]: unknown }} request The request, as the window sent it.
 * @returns {boolean} Whether it was an input request.
 */
function deliver(contents, request) {
  const input = /** @type {InputRequest} */ (request);
  if ('PointerMoved' in input) {
    const body = input.PointerMoved;
    contents.sendInputEvent({
      type: body.left ? 'mouseLeave' : 'mouseMove',
      x: body.x,
      y: body.y,
      modifiers: modifiersOf(body.modifiers),
    });
    return true;
  }
  if ('PointerButton' in input) {
    const body = input.PointerButton;
    contents.sendInputEvent({
      type: body.pressed ? 'mouseDown' : 'mouseUp',
      x: body.x,
      y: body.y,
      button: /** @type {'left' | 'middle' | 'right'} */ (body.button.toLowerCase()),
      clickCount: body.clicks,
      modifiers: modifiersOf(body.modifiers),
    });
    return true;
  }
  if ('Scrolled' in input) {
    const body = input.Scrolled;
    contents.sendInputEvent({
      type: 'mouseWheel',
      x: body.x,
      y: body.y,
      deltaX: body.dx,
      deltaY: body.dy,
      canScroll: true,
      modifiers: modifiersOf(body.modifiers),
    });
    return true;
  }
  if ('Key' in input) {
    for (const event of keyEvents(input.Key.key)) contents.sendInputEvent(event);
    return true;
  }
  if ('CommitText' in input) {
    contents.insertText(input.CommitText.text);
    return true;
  }
  if ('Edit' in input) {
    switch (input.Edit.action) {
      case 'Cut':
        contents.cut();
        break;
      case 'Copy':
        contents.copy();
        break;
      case 'Paste':
        contents.paste();
        break;
      case 'SelectAll':
        contents.selectAll();
        break;
      default:
        break;
    }
    return true;
  }
  return false;
}

module.exports = { cursorOf, deliver, modifiersOf };
