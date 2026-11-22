'use strict';

/**
 * What the window says about the pointer and the keyboard, put into the terms Electron uses.
 */

const { readerZoom } = require('./emulation');
const { offsetOf } = require('./frames');
const routing = require('./routing');

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

/** The bit the window sets for the key that carries editing shortcuts: command on macOS, control elsewhere. */
const SHORTCUT = process.platform === 'darwin' ? COMMAND : CONTROL;

/** @typedef {'cut' | 'copy' | 'paste' | 'selectAll' | 'undo' | 'redo'} ShortcutEdit */

/**
 * The editing the page is asked to do for a shortcut, by the lowercase letter pressed with the shortcut key. Chromium
 * binds these in its browser shell, which an offscreen page does not have, so a key event alone does nothing.
 * @type {Readonly<Record<string, ShortcutEdit>>}
 */
const EDITS = {
  x: 'cut',
  c: 'copy',
  v: 'paste',
  a: 'selectAll',
  z: 'undo',
  ...(process.platform === 'darwin' ? {} : { y: 'redo' }),
};

/**
 * The editing asked for by a letter pressed with the shortcut key and shift.
 * @type {Readonly<Record<string, ShortcutEdit>>}
 */
const SHIFTED_EDITS = {
  z: 'redo',
};

/**
 * The script that reads what is marked in a frame: the text of a text field's selection, or the document's selection
 * as text and HTML, looking through shadow roots for the focused field.
 */
const SELECTION_SCRIPT = `(() => {
  let active = document.activeElement;
  while (active && active.shadowRoot && active.shadowRoot.activeElement) active = active.shadowRoot.activeElement;
  if (active && (active.tagName === 'TEXTAREA' || active.tagName === 'INPUT') && typeof active.selectionStart === 'number') {
    if (active.type === 'password') return { text: '', html: '' };
    return { text: active.value.slice(active.selectionStart, active.selectionEnd), html: '' };
  }
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return { text: '', html: '' };
  const holder = document.createElement('div');
  for (let index = 0; index < selection.rangeCount; index += 1) holder.appendChild(selection.getRangeAt(index).cloneContents());
  return { text: selection.toString(), html: holder.innerHTML };
})()`;

/**
 * The keys Electron names by a word or by the character of a US layout, by Windows virtual-key code. A character is
 * the unshifted one, since Electron adds shift to a key it names by a shifted character.
 * @type {Readonly<Record<number, string>>}
 */
const NAMED_KEYS = {
  8: 'Backspace',
  9: 'Tab',
  13: 'Enter',
  16: 'Shift',
  17: 'Control',
  18: 'Alt',
  20: 'CapsLock',
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
  44: 'PrintScreen',
  45: 'Insert',
  46: 'Delete',
  91: 'Meta',
  106: 'nummult',
  107: 'numadd',
  109: 'numsub',
  110: 'numdec',
  111: 'numdiv',
  144: 'NumLock',
  145: 'ScrollLock',
  173: 'VolumeMute',
  174: 'VolumeDown',
  175: 'VolumeUp',
  176: 'MediaNextTrack',
  177: 'MediaPreviousTrack',
  178: 'MediaStop',
  179: 'MediaPlayPause',
  186: ';',
  187: '=',
  188: ',',
  189: '-',
  190: '.',
  191: '/',
  192: '`',
  219: '[',
  220: '\\',
  221: ']',
  222: "'",
};

/**
 * The key names of the DevTools protocol for the keys macOS edits text with, by Windows virtual-key code.
 * @type {Readonly<Record<number, string>>}
 */
const DOM_KEYS = {
  8: 'Backspace',
  37: 'ArrowLeft',
  38: 'ArrowUp',
  39: 'ArrowRight',
  40: 'ArrowDown',
  46: 'Delete',
};

/**
 * The editing commands macOS binds to a key pressed with command or with option, by Windows virtual-key code. An
 * offscreen page gets no such bindings from the system, so they are sent with the key. The movements also take shift,
 * which marks what they pass over.
 * @type {Readonly<Record<number, { command: string, option: string, marks: boolean }>>}
 */
const MAC_EDITING = {
  37: { command: 'moveToBeginningOfLine', option: 'moveWordLeft', marks: true },
  39: { command: 'moveToEndOfLine', option: 'moveWordRight', marks: true },
  38: { command: 'moveToBeginningOfDocument', option: 'moveToBeginningOfParagraph', marks: true },
  40: { command: 'moveToEndOfDocument', option: 'moveToEndOfParagraph', marks: true },
  8: { command: 'deleteToBeginningOfLine', option: 'deleteWordBackward', marks: false },
  46: { command: 'deleteToEndOfLine', option: 'deleteWordForward', marks: false },
};

/** The logical pixels the window turns one notch of a mouse wheel into. */
const NOTCH = 48;

/**
 * The pages in the middle of an input method's composition, so a composition that ended is not ended again.
 * @type {WeakSet<import('electron').WebContents>}
 */
const COMPOSING = new WeakSet();

/**
 * The script that finds where the caret, or the end of what is marked, is in a frame, in CSS pixels from its top left:
 * the end of the selection where it has a box, and otherwise the element that holds the keyboard, looking through
 * shadow roots.
 */
const CARET_SCRIPT = `(() => {
  let found = null;
  const selection = window.getSelection();
  if (selection && selection.rangeCount > 0) {
    const range = selection.getRangeAt(selection.rangeCount - 1).cloneRange();
    range.collapse(false);
    const boxes = range.getClientRects();
    found = boxes.length > 0 ? boxes[boxes.length - 1] : range.getBoundingClientRect();
  }
  if (!found || (found.width === 0 && found.height === 0)) {
    let active = document.activeElement;
    while (active && active.shadowRoot && active.shadowRoot.activeElement) active = active.shadowRoot.activeElement;
    if (active && active !== document.body) found = active.getBoundingClientRect();
  }
  return found ? { x: found.left, y: found.top, width: found.width, height: found.height } : null;
})()`;

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
  cell: 'Cross',
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
  'nwse-resize': 'ResizeNwse',
  'nw-resize': 'ResizeNwse',
  'se-resize': 'ResizeNwse',
  'nesw-resize': 'ResizeNesw',
  'ne-resize': 'ResizeNesw',
  'sw-resize': 'ResizeNesw',
  grab: 'Grab',
  grabbing: 'Grabbing',
  move: 'Move',
  'all-scroll': 'Move',
  'm-panning': 'Move',
  'e-panning': 'Move',
  'n-panning': 'Move',
  'ne-panning': 'Move',
  'nw-panning': 'Move',
  's-panning': 'Move',
  'se-panning': 'Move',
  'sw-panning': 'Move',
  'w-panning': 'Move',
  'not-allowed': 'NotAllowed',
  'no-drop': 'NotAllowed',
  nodrop: 'NotAllowed',
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
 * @returns {string | null} Its name, such as `Enter`, `A`, `num5`, `/` or `F5`, or nothing when it has none.
 */
function keyName(code) {
  if (NAMED_KEYS[code]) return NAMED_KEYS[code];
  if (code >= 65 && code <= 90) return String.fromCharCode(code);
  if (code >= 48 && code <= 57) return String.fromCharCode(code);
  if (code >= 96 && code <= 105) return `num${code - 96}`;
  if (code >= 112 && code <= 135) return `F${code - 111}`;
  return null;
}

/**
 * The modifiers held, as the bits the DevTools protocol counts them in.
 * @param {number} bits The modifiers held, as the window counts them.
 * @returns {number} Alt as 1, control as 2, command as 4 and shift as 8.
 */
function protocolModifiers(bits) {
  return (bits & 8 ? 1 : 0) | (bits & CONTROL ? 2 : 0) | (bits & COMMAND ? 4 : 0) | (bits & 2 ? 8 : 0);
}

/**
 * The Electron key events for one press or release of a key the window reports. Enter types its carriage return only
 * when the window says it typed one, which it does not while control or alt is held.
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
 * The macOS editing command a press of `key` asks for: command or option with an arrow, Backspace or Delete, shift
 * marking what a movement passes over.
 * @param {Key} key The key.
 * @param {string} [platform] The system the helper runs on, as Node names it.
 * @returns {string | null} The command, such as `moveWordLeftAndModifySelection`, or nothing when the key is not one
 *   or the system is not macOS.
 */
function macEditing(key, platform = process.platform) {
  if (platform !== 'darwin' || !key.pressed || key.modifiers & CONTROL) return null;
  const binding = MAC_EDITING[key.code];
  if (!binding) return null;
  const command = key.modifiers & COMMAND;
  const option = key.modifiers & 8;
  if (!command === !option) return null;
  if (key.modifiers & 2 && !binding.marks) return null;
  const named = command ? binding.command : binding.option;
  return key.modifiers & 2 ? `${named}AndModifySelection` : named;
}

/**
 * Whether the page's DevTools protocol can be reached, which the input method and the keys Electron cannot name need.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {boolean} Whether it can.
 */
function attached(contents) {
  try {
    return contents.debugger.isAttached();
  } catch {
    return false;
  }
}

/**
 * Sends a press or release of `key` through the DevTools protocol, with the editing `commands` it carries: for the
 * macOS editing keys and for keys Electron has no name for, such as the context menu key.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {Key} key The key.
 * @param {string[]} commands The editing commands to run with it.
 * @returns {void}
 */
function rawKey(contents, key, commands) {
  contents.debugger
    .sendCommand('Input.dispatchKeyEvent', {
      type: key.pressed ? 'rawKeyDown' : 'keyUp',
      modifiers: protocolModifiers(key.modifiers),
      windowsVirtualKeyCode: key.code,
      ...(DOM_KEYS[key.code] ? { key: DOM_KEYS[key.code] } : {}),
      commands,
    })
    .catch(() => {});
}

/**
 * Sends a press or release of `key` to the page: an editing shortcut the helper runs itself is handled before this, a
 * macOS editing key goes with its command, a key Electron can name or that types text goes as Electron's events, and
 * any other key with a code goes through the DevTools protocol.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {Key} key The key.
 * @returns {void}
 */
function sendKey(contents, key) {
  const command = macEditing(key);
  if (command && attached(contents)) {
    rawKey(contents, key, [command]);
    return;
  }
  const events = keyEvents(key);
  if (events.length === 0 && key.code && attached(contents)) {
    rawKey(contents, key, []);
    return;
  }
  for (const event of events) contents.sendInputEvent(event);
}

/**
 * Shows `text` an input method is composing in the page, with the caret at its end, and reports where it is so the
 * input method's candidates sit by it; empty text ends a composition that is under way without typing.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {number} page The page's number.
 * @param {string} text What is being composed.
 * @param {(event: import('./types').Event) => void} send Reports to the window.
 * @returns {void}
 */
function compose(contents, page, text, send) {
  if (!text && !COMPOSING.has(contents)) return;
  if (!attached(contents)) return;
  if (text) COMPOSING.add(contents);
  else COMPOSING.delete(contents);
  contents.debugger
    .sendCommand('Input.imeSetComposition', { text, selectionStart: text.length, selectionEnd: text.length })
    .then(() => (text ? reportCaret(contents, page, send) : undefined))
    .catch(() => {});
}

/**
 * Reports where the caret is in the page, in logical pixels of the page, as found in the frame that holds the keyboard.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {number} page The page's number.
 * @param {(event: import('./types').Event) => void} send Reports to the window.
 * @returns {Promise<void>} Settles once reported, or once it is clear there is nothing to report.
 */
async function reportCaret(contents, page, send) {
  try {
    const frame = focusedFrame(contents);
    const found = /** @type {{ x: number, y: number, width: number, height: number } | null} */ (
      await frame.executeJavaScript(CARET_SCRIPT)
    );
    if (!found || contents.isDestroyed()) return;
    const offset = await offsetOf(frame);
    if (!offset || contents.isDestroyed()) return;
    const zoom = readerZoom(contents);
    send({
      Caret: {
        page,
        x: Math.round((found.x + offset.x) * zoom),
        y: Math.round((found.y + offset.y) * zoom),
        width: Math.round(found.width * zoom),
        height: Math.round(found.height * zoom),
      },
    });
  } catch {
    /* the page went away or refused the script */
  }
}

/**
 * The editing a press of `key` asks for, when it is an editing shortcut.
 * @param {Key} key The key.
 * @returns {ShortcutEdit | null} The editing, or nothing when the key is not a shortcut.
 */
function shortcutEdit(key) {
  if (!key.pressed || !(key.modifiers & SHORTCUT) || key.modifiers & 8) return null;
  const letter = keyName(key.code);
  if (!letter || letter.length !== 1) return null;
  const lower = letter.toLowerCase();
  if (key.modifiers & 2) return SHIFTED_EDITS[lower] || null;
  return EDITS[lower] || null;
}

/**
 * The frame that holds the keyboard in `contents`, where what is marked lives, or the main frame when none does.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {import('electron').WebFrameMain} The frame.
 */
function focusedFrame(contents) {
  return contents.focusedFrame || contents.mainFrame;
}

/**
 * What is marked in `contents`, read in the frame that holds the keyboard.
 * @param {import('electron').WebContents} contents The page's contents.
 * @returns {Promise<{ text: string, html: string }>} The marked text and its HTML; both empty when nothing is marked or it cannot be read.
 */
async function selection(contents) {
  try {
    const found = /** @type {{ text?: unknown, html?: unknown } | null} */ (
      await focusedFrame(contents).executeJavaScript(SELECTION_SCRIPT)
    );
    return {
      text: typeof found?.text === 'string' ? found.text : '',
      html: typeof found?.html === 'string' ? found.html : '',
    };
  } catch {
    return { text: '', html: '' };
  }
}

/**
 * Copies what is marked in the page to the window's clipboard by reporting it, since the hidden engine cannot reach
 * the system clipboard, and for a cut deletes it from the page afterwards.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {number} page The page's number.
 * @param {boolean} cut Whether to delete what was marked once it is copied.
 * @param {(event: import('./types').Event) => void} send Reports to the window.
 * @returns {Promise<void>} Settles once the copy is reported and any cut done.
 */
async function copyOut(contents, page, cut, send) {
  const marked = await selection(contents);
  if (!marked.text) return;
  send({ Copied: { page, text: marked.text, html: marked.html } });
  if (cut && !contents.isDestroyed()) contents.delete();
}

/**
 * Carries out an editing shortcut or request on the page.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {number} page The page's number.
 * @param {ShortcutEdit} edit The editing.
 * @param {(event: import('./types').Event) => void} send Reports to the window.
 * @returns {void}
 */
function runEdit(contents, page, edit, send) {
  if (edit === 'copy' || edit === 'cut') {
    void copyOut(contents, page, edit === 'cut', send);
    return;
  }
  contents[edit]();
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
 * The editing the window's edit request names, as the page's contents call it.
 * @type {Readonly<Record<string, ShortcutEdit>>}
 */
const EDIT_ACTIONS = {
  Cut: 'cut',
  Copy: 'copy',
  Paste: 'paste',
  SelectAll: 'selectAll',
  Undo: 'undo',
  Redo: 'redo',
};

/**
 * A point the window gave in logical pixels, as the page's window takes it: in its own pixels, which are device pixels
 * since every window is sized in them (see `ENGINE_SCALE` in `surface.js`).
 * @param {number} x Across, in logical pixels.
 * @param {number} y Down, in logical pixels.
 * @param {number} scale The pane's display scale, device pixels per logical pixel.
 * @returns {{ x: number, y: number }} The point in the window's pixels.
 */
function toWindow(x, y, scale) {
  return { x: Math.round(x * scale), y: Math.round(y * scale) };
}

/**
 * The buttons held, as the DevTools protocol counts them, from the modifier bits the window sets.
 * @param {number} bits The modifiers held, as the window counts them.
 * @returns {number} Left as 1, right as 2 and middle as 4.
 */
function protocolButtons(bits) {
  return (bits & 16 ? 1 : 0) | (bits & 64 ? 2 : 0) | (bits & 32 ? 4 : 0);
}

/**
 * The button a move reports while buttons are held, as the DevTools protocol names it.
 * @param {number} bits The modifiers held, as the window counts them.
 * @returns {'none' | 'left' | 'middle' | 'right'} The first button held, or none.
 */
function heldButton(bits) {
  if (bits & 16) return 'left';
  if (bits & 64) return 'right';
  if (bits & 32) return 'middle';
  return 'none';
}

/**
 * Sends a pointer event through the page's router, which passes it to the frame under it even when that frame is a
 * cross-origin frame in a process of its own; Electron's own input events reach only the page's top frame.
 * @param {import('./routing').Router} router The page's router.
 * @param {'mouseMoved' | 'mousePressed' | 'mouseReleased'} type The event.
 * @param {{ x: number, y: number }} at Where, in the window's pixels.
 * @param {number} bits The modifiers and buttons held, as the window counts them.
 * @param {'none' | 'left' | 'middle' | 'right'} button The button pressed or released; for a move, ignored for the one held.
 * @param {number} clicks The click count of a press or release.
 * @returns {void}
 */
function pointer(router, type, at, bits, button, clicks) {
  const own = { none: 0, left: 1, right: 2, middle: 4 }[button];
  const before = protocolButtons(bits);
  const held = type === 'mousePressed' ? before | own : type === 'mouseReleased' ? before & ~own : before;
  void router.send({
    type,
    ...at,
    modifiers: protocolModifiers(bits),
    button: type === 'mouseMoved' ? heldButton(bits) : button,
    buttons: held,
    clickCount: type === 'mouseMoved' ? 0 : clicks,
  });
}

/**
 * Types committed or pasted text into the frame that holds the keyboard through the page's DevTools protocol. Electron's
 * `insertText` crashes the page when that frame is a cross-origin frame, so it is used only when the protocol is out of reach.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {string} text The text.
 * @returns {void}
 */
function typeText(contents, text) {
  if (!attached(contents)) {
    contents.insertText(text);
    return;
  }
  contents.debugger.sendCommand('Input.insertText', { text }).catch(() => {});
}

/**
 * Sends the input of `request` to `contents`. Points are given in logical pixels and sent in the window's; wheel
 * distances stay as given, since the engine scrolls them in CSS pixels.
 * @param {import('electron').WebContents} contents The page's contents.
 * @param {{ [name: string]: unknown }} request The request, as the window sent it.
 * @param {(event: import('./types').Event) => void} [send] Reports to the window, as copying does; without it a copy reports nothing.
 * @param {number} [scale] The pane's display scale, device pixels per logical pixel; 1 when not given.
 * @returns {boolean} Whether it was an input request.
 */
function deliver(contents, request, send = () => {}, scale = 1) {
  const input = /** @type {InputRequest} */ (request);
  if ('PointerMoved' in input) {
    const body = input.PointerMoved;
    const router = routing.of(contents);
    if (router && body.left) void router.leave();
    if (router && !body.left) {
      pointer(router, 'mouseMoved', toWindow(body.x, body.y, scale), body.modifiers, 'none', 0);
      return true;
    }
    contents.sendInputEvent({
      type: body.left ? 'mouseLeave' : 'mouseMove',
      ...toWindow(body.x, body.y, scale),
      modifiers: modifiersOf(body.modifiers),
    });
    return true;
  }
  if ('PointerButton' in input) {
    const body = input.PointerButton;
    const button = /** @type {'left' | 'middle' | 'right'} */ (body.button.toLowerCase());
    const router = routing.of(contents);
    if (router) {
      pointer(router, body.pressed ? 'mousePressed' : 'mouseReleased', toWindow(body.x, body.y, scale), body.modifiers, button, body.clicks);
      return true;
    }
    contents.sendInputEvent({
      type: body.pressed ? 'mouseDown' : 'mouseUp',
      ...toWindow(body.x, body.y, scale),
      button,
      clickCount: body.clicks,
      modifiers: modifiersOf(body.modifiers),
    });
    return true;
  }
  if ('Scrolled' in input) {
    const body = input.Scrolled;
    const deltaX = typeof body.exact_dx === 'number' ? body.exact_dx : body.dx;
    const deltaY = typeof body.exact_dy === 'number' ? body.exact_dy : body.dy;
    contents.sendInputEvent({
      type: 'mouseWheel',
      ...toWindow(body.x, body.y, scale),
      deltaX,
      deltaY,
      wheelTicksX: deltaX / NOTCH,
      wheelTicksY: deltaY / NOTCH,
      hasPreciseScrollingDeltas: Boolean(body.precise),
      canScroll: true,
      modifiers: modifiersOf(body.modifiers),
    });
    return true;
  }
  if ('Key' in input) {
    const edit = shortcutEdit(input.Key.key);
    if (edit) {
      runEdit(contents, input.Key.page, edit, send);
      return true;
    }
    sendKey(contents, input.Key.key);
    return true;
  }
  if ('CommitText' in input) {
    COMPOSING.delete(contents);
    typeText(contents, input.CommitText.text);
    return true;
  }
  if ('Compose' in input) {
    compose(contents, input.Compose.page, input.Compose.text, send);
    return true;
  }
  if ('PasteText' in input) {
    typeText(contents, input.PasteText.text);
    return true;
  }
  if ('Edit' in input) {
    const edit = EDIT_ACTIONS[input.Edit.action];
    if (edit) runEdit(contents, input.Edit.page, edit, send);
    return true;
  }
  return false;
}

module.exports = { cursorOf, deliver, keyEvents, keyName, macEditing, modifiersOf, shortcutEdit, toWindow };
