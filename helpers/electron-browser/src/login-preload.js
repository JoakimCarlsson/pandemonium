'use strict';

/**
 * Login forms of pages, for the helper's keeping of logins (`src/passwords.js`). It runs in the isolated world of the
 * top frame of every page of the web, never in sealed pages, and exposes nothing to the page. It tells the helper when
 * the reader sends a form with a password, by submitting it, pressing Enter in its password or pressing one of its
 * buttons; when a field of a login form takes or leaves the keyboard, with where the field is; and it fills the form
 * when the helper sends a login the reader picked.
 */

const { ipcRenderer } = require('electron');

/** The input types a user name is typed in. */
const NAME_TYPES = new Set(['text', 'email', 'tel', '']);

/**
 * Whether an element is a field the reader can see and type in.
 * @param {Element | null} element The element.
 * @returns {element is HTMLInputElement} Whether it is an input that is shown, enabled and not read-only.
 */
function usable(element) {
  if (!(element instanceof HTMLInputElement) || element.disabled || element.readOnly) return false;
  const box = element.getBoundingClientRect();
  return box.width > 0 && box.height > 0;
}

/**
 * The fields of the login form around an element: its form, or the whole document when it is in none.
 * @param {Element} around An element of the form.
 * @returns {{ username: HTMLInputElement | null, password: HTMLInputElement | null }} The field for the user name, when there is one, and the first password field.
 */
function loginFields(around) {
  const scope = (around instanceof HTMLInputElement && around.form) || around.closest('form') || document;
  const inputs = Array.from(scope.querySelectorAll('input')).filter(usable);
  const password = inputs.find((input) => input.type === 'password') || null;
  if (!password) return { username: null, password: null };
  const named = inputs.find((input) => /\b(username|email)\b/.test(input.autocomplete));
  const before = inputs.slice(0, inputs.indexOf(password)).filter((input) => NAME_TYPES.has(input.type));
  return { username: named || before[before.length - 1] || null, password };
}

/**
 * Tells the helper the reader sent the login of the form around an element, when it holds a password.
 * @param {Element} around An element of the form.
 * @returns {void}
 */
function sent(around) {
  const { username, password } = loginFields(around);
  if (!password || !password.value) return;
  ipcRenderer.send('pandemonium:login-sent', username ? username.value : '', password.value);
}

/**
 * Whether an element is a field of a login form.
 * @param {EventTarget | null} element The element.
 * @returns {element is HTMLInputElement} Whether it is the password or the user name of a form with a password.
 */
function loginField(element) {
  if (!usable(element instanceof Element ? element : null)) return false;
  const input = /** @type {HTMLInputElement} */ (element);
  if (input.type === 'password') return true;
  const { username } = loginFields(input);
  return username === input;
}

/**
 * Tells the helper a field of a login form holds the keyboard, and where it is in the page's viewport.
 * @param {HTMLInputElement} field The field.
 * @returns {void}
 */
function focused(field) {
  const box = field.getBoundingClientRect();
  ipcRenderer.send('pandemonium:login-focus', { x: box.left, y: box.top, width: box.width, height: box.height });
}

/**
 * Sets the value of a field as typing would, so the page's own handlers see the change.
 * @param {HTMLInputElement} field The field.
 * @param {string} value What it holds.
 * @returns {void}
 */
function setValue(field, value) {
  field.value = value;
  field.dispatchEvent(new Event('input', { bubbles: true }));
  field.dispatchEvent(new Event('change', { bubbles: true }));
}

/**
 * Fills the login form that holds the keyboard, or the first one of the page, with a login, when the page is still on
 * the origin it was picked for.
 * @param {unknown} origin The origin the login is for.
 * @param {unknown} username The user name.
 * @param {unknown} password The password.
 * @returns {void}
 */
function fill(origin, username, password) {
  if (location.origin !== origin) return;
  const active = document.activeElement;
  const around = active && active !== document.body ? active : document.querySelector('input[type=password]');
  if (!around) return;
  const fields = loginFields(around);
  if (!fields.password) return;
  if (fields.username && username) setValue(fields.username, String(username));
  setValue(fields.password, String(password ?? ''));
}

/**
 * Starts watching the login forms of the page.
 * @returns {void}
 */
function watchLogins() {
  /** Whether the helper was told a login field holds the keyboard. */
  let told = false;
  window.addEventListener(
    'submit',
    (event) => {
      if (event.target instanceof Element) sent(event.target);
    },
    true,
  );
  window.addEventListener(
    'keydown',
    (event) => {
      if (event.key === 'Enter' && event.target instanceof HTMLInputElement && loginField(event.target)) sent(event.target);
    },
    true,
  );
  window.addEventListener(
    'click',
    (event) => {
      const button = event.target instanceof Element ? event.target.closest('button, input[type=submit], input[type=button], [role=button]') : null;
      if (button) sent(button);
    },
    true,
  );
  window.addEventListener(
    'focusin',
    (event) => {
      if (loginField(event.target)) {
        const field = /** @type {HTMLInputElement} */ (event.target);
        told = true;
        requestAnimationFrame(() => {
          if (told && document.activeElement === field) focused(field);
        });
      } else if (told) {
        told = false;
        ipcRenderer.send('pandemonium:login-left');
      }
    },
    true,
  );
  window.addEventListener(
    'focusout',
    () => {
      setTimeout(() => {
        if (!told || !document.hasFocus() || loginField(document.activeElement)) return;
        told = false;
        ipcRenderer.send('pandemonium:login-left');
      }, 0);
    },
    true,
  );
  window.addEventListener(
    'scroll',
    () => {
      if (!told) return;
      told = false;
      ipcRenderer.send('pandemonium:login-left');
    },
    true,
  );
  ipcRenderer.on('pandemonium:login-fill', (_event, origin, username, password) => fill(origin, username, password));
}

if (window.top === window && (location.protocol === 'http:' || location.protocol === 'https:')) watchLogins();
