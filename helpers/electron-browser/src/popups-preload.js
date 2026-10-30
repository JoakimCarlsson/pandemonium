'use strict';

/**
 * Drop-down lists and date pickers for pages, drawn inside the page.
 *
 * Chromium draws the popup of a `<select>` or of a date, time or month input as a separate widget. An offscreen
 * Electron page never paints that widget, so a click on one seems to do nothing. This preload, which runs in every
 * page's isolated world, catches the click and draws a small popup of its own in a closed shadow root at the top of
 * the page, so it is painted with the page. Choosing something sets the value and fires `input` and `change`, as the
 * engine's own popup would. Lists with `multiple` or a `size` are drawn by the page itself and are left alone.
 */

/** How far from the right edge of a date input, in pixels, a click counts as one on the picker button. */
const BUTTON_ZONE = 30;

/** The gap between an input and its popup, in pixels. */
const GAP = 2;

/** The input types that have a picker, and what each asks for. */
const PICKERS = {
  date: { date: true, time: false },
  month: { date: true, time: false },
  time: { date: false, time: true },
  'datetime-local': { date: true, time: true },
};

/** The look of every popup. */
const STYLE = `
  :host { all: initial; }
  .panel { position: fixed; z-index: 2147483647; box-sizing: border-box; background: #fff; color: #1b1b1f;
    border: 1px solid #8a8a93; border-radius: 6px; box-shadow: 0 6px 20px rgba(0,0,0,.28); color-scheme: light;
    font: 13px system-ui, sans-serif; user-select: none; }
  .list { overflow-y: auto; padding: 4px 0; max-height: 280px; }
  .item { padding: 4px 12px; white-space: nowrap; cursor: default; }
  .item.on { background: #1a73e8; color: #fff; }
  .item.off { color: #8a8a93; }
  .group { padding: 4px 8px; font-weight: 600; color: #55555e; }
  .nested { padding-left: 24px; }
  .bar { display: flex; align-items: center; justify-content: space-between; padding: 6px 8px; font-weight: 600; }
  .bar button, .foot button { font: inherit; border: 0; background: none; padding: 2px 8px; border-radius: 4px; cursor: pointer; color: inherit; }
  .bar button:hover, .foot button:hover { background: #e4e4ea; }
  .grid { display: grid; grid-template-columns: repeat(7, 32px); gap: 2px; padding: 0 8px 6px; text-align: center; }
  .months { grid-template-columns: repeat(3, 64px); }
  .grid .head { color: #55555e; font-size: 11px; line-height: 24px; }
  .cell { line-height: 28px; border-radius: 4px; cursor: pointer; }
  .months .cell { line-height: 36px; }
  .cell:hover { background: #e4e4ea; }
  .cell.today { box-shadow: inset 0 0 0 1px #1a73e8; }
  .cell.on, .cell.on:hover { background: #1a73e8; color: #fff; }
  .cell.off { color: #b8b8c0; pointer-events: none; }
  .clock { display: flex; align-items: center; gap: 4px; padding: 6px 8px; justify-content: center; }
  .clock input { width: 48px; font: inherit; padding: 3px 4px; }
  .foot { display: flex; justify-content: space-between; padding: 4px 8px 6px; }
`;

/** @type {{ host: HTMLElement, close: () => void } | null} The popup that is showing, if one is. */
let shown = null;

/**
 * Closes the popup that is open.
 * @returns {void}
 */
function closePopup() {
  if (shown) shown.close();
}

/**
 * Makes an element with a class and some text.
 * @param {string} tag The element's name.
 * @param {string} [name] Its class.
 * @param {string} [text] Its text.
 * @returns {HTMLElement} The element.
 */
function make(tag, name, text) {
  const element = document.createElement(tag);
  if (name) element.className = name;
  if (text !== undefined) element.textContent = text;
  return element;
}

/**
 * Opens a popup under an input, or above it when there is no room below.
 * @param {HTMLElement} anchor The input the popup belongs to.
 * @param {HTMLElement} panel The popup's contents.
 * @param {(event: KeyboardEvent) => boolean} onKey Handles a key while the popup is open; returns whether it took it.
 * @param {boolean} [matchWidth] Whether the popup is at least as wide as the input.
 * @returns {void}
 */
function show(anchor, panel, onKey, matchWidth = false) {
  closePopup();
  const host = document.createElement('div');
  const root = host.attachShadow({ mode: 'closed' });
  const style = document.createElement('style');
  style.textContent = STYLE;
  panel.classList.add('panel');
  root.append(style, panel);
  document.documentElement.append(host);
  const box = anchor.getBoundingClientRect();
  if (matchWidth) panel.style.minWidth = `${box.width}px`;
  const size = panel.getBoundingClientRect();
  const below = window.innerHeight - box.bottom - GAP;
  const top = size.height <= below || box.top < size.height ? box.bottom + GAP : box.top - size.height - GAP;
  panel.style.top = `${Math.max(0, Math.min(top, window.innerHeight - size.height))}px`;
  panel.style.left = `${Math.max(0, Math.min(box.left, window.innerWidth - size.width))}px`;

  const away = (/** @type {Event} */ event) => {
    if (event.target !== host) close();
  };
  const typed = (/** @type {KeyboardEvent} */ event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (onKey(event)) {
      event.preventDefault();
      event.stopPropagation();
    }
  };
  const scrolled = (/** @type {Event} */ event) => {
    if (event.target !== host && !(event.target instanceof Node && panel.contains(event.target))) close();
  };
  function close() {
    window.removeEventListener('mousedown', away, true);
    window.removeEventListener('keydown', typed, true);
    window.removeEventListener('scroll', scrolled, true);
    window.removeEventListener('resize', close);
    window.removeEventListener('blur', close);
    host.remove();
    if (shown && shown.host === host) shown = null;
  }
  window.addEventListener('mousedown', away, true);
  window.addEventListener('keydown', typed, true);
  window.addEventListener('scroll', scrolled, true);
  window.addEventListener('resize', close);
  window.addEventListener('blur', close);
  shown = { host, close };
}

/**
 * Gives an input a new value the way a person would: the value, then `input` and `change`.
 * @param {HTMLInputElement | HTMLSelectElement} field The field.
 * @param {() => void} set Changes the field.
 * @returns {void}
 */
function commit(field, set) {
  set();
  field.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
  field.dispatchEvent(new Event('change', { bubbles: true }));
}

/**
 * Opens the list of a `<select>`.
 * @param {HTMLSelectElement} select The select.
 * @returns {void}
 */
function showSelect(select) {
  const panel = make('div', 'list');
  /** @type {Array<{ option: HTMLOptionElement, row: HTMLElement }>} */
  const rows = [];
  let current = select.selectedIndex;
  const paint = () => {
    for (const { option, row } of rows) row.classList.toggle('on', option.index === current);
    const chosen = rows.find(({ option }) => option.index === current);
    if (chosen) chosen.row.scrollIntoView({ block: 'nearest' });
  };
  const choose = (/** @type {number} */ index) => {
    if (index !== select.selectedIndex) commit(select, () => (select.selectedIndex = index));
    closePopup();
    select.focus();
  };
  for (const child of Array.from(select.children)) {
    const group = child instanceof HTMLOptGroupElement ? child : null;
    if (group) panel.append(make('div', 'group', group.label));
    for (const option of group ? Array.from(group.children) : [child]) {
      if (!(option instanceof HTMLOptionElement)) continue;
      const row = make('div', `item${group ? ' nested' : ''}${option.disabled || (group && group.disabled) ? ' off' : ''}`, option.label || option.text);
      if (!row.classList.contains('off')) {
        row.addEventListener('mouseover', () => {
          current = option.index;
          paint();
        });
        row.addEventListener('mouseup', () => choose(option.index));
      }
      rows.push({ option, row });
      panel.append(row);
    }
  }
  const enabled = () => rows.filter(({ option }) => !option.disabled).map(({ option }) => option.index);
  const step = (/** @type {number} */ by) => {
    const all = enabled();
    const at = all.indexOf(current);
    current = all[Math.max(0, Math.min(all.length - 1, at + by))] ?? current;
    paint();
  };
  show(
    select,
    panel,
    (event) => {
      if (event.key === 'ArrowDown') step(1);
      else if (event.key === 'ArrowUp') step(-1);
      else if (event.key === 'Home') step(-rows.length);
      else if (event.key === 'End') step(rows.length);
      else if (event.key === 'Enter' || event.key === ' ') choose(current);
      else return event.key !== 'Tab';
      return true;
    },
    true,
  );
  for (const { row } of rows) row.addEventListener('mousedown', (event) => event.stopPropagation());
  paint();
}

/**
 * Adds a zero in front of a number that has one digit.
 * @param {number} value The number.
 * @param {number} [width] How many digits.
 * @returns {string} The text.
 */
function pad(value, width = 2) {
  return String(value).padStart(width, '0');
}

/**
 * The parts of a date, time or month value.
 * @param {string} value What the input holds.
 * @returns {{ year: number, month: number, day: number, hour: number, minute: number, set: boolean }} The parts; today and now when the value is empty.
 */
function parts(value) {
  const now = new Date();
  const found = /^(\d{4,})-(\d{2})(?:-(\d{2}))?/.exec(value);
  const clock = /(\d{2}):(\d{2})/.exec(value);
  return {
    year: found ? Number(found[1]) : now.getFullYear(),
    month: found ? Number(found[2]) - 1 : now.getMonth(),
    day: found && found[3] ? Number(found[3]) : now.getDate(),
    hour: clock ? Number(clock[1]) : now.getHours(),
    minute: clock ? Number(clock[2]) : now.getMinutes(),
    set: value !== '',
  };
}

/**
 * Opens the picker of a date, month, time or date-and-time input.
 * @param {HTMLInputElement} input The input.
 * @returns {void}
 */
function showPicker(input) {
  const kind = /** @type {keyof typeof PICKERS} */ (input.type);
  const { date, time } = PICKERS[kind];
  const at = parts(input.value);
  const state = { ...at };
  const view = { year: at.year, month: at.month };
  const first = input.min ? parts(input.min) : null;
  const last = input.max ? parts(input.max) : null;
  const panel = make('div');
  const names = new Intl.DateTimeFormat(navigator.language, { month: 'short' });
  const title = new Intl.DateTimeFormat(navigator.language, { month: 'long', year: 'numeric' });
  const weekday = new Intl.DateTimeFormat(navigator.language, { weekday: 'narrow' });

  const text = () => {
    const day = `${pad(state.year, 4)}-${pad(state.month + 1)}${kind === 'month' ? '' : `-${pad(state.day)}`}`;
    const clock = `${pad(state.hour)}:${pad(state.minute)}`;
    return kind === 'time' ? clock : time ? `${day}T${clock}` : day;
  };
  const send = () => {
    state.set = true;
    commit(input, () => (input.value = text()));
  };
  const outside = (/** @type {number} */ year, /** @type {number} */ month, /** @type {number | null} */ day) => {
    const key = year * 10000 + month * 100 + (day ?? 0);
    const low = first ? first.year * 10000 + first.month * 100 + (day === null ? 0 : first.day) : -Infinity;
    const high = last ? last.year * 10000 + last.month * 100 + (day === null ? 0 : last.day) : Infinity;
    return key < low || key > high;
  };

  const draw = () => {
    panel.replaceChildren();
    if (date) {
      const bar = make('div', 'bar');
      const back = make('button', '', '‹');
      const forward = make('button', '', '›');
      const label = make('span', '', kind === 'month' ? String(view.year) : title.format(new Date(view.year, view.month, 1)));
      const turn = (/** @type {number} */ by) => () => {
        if (kind === 'month') view.year += by;
        else {
          const moved = new Date(view.year, view.month + by, 1);
          view.year = moved.getFullYear();
          view.month = moved.getMonth();
        }
        draw();
      };
      back.addEventListener('click', turn(-1));
      forward.addEventListener('click', turn(1));
      bar.append(back, label, forward);
      panel.append(bar);
      const grid = make('div', kind === 'month' ? 'grid months' : 'grid');
      if (kind === 'month') {
        for (let month = 0; month < 12; month += 1) {
          const cell = make('div', 'cell', names.format(new Date(view.year, month, 1)));
          const chosen = state.set && state.year === view.year && state.month === month;
          cell.classList.toggle('on', chosen);
          cell.classList.toggle('off', outside(view.year, month, null));
          cell.addEventListener('click', () => {
            state.year = view.year;
            state.month = month;
            send();
            closePopup();
          });
          grid.append(cell);
        }
      } else {
        for (let index = 0; index < 7; index += 1) grid.append(make('div', 'head', weekday.format(new Date(2024, 0, 1 + index))));
        const lead = (new Date(view.year, view.month, 1).getDay() + 6) % 7;
        for (let blank = 0; blank < lead; blank += 1) grid.append(make('div'));
        const days = new Date(view.year, view.month + 1, 0).getDate();
        const now = new Date();
        for (let day = 1; day <= days; day += 1) {
          const cell = make('div', 'cell', String(day));
          cell.classList.toggle('on', state.set && state.year === view.year && state.month === view.month && state.day === day);
          cell.classList.toggle('today', now.getFullYear() === view.year && now.getMonth() === view.month && now.getDate() === day);
          cell.classList.toggle('off', outside(view.year, view.month, day));
          cell.addEventListener('click', () => {
            state.year = view.year;
            state.month = view.month;
            state.day = day;
            send();
            if (!time) closePopup();
            else draw();
          });
          grid.append(cell);
        }
      }
      panel.append(grid);
    }
    if (time) {
      const clock = make('div', 'clock');
      const field = (/** @type {'hour' | 'minute'} */ name, /** @type {number} */ max) => {
        const box = document.createElement('input');
        box.type = 'number';
        box.min = '0';
        box.max = String(max);
        box.value = pad(state[name]);
        box.addEventListener('change', () => {
          state[name] = Math.max(0, Math.min(max, Number(box.value) || 0));
          box.value = pad(state[name]);
          send();
        });
        return box;
      };
      clock.append(field('hour', 23), make('span', '', ':'), field('minute', 59));
      panel.append(clock);
    }
    const foot = make('div', 'foot');
    const clear = make('button', '', 'Clear');
    const today = make('button', '', time && !date ? 'Now' : kind === 'month' ? 'This month' : 'Today');
    clear.addEventListener('click', () => {
      commit(input, () => (input.value = ''));
      closePopup();
    });
    today.addEventListener('click', () => {
      const now = new Date();
      Object.assign(state, { year: now.getFullYear(), month: now.getMonth(), day: now.getDate(), hour: now.getHours(), minute: now.getMinutes() });
      send();
      closePopup();
    });
    foot.append(clear, today);
    panel.append(foot);
  };

  draw();
  show(input, panel, () => false);
}

/**
 * Whether a press landed on the picker button at the right end of a date input.
 * @param {HTMLInputElement} input The input.
 * @param {MouseEvent} event The press.
 * @returns {boolean} Whether it did.
 */
function onButton(input, event) {
  const box = input.getBoundingClientRect();
  return event.clientX >= box.right - BUTTON_ZONE;
}

window.addEventListener(
  'mousedown',
  (event) => {
    const target = event.target;
    if (event.button !== 0 || !(target instanceof Element)) return;
    if (target instanceof HTMLSelectElement && !target.multiple && target.size <= 1 && !target.disabled) {
      event.preventDefault();
      target.focus();
      if (shown) closePopup();
      else showSelect(target);
    } else if (target instanceof HTMLInputElement && target.type in PICKERS && !target.readOnly && !target.disabled && onButton(target, event)) {
      event.preventDefault();
      target.focus();
      if (shown) closePopup();
      else showPicker(target);
    }
  },
  true,
);

window.addEventListener(
  'keydown',
  (event) => {
    const target = event.target;
    if (shown || !(target instanceof HTMLSelectElement) || target.multiple || target.size > 1) return;
    if (event.key === ' ' || event.key === 'F4' || (event.altKey && (event.key === 'ArrowDown' || event.key === 'ArrowUp'))) {
      event.preventDefault();
      showSelect(target);
    }
  },
  true,
);
