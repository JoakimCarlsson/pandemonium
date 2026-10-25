(() => {
  const nonce = '__NONCE__';
  if (window.__pmPick) {
    window.__pmPick.stop(false);
  }
  const box = document.createElement('div');
  box.style.cssText =
    'position:fixed;z-index:2147483647;pointer-events:none;box-sizing:border-box;' +
    'border:2px solid #4c8bf5;background:rgba(76,139,245,.18);left:0;top:0;width:0;height:0;';
  document.documentElement.appendChild(box);
  const before = document.documentElement.style.cursor;
  document.documentElement.style.cursor = 'crosshair';
  let target = null;
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
  const describe = (el) => {
    const r = el.getBoundingClientRect();
    const s = getComputedStyle(el);
    const keys = ['color', 'background-color', 'font-family', 'font-size', 'font-weight', 'display', 'position', 'margin', 'padding', 'border'];
    return {
      selector: path(el),
      tag: el.tagName.toLowerCase(),
      id: el.id || '',
      classes: Array.from(el.classList),
      text: (el.innerText || el.textContent || '').trim().slice(0, 400),
      html: el.outerHTML.slice(0, 2000),
      x: r.x, y: r.y, width: r.width, height: r.height,
      styles: keys.map((k) => [k, s.getPropertyValue(k)]),
      url: location.href,
      title: document.title,
    };
  };
  const move = (e) => {
    const el = e.target;
    if (!el || el === box) return;
    target = el;
    const r = el.getBoundingClientRect();
    box.style.left = r.left + 'px';
    box.style.top = r.top + 'px';
    box.style.width = r.width + 'px';
    box.style.height = r.height + 'px';
  };
  const swallow = (e) => {
    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();
  };
  const click = (e) => {
    swallow(e);
    const el = target || e.target;
    stop(true);
    console.log('__pm_pick__' + nonce + JSON.stringify(describe(el)));
  };
  const key = (e) => {
    if (e.key === 'Escape') {
      swallow(e);
      stop(true);
      console.log('__pm_pick_cancel__' + nonce);
    }
  };
  const blocked = ['mousedown', 'mouseup', 'pointerdown', 'pointerup', 'dblclick', 'contextmenu', 'auxclick'];
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
