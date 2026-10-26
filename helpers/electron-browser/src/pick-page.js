/**
 * The script that runs inside a page, in a scripting world the page cannot see, while the reader
 * picks an element: it outlines the element under the pointer and, on a click, reports the element
 * through `__pmPicked`, a function that world is given and the page is not. `src/pick.js` injects
 * it; it is not loaded as a module and is left out of the type check, which has no page to check
 * it against.
 *
 * Everything the page can touch is untrusted: only events the reader made (`isTrusted`) count.
 */
(() => {
  if (window.__pmPick) {
    window.__pmPick.stop(false);
  }
  /** @type {HTMLDivElement} The outline drawn over the element under the pointer. */
  const box = document.createElement('div');
  box.style.cssText =
    'position:fixed;z-index:2147483647;pointer-events:none;box-sizing:border-box;' +
    'border:2px solid #4c8bf5;background:rgba(76,139,245,.18);left:0;top:0;width:0;height:0;';
  document.documentElement.appendChild(box);
  /** @type {string} How the pointer looked before the mode began. */
  const before = document.documentElement.style.cursor;
  document.documentElement.style.cursor = 'crosshair';
  /** @type {Element | null} The element under the pointer. */
  let target = null;

  /**
   * A selector that finds an element, from the nearest id or a few levels up.
   * @param {Element} el The element.
   * @returns {string} The selector.
   */
  const path = (el) => {
    const parts = [];
    for (let node = el; node && node.nodeType === 1 && parts.length < 6; node = node.parentElement) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        parts.unshift(part + '#' + CSS.escape(node.id));
        break;
      }
      const classes = Array.from(node.classList).slice(0, 2).map((c) => '.' + CSS.escape(c)).join('');
      part += classes;
      const parent = node.parentElement;
      if (parent) {
        const same = Array.from(parent.children).filter((c) => c.tagName === node.tagName);
        if (same.length > 1) {
          part += ':nth-of-type(' + (same.indexOf(node) + 1) + ')';
        }
      }
      parts.unshift(part);
    }
    return parts.join(' > ');
  };

  /**
   * What the reader is told about an element.
   * @param {Element} el The element.
   * @returns {Record<string, unknown>} Its selector, text, markup, box and a few styles.
   */
  const describe = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    const keys = ['color', 'background-color', 'font-family', 'font-size', 'font-weight', 'display', 'position', 'margin', 'padding', 'border'];
    return {
      selector: path(el),
      tag: el.tagName.toLowerCase(),
      id: el.id || '',
      classes: Array.from(el.classList),
      text: ((/** @type {HTMLElement} */ (el)).innerText || el.textContent || '').trim().slice(0, 400),
      html: el.outerHTML.slice(0, 2000),
      x: r.x, y: r.y, width: r.width, height: r.height,
      scroll_x: window.scrollX, scroll_y: window.scrollY,
      styles: keys.map((k) => [k, s.getPropertyValue(k)]),
      url: location.href,
      title: document.title,
    };
  };

  /**
   * Outlines the element under the pointer.
   * @param {MouseEvent} e The pointer moving.
   * @returns {void}
   */
  const move = (e) => {
    const el = /** @type {Element | null} */ (e.target);
    if (!el || el === box) return;
    target = el;
    const r = el.getBoundingClientRect();
    box.style.left = r.left + 'px';
    box.style.top = r.top + 'px';
    box.style.width = r.width + 'px';
    box.style.height = r.height + 'px';
  };

  /**
   * Keeps an event from reaching the page.
   * @param {Event} e The event.
   * @returns {void}
   */
  const swallow = (e) => {
    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();
  };

  /**
   * Picks the element that was clicked, and reports it.
   * @param {MouseEvent} e The click.
   * @returns {void}
   */
  const click = (e) => {
    if (!e.isTrusted) return;
    swallow(e);
    const el = target || /** @type {Element} */ (e.target);
    stop(true);
    __pmPicked(JSON.stringify({ element: describe(el) }));
  };

  /**
   * Ends the mode, reporting that nothing was picked, when the reader presses Escape.
   * @param {KeyboardEvent} e The key.
   * @returns {void}
   */
  const key = (e) => {
    if (e.isTrusted && e.key === 'Escape') {
      swallow(e);
      stop(true);
      __pmPicked(JSON.stringify({}));
    }
  };

  /** The events the page must not see while the mode is on. */
  const blocked = ['mousedown', 'mouseup', 'pointerdown', 'pointerup', 'dblclick', 'contextmenu', 'auxclick'];

  /**
   * Turns the mode off and puts the page back as it was.
   * @param {boolean} report Whether the end is to be reported; the caller does that.
   * @returns {void}
   */
  function stop(report) {
    document.removeEventListener('mousemove', move, true);
    document.removeEventListener('click', click, true);
    document.removeEventListener('keydown', key, true);
    blocked.forEach((n) => document.removeEventListener(n, swallow, true));
    box.remove();
    document.documentElement.style.cursor = before;
    delete window.__pmPick;
  }
  document.addEventListener('mousemove', move, true);
  document.addEventListener('click', click, true);
  document.addEventListener('keydown', key, true);
  blocked.forEach((n) => document.addEventListener(n, swallow, true));
  window.__pmPick = { stop };
})();
