'use strict';

/**
 * `browser_run`: a whole script against pages in one tool call. The script is the body of an async function written
 * against a small page API in the style of browser automation libraries (`page.goto`, `page.click`, `page.fill`,
 * `page.waitForResponse`), and only what it returns, the checks it made and what it logged come back. Each step runs
 * the same code as the browser tool of the same name. `browser.newContext` opens pages in a fresh profile kept in
 * memory, with a device, locale and time zone of its own, headless by default and shown as tabs on request, at most
 * `limit` pages at once. Scripts can be saved as named recipes and run again by name, by an agent or from the pane.
 */

const fs = require('fs');
const path = require('path');
const vm = require('vm');
const emulation = require('./emulation');
const netview = require('./netview');
const partitions = require('./partitions');
const recording = require('./recording');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */
/** @typedef {{ session: string, name: string }} Caller */

/** How long a script may run when it names no time, in milliseconds. */
const DEFAULT_TIMEOUT = 60000;

/** The longest a script may run, in milliseconds. */
const MOST_TIMEOUT = 600000;

/** How many pages of new contexts may be open at once when the script names no limit. */
const DEFAULT_LIMIT = 4;

/** The most pages of new contexts a script may have open at once. */
const MOST_LIMIT = 8;

/** The most characters of the answer. */
const MOST_ANSWER = 20000;

/** The most lines a script may log. */
const MOST_LOGS = 200;

/** How long the window is given to open a tab for a context shown in the pane, in milliseconds. */
const TAB_OPENS = 10000;

/** The delay between keys when a script types like a person and names none, in milliseconds. */
const HUMAN_DELAY = 80;

/** The size of a headless page when its context names no device or viewport. */
const HEADLESS_SIZE = { width: 1280, height: 800 };

/**
 * What the helper gives the runner: how to make a page no tab shows, how to tell the window, and where recipes live.
 * @typedef {object} Host
 * @property {(partition: string, width: number, height: number) => Page} makePage Makes a page that no tab shows, in a profile.
 * @property {() => string} recipes The folder recipes are kept in.
 */

/** @type {Host | null} */
let host = null;

/**
 * The tabs a script asked the window to open, by the profile each must live in, waiting for the window to open them.
 * @type {Map<string, (page: Page) => void>}
 */
const awaitedTabs = new Map();

/** How many contexts were made, which names their profiles. */
let contextsMade = 0;

/**
 * Hands the runner what it needs of the helper.
 * @param {Host} given How to make pages and where recipes live.
 * @returns {void}
 */
function configure(given) {
  host = given;
}

/**
 * Tells the runner that the window opened a page, so that a script waiting for a tab in that page's profile gets it.
 * @param {Page} page The page the window opened.
 * @returns {void}
 */
function opened(page) {
  const waiting = awaitedTabs.get(page.partition);
  if (!waiting) return;
  awaitedTabs.delete(page.partition);
  waiting(page);
}

/**
 * The name a recipe is kept under: lower case letters, digits and dashes.
 * @param {unknown} name The name given.
 * @returns {string} The name made safe, or empty when nothing of it is left.
 */
function recipeName(name) {
  if (typeof name !== 'string') return '';
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 60);
}

/**
 * The file of a recipe.
 * @param {string} name The recipe's safe name.
 * @returns {string} The path.
 */
function recipeFile(name) {
  if (!host) throw new Error('the runner is not set up');
  return path.join(host.recipes(), `${name}.json`);
}

/**
 * Reads a saved recipe.
 * @param {string} name The recipe's safe name.
 * @returns {Json | null} The recipe, with `name`, `script`, `base_url` and `human`, or nothing when there is none.
 */
function loadRecipe(name) {
  try {
    const recipe = JSON.parse(fs.readFileSync(recipeFile(name), 'utf8'));
    return recipe && typeof recipe.script === 'string' ? recipe : null;
  } catch (_) {
    return null;
  }
}

/**
 * Saves a script as a recipe, replacing one of the same name.
 * @param {string} name The recipe's safe name.
 * @param {Json} args What `browser_run` was given: `script`, `base_url`, `human`, `limit` and `timeout_ms`.
 * @returns {void}
 * @throws {Error} When the file cannot be written.
 */
function saveRecipe(name, args) {
  const file = recipeFile(name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const recipe = { name, script: args.script, base_url: args.base_url, human: args.human, limit: args.limit, timeout_ms: args.timeout_ms, saved: Date.now() };
  fs.writeFileSync(file, JSON.stringify(recipe, null, 2));
}

/**
 * The saved recipes, newest first.
 * @returns {Json[]} Each recipe's `name`, `saved` time and first line.
 */
function listRecipes() {
  if (!host) return [];
  /** @type {string[]} */
  let files = [];
  try {
    files = fs.readdirSync(host.recipes()).filter((file) => file.endsWith('.json'));
  } catch (_) {
    return [];
  }
  return files
    .map((file) => loadRecipe(file.slice(0, -5)))
    .filter((recipe) => recipe !== null)
    .map((recipe) => ({ name: recipe.name, saved: recipe.saved, first: String(recipe.script).trim().split('\n')[0].slice(0, 120) }))
    .sort((a, b) => b.saved - a.saved);
}

/**
 * Waits for `ms` milliseconds.
 * @param {number} ms How long.
 * @returns {Promise<void>} Resolves when it has passed.
 */
function pause(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * A request the recorder kept, as plain data a script can test.
 * @param {import('./recorder').NetEntry} entry The request.
 * @param {boolean} withBody Whether to give the body of the response.
 * @returns {Json} Its `url`, `method`, `type`, `status` (0 while none came), `failure`, and with `withBody` its `body`.
 */
function requestData(entry, withBody) {
  /** @type {Json} */
  const data = { url: entry.url, method: entry.method, type: entry.type, status: entry.status || 0, failure: entry.failure || null, finished: entry.endedAt !== null };
  if (withBody) data.body = typeof entry.body === 'string' ? entry.body : null;
  return data;
}

/**
 * A limit on how many pages are open at once, which scripts that open many wait on.
 */
class Slots {
  /**
   * @param {number} most How many may be held at once.
   */
  constructor(most) {
    /** @type {number} How many may be held at once. */
    this.most = most;
    /** @type {number} How many are held. */
    this.held = 0;
    /** @type {Array<() => void>} Those waiting for one, first come first served. */
    this.waiting = [];
  }

  /**
   * Takes a slot, waiting until one is free.
   * @returns {Promise<void>} Resolves once one is held.
   */
  async take() {
    if (this.held < this.most) {
      this.held += 1;
      return;
    }
    await new Promise((resolve) => this.waiting.push(() => resolve(undefined)));
  }

  /**
   * Gives a slot back, handing it to the first that waits.
   * @returns {void}
   */
  give() {
    const next = this.waiting.shift();
    if (next) next();
    else this.held -= 1;
  }
}

/**
 * One run of a script: the pages and contexts it opened, the checks it made, what it logged, and how it reaches the tools.
 */
class Run {
  /**
   * @param {Page} base The page the call was made on, which the script knows as `page`.
   * @param {Json} options `base_url`, `human`, `limit` and whether it is `recorded`.
   * @param {Caller} caller The session that made the call.
   * @param {(page: Page, tool: string, args: Json, caller: Caller) => Promise<ToolOutcome>} tool Runs one browser tool on a page.
   */
  constructor(base, options, caller, tool) {
    /** @type {Page} The page the call was made on. */
    this.base = base;
    /** @type {string} The address relative addresses are taken from. */
    this.baseUrl = typeof options.base_url === 'string' ? options.base_url : '';
    /** @type {boolean} Whether clicks and typing are made like a person's unless a step says otherwise. */
    this.human = options.human === true;
    /** @type {Caller} The session that made the call. */
    this.caller = caller;
    /** @type {(page: Page, tool: string, args: Json, caller: Caller) => Promise<ToolOutcome>} Runs one browser tool on a page. */
    this.tool = tool;
    /** @type {Slots} The pages of new contexts open at once. */
    this.slots = new Slots(options.limit);
    /** @type {Set<ScriptContext>} The contexts the script made and has not closed. */
    this.contexts = new Set();
    /** @type {Array<{ name: string, ok: boolean, detail: string }>} The checks the script made, in order. */
    this.checks = [];
    /** @type {string[]} What the script logged. */
    this.logs = [];
    /** @type {number} How many steps ran. */
    this.steps = 0;
    /** @type {boolean} Whether the run is over, so that no new step starts. */
    this.over = false;
  }

  /**
   * An address the script gave, made whole against the base address.
   * @param {string} url The address, whole or relative.
   * @returns {string} The whole address.
   */
  resolve(url) {
    if (!this.baseUrl) return url;
    try {
      return new URL(url, this.baseUrl).href;
    } catch (_) {
      return url;
    }
  }

  /**
   * Runs one browser tool on a page, keeping it as a step of the session's recording when one is under way.
   * @param {Page} page The page.
   * @param {string} tool The tool.
   * @param {Json} args What to give it.
   * @returns {Promise<ToolOutcome>} What it came to; rejects with its text when it failed.
   */
  async step(page, tool, args) {
    if (this.over) throw new Error('the script ran out of time');
    this.steps += 1;
    page.restriction.start(tool);
    /** @type {ToolOutcome} */
    let outcome;
    try {
      outcome = await this.tool(page, tool, { ...args }, this.caller);
    } finally {
      page.restriction.end(tool);
    }
    const refused = page.restriction.takeRefusal();
    if (refused) outcome = { ok: false, text: refused, image: null };
    if (recording.recording(this.caller)) {
      const secret = tool === 'browser_type' ? [true] : [];
      await recording.afterTool(page, tool, JSON.stringify(args), outcome, this.caller, secret);
    }
    if (!outcome.ok) throw new Error(`${tool.replace(/^browser_/, '')}: ${outcome.text}`);
    return outcome;
  }

  /**
   * Makes a context: a fresh profile kept in memory, with its device, locale and time zone.
   * @param {Json} options `device`, `viewport` ({ width, height }), `locale`, `timezone`, `userAgent` and `headless` (true when not given).
   * @returns {ScriptContext} The context.
   */
  newContext(options) {
    const given = options && typeof options === 'object' ? options : {};
    const wanted = emulation.resolve(
      {
        device: given.device,
        width: given.viewport && given.viewport.width,
        height: given.viewport && given.viewport.height,
        user_agent: given.userAgent,
      },
      { viewport: null, userAgent: null },
      HEADLESS_SIZE,
    );
    if ('error' in wanted) throw new Error(`newContext: ${wanted.error}`);
    contextsMade += 1;
    const partition = `context-${process.pid}-${contextsMade}`;
    const context = new ScriptContext(this, partition, {
      viewport: wanted.viewport,
      userAgent: wanted.userAgent,
      locale: typeof given.locale === 'string' ? given.locale : '',
      timezone: typeof given.timezone === 'string' ? given.timezone : '',
      headless: given.headless !== false,
    });
    this.contexts.add(context);
    return context;
  }

  /**
   * Closes every context the script left open.
   * @returns {Promise<void>} Resolves once they are closed.
   */
  async closeAll() {
    await Promise.all([...this.contexts].map((context) => context.close()));
  }
}

/**
 * A context of a script: pages that share a fresh profile no other page sees, which is emptied when the context closes.
 */
class ScriptContext {
  /**
   * @param {Run} run The run it belongs to.
   * @param {string} partition The engine's name for its profile, kept in memory.
   * @param {{ viewport: import('./emulation').Emulated | null, userAgent: string | null, locale: string, timezone: string, headless: boolean }} settings What its pages emulate, and whether they are shown.
   */
  constructor(run, partition, settings) {
    /** @type {Run} */
    this.run = run;
    /** @type {string} */
    this.partition = partition;
    /** @type {typeof settings} */
    this.settings = settings;
    /** @type {Set<ScriptPage>} The pages open in it. */
    this.pages = new Set();
    /** @type {boolean} */
    this.closed = false;
  }

  /**
   * Opens a page in the context, once fewer than the run's limit of pages are open: a page no tab shows, or when the
   * context is not headless, a tab the window opens behind the one shown.
   * @returns {Promise<ScriptPage>} The page, blank, with the context's device, locale and time zone set.
   */
  async newPage() {
    if (this.closed) throw new Error('newPage: the context is closed');
    if (!host) throw new Error('the runner is not set up');
    await this.run.slots.take();
    /** @type {Page} */
    let page;
    try {
      page = this.settings.headless ? await this.headlessPage() : await this.shownPage();
      await this.emulate(page);
    } catch (error) {
      this.run.slots.give();
      throw error;
    }
    const made = new ScriptPage(this.run, page, this);
    this.pages.add(made);
    return made;
  }

  /**
   * Makes a page no tab shows and waits for its blank document.
   * @returns {Promise<Page>} The page.
   */
  async headlessPage() {
    const size = this.settings.viewport || HEADLESS_SIZE;
    const page = /** @type {Host} */ (host).makePage(this.partition, size.width, size.height);
    await new Promise((resolve) => {
      page.contents.once('did-stop-loading', () => resolve(undefined));
      page.load('about:blank');
    });
    return page;
  }

  /**
   * Asks the window for a tab in the context's profile, as a page asks for a new window, and waits for it.
   * @returns {Promise<Page>} The tab's page; rejects when the window opened none in time.
   */
  shownPage() {
    const { base } = this.run;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        awaitedTabs.delete(this.partition);
        reject(new Error('newPage: the window opened no tab for the context; the strip may be full, or the window may be too old for contexts shown in the pane'));
      }, TAB_OPENS);
      awaitedTabs.set(this.partition, (page) => {
        clearTimeout(timer);
        const ready = () => resolve(page);
        if (page.cdp && !page.contents.isLoading()) ready();
        else page.contents.once('did-stop-loading', ready);
      });
      base.send({ OpenRequested: { page: base.id, url: 'about:blank', agent: true, partition: this.partition } });
    });
  }

  /**
   * Gives a page the context's device, user agent, languages and time zone.
   * @param {Page} page The page.
   * @returns {Promise<void>} Resolves once the engine has taken them.
   */
  async emulate(page) {
    const { viewport, userAgent, locale, timezone } = this.settings;
    if (viewport) await page.emulateViewport(viewport);
    const cdp = page.cdp;
    if (!cdp) throw new Error('newPage: the page has no protocol');
    if (userAgent || locale) {
      const agent = userAgent || page.contents.getUserAgent();
      await cdp.call('Emulation.setUserAgentOverride', locale ? { userAgent: agent, acceptLanguage: locale } : { userAgent: agent });
      page.userAgent = agent;
    }
    if (locale) await cdp.call('Emulation.setLocaleOverride', { locale }).catch((error) => Promise.reject(new Error(`newContext: locale ${locale}: ${error.message}`)));
    if (timezone) await cdp.call('Emulation.setTimezoneOverride', { timezoneId: timezone }).catch((error) => Promise.reject(new Error(`newContext: timezone ${timezone}: ${error.message}`)));
  }

  /**
   * Closes the pages of the context and empties its profile.
   * @returns {Promise<void>} Resolves once it is done.
   */
  async close() {
    if (this.closed) return;
    this.closed = true;
    await Promise.all([...this.pages].map((page) => page.close()));
    this.run.contexts.delete(this);
    await partitions.release(this.partition, []);
  }
}

/**
 * A page as a script sees it: each method runs the browser tool of the same name and rejects with its text when it fails.
 */
class ScriptPage {
  /**
   * @param {Run} run The run it belongs to.
   * @param {Page} page The helper's page.
   * @param {ScriptContext | null} context The context that opened it, or nothing for the page of the call.
   */
  constructor(run, page, context) {
    /** @type {Run} */
    this.run = run;
    /** @type {Page} */
    this.page = page;
    /** @type {ScriptContext | null} */
    this.context = context;
    /** @type {boolean} */
    this.closed = false;
  }

  /**
   * Runs a browser tool on the page.
   * @param {string} tool The tool.
   * @param {Json} args What to give it.
   * @returns {Promise<ToolOutcome>} What it came to.
   */
  step(tool, args) {
    if (this.closed) return Promise.reject(new Error(`${tool.replace(/^browser_/, '')}: the page is closed`));
    return this.run.step(this.page, tool, args);
  }

  /**
   * The arguments that name an element: a CSS selector, or `{ ref }` from a snapshot.
   * @param {unknown} element A selector, or an object with `ref` or `selector`.
   * @returns {Json} `selector` or `ref`.
   */
  static element(element) {
    if (typeof element === 'string') return { selector: element };
    if (element && typeof element === 'object') return /** @type {Json} */ (element);
    throw new Error('name an element with a CSS selector');
  }

  /**
   * Opens an address, relative to the run's `base_url` when it is not whole, and waits for the page to load and settle.
   * @param {string} url The address.
   * @returns {Promise<string>} What the browser says of the page that loaded.
   */
  async goto(url) {
    return (await this.step('browser_navigate', { url: this.run.resolve(String(url)) })).text;
  }

  /**
   * Clicks an element, or a point, and waits for what the click started.
   * @param {unknown} element A selector, `{ ref }`, or `{ x, y }`.
   * @param {Json} [options] `human` to glide the pointer and scroll smoothly as a person does; the run's `human` when not given.
   * @returns {Promise<string>} What was clicked.
   */
  async click(element, options = {}) {
    const human = options.human === undefined ? this.run.human : options.human === true;
    return (await this.step('browser_click', { ...ScriptPage.element(element), human })).text;
  }

  /**
   * Puts text into a field at once, replacing what it held.
   * @param {unknown} element A selector or `{ ref }`.
   * @param {string} text The text.
   * @returns {Promise<void>} Resolves once it is in.
   */
  async fill(element, text) {
    await this.step('browser_type', { ...ScriptPage.element(element), text: String(text) });
  }

  /**
   * Types text into a field key by key, replacing what it held.
   * @param {unknown} element A selector or `{ ref }`.
   * @param {string} text The text.
   * @param {Json} [options] `delay` between keys in milliseconds (80 when not given), and `submit` to press Enter after.
   * @returns {Promise<void>} Resolves once it is typed.
   */
  async type(element, text, options = {}) {
    const delay = Number.isFinite(options.delay) ? Number(options.delay) : HUMAN_DELAY;
    await this.step('browser_type', { ...ScriptPage.element(element), text: String(text), delay_ms: Math.max(delay, 1), submit: options.submit === true });
  }

  /**
   * Presses a key in the page.
   * @param {string} key The key, such as `Enter` or `a`.
   * @param {Json} [options] `modifiers`: some of `shift`, `control`, `alt` and `meta`.
   * @returns {Promise<void>} Resolves once it is pressed.
   */
  async press(key, options = {}) {
    await this.step('browser_press_key', { key: String(key), modifiers: options.modifiers });
  }

  /**
   * Chooses an option of a list.
   * @param {unknown} element A selector or `{ ref }`.
   * @param {string} value The option's value or text.
   * @returns {Promise<void>} Resolves once it is chosen.
   */
  async select(element, value) {
    await this.step('browser_select_option', { ...ScriptPage.element(element), value: String(value) });
  }

  /**
   * Ticks or clears a checkbox, or chooses a radio button.
   * @param {unknown} element A selector or `{ ref }`.
   * @param {boolean} [on] Whether it should end up ticked; true when not given.
   * @returns {Promise<void>} Resolves once it is so.
   */
  async check(element, on = true) {
    await this.step('browser_fill_form', { fields: [{ ...ScriptPage.element(element), value: on !== false }] });
  }

  /**
   * Moves the pointer over an element.
   * @param {unknown} element A selector, `{ ref }`, or `{ x, y }`.
   * @returns {Promise<void>} Resolves once the page had time to react.
   */
  async hover(element) {
    await this.step('browser_hover', ScriptPage.element(element));
  }

  /**
   * Waits until an element is on the page, or a text is.
   * @param {string | Json} what A selector, or `{ text }`.
   * @param {Json} [options] `timeout` in milliseconds, 10000 when not given.
   * @returns {Promise<void>} Resolves once it is there; rejects when it did not come in time.
   */
  async waitFor(what, options = {}) {
    const args = typeof what === 'string' ? { selector: what } : { ...what };
    await this.step('browser_wait', { ...args, timeout_ms: options.timeout });
  }

  /**
   * Waits for a request whose address matches to finish: one that finished since the step before started, or the next.
   * @param {string} pattern Text the address holds, or a pattern over the whole address with `*` for any run of characters.
   * @param {Json} [options] `timeout` in milliseconds (10000 when not given) and `type`, such as `fetch` or `xhr,fetch`.
   * @returns {Promise<Json>} The request: `url`, `method`, `type`, `status`, `failure`, `finished` and `body`.
   */
  async waitForResponse(pattern, options = {}) {
    const recorder = this.page.recorder;
    if (!recorder) throw new Error('waitForResponse: the page keeps no network');
    const timeout = Math.min(Number.isFinite(options.timeout) ? Number(options.timeout) : 10000, MOST_TIMEOUT);
    const types = typeof options.type === 'string' ? netview.parseTypes(options.type) : null;
    if (types && 'error' in types) throw new Error(`waitForResponse: ${types.error}`);
    const entry = await recorder.waitFor(String(pattern), types, timeout);
    if (!entry) throw new Error(`waitForResponse: no request matching ${pattern} finished within ${timeout} ms`);
    return requestData(entry, true);
  }

  /**
   * The requests the page made that the recorder still holds, oldest first.
   * @param {string} [pattern] Keeps only those whose address matches; all when not given.
   * @returns {Json[]} Each request's `url`, `method`, `type`, `status`, `failure` and `finished`.
   */
  requests(pattern) {
    const recorder = this.page.recorder;
    if (!recorder) return [];
    return recorder.network.filter((entry) => !pattern || netview.urlMatches(entry.url, String(pattern))).map((entry) => requestData(entry, false));
  }

  /**
   * Counts the requests whose address matches, by the status they were answered with.
   * @param {string} [pattern] Keeps only those whose address matches; all when not given.
   * @returns {Json} `total`, `statuses` (each status as text, such as `"202"`, to how many had it; `"0"` for none yet) and `failed`.
   */
  count(pattern) {
    /** @type {Record<string, number>} */
    const statuses = {};
    let failed = 0;
    const found = this.requests(pattern);
    for (const request of found) {
      statuses[String(request.status)] = (statuses[String(request.status)] || 0) + 1;
      if (request.failure) failed += 1;
    }
    return { total: found.length, statuses, failed };
  }

  /**
   * Runs a function or an expression in the page and gives its value.
   * @param {Function | string} code A function, called with `arg`, or an expression.
   * @param {unknown} [arg] What to call the function with; it must be JSON.
   * @returns {Promise<unknown>} The value, as JSON gives it back.
   */
  async evaluate(code, arg) {
    const expression = typeof code === 'function' ? `(${code.toString()})(${JSON.stringify(arg === undefined ? null : arg)})` : String(code);
    return JSON.parse((await this.step('browser_evaluate', { expression })).text);
  }

  /**
   * The text an element shows.
   * @param {string} selector A CSS selector.
   * @returns {Promise<string | null>} Its text, or nothing when there is no such element.
   */
  text(selector) {
    return /** @type {Promise<string | null>} */ (this.evaluate(`(() => { const el = document.querySelector(${JSON.stringify(String(selector))}); return el ? el.innerText : null; })()`));
  }

  /**
   * The page's address.
   * @returns {string} The address.
   */
  url() {
    return this.page.contents.getURL();
  }

  /**
   * The page's title.
   * @returns {string} The title.
   */
  title() {
    return this.page.contents.getTitle();
  }

  /**
   * Takes a picture of the view, of the whole page, or of one element.
   * @param {Json} [options] `fullPage`, or `selector`.
   * @returns {Promise<string | null>} Where the picture was saved.
   */
  async screenshot(options = {}) {
    const outcome = await this.step('browser_screenshot', { full_page: options.fullPage === true, selector: options.selector });
    return outcome.image;
  }

  /**
   * Lists what can be clicked or typed in, numbered for `{ ref }`, and the page's text.
   * @returns {Promise<string>} The snapshot.
   */
  async snapshot() {
    return (await this.step('browser_snapshot', {})).text;
  }

  /**
   * Closes a page of a context: a headless one at once, a tab by asking the window to close it. The page of the call cannot be closed.
   * @returns {Promise<void>} Resolves once it is closed.
   */
  async close() {
    if (this.closed || !this.context) return;
    this.closed = true;
    this.context.pages.delete(this);
    if (this.context.settings.headless) this.page.close();
    else this.page.send({ PageClosed: { page: this.page.id } });
    this.run.slots.give();
  }
}

/**
 * Runs `work` on each item with at most `limit` running at once.
 * @param {unknown[]} items The items.
 * @param {(item: unknown, index: number) => Promise<unknown>} work What to do with each.
 * @param {number} limit How many at once.
 * @returns {Promise<Json[]>} For each item, `{ ok, value }` or `{ ok: false, error }`, in the order of the items.
 */
async function parallel(items, work, limit) {
  /** @type {Json[]} */
  const results = new Array(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const index = next;
      next += 1;
      try {
        results[index] = { ok: true, value: await work(items[index], index) };
      } catch (error) {
        results[index] = { ok: false, error: error instanceof Error ? error.message : String(error) };
      }
    }
  };
  await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, worker));
  return results;
}

/**
 * A value made plain JSON, as the answer carries it.
 * @param {unknown} value The value.
 * @returns {unknown} It, through JSON; nothing when it is not JSON.
 */
function plain(value) {
  if (value === undefined) return null;
  try {
    return JSON.parse(JSON.stringify(value));
  } catch (_) {
    return String(value);
  }
}

/**
 * The script a call runs, with what to run it with: the script given, or a recipe by name, whose settings the call can override.
 * @param {Json} args What `browser_run` was given.
 * @returns {{ script: string, options: Json, recipe: string } | { error: string }} The script and its options, or why there is none.
 */
function chosen(args) {
  if (typeof args.recipe === 'string') {
    const name = recipeName(args.recipe);
    const recipe = name ? loadRecipe(name) : null;
    if (!recipe) {
      const names = listRecipes().map((held) => held.name);
      return { error: `there is no recipe ${args.recipe}; saved recipes: ${names.length ? names.join(', ') : 'none'}` };
    }
    /** @type {Json} */
    const options = { ...recipe };
    for (const key of ['base_url', 'human', 'limit', 'timeout_ms', 'record']) if (args[key] !== undefined) options[key] = args[key];
    return { script: recipe.script, options, recipe: name };
  }
  if (typeof args.script !== 'string' || !args.script.trim()) return { error: 'give script, the body of an async function, or recipe, the name of a saved one' };
  return { script: args.script, options: args, recipe: '' };
}

/**
 * `browser_run`: runs a script against the page of the call and the contexts it opens, and answers with JSON.
 * @param {Page} base The page the call was made on.
 * @param {Json} args `script` or `recipe`, `save`, `list`, `base_url`, `human`, `limit`, `timeout_ms` and `record`.
 * @param {Caller} caller The session that made the call.
 * @param {(page: Page, tool: string, args: Json, caller: Caller) => Promise<ToolOutcome>} tool Runs one browser tool on a page.
 * @returns {Promise<ToolOutcome>} The answer: JSON with `ok`, `value`, `checks`, `passed`, `failed`, `logs`, `error`, `steps`, `ms` and `recording`.
 */
async function runTool(base, args, caller, tool) {
  if (args.list === true) return { ok: true, text: JSON.stringify({ recipes: listRecipes() }), image: null };
  const picked = chosen(args);
  if ('error' in picked) return { ok: false, text: picked.error, image: null };
  const { script, options } = picked;
  if (typeof args.save === 'string') {
    const name = recipeName(args.save);
    if (!name) return { ok: false, text: 'give save as a name of letters, digits and dashes', image: null };
    try {
      saveRecipe(name, { ...options, script });
    } catch (error) {
      return { ok: false, text: `the recipe could not be saved: ${error instanceof Error ? error.message : error}`, image: null };
    }
  }
  const limit = Number.isInteger(options.limit) ? Math.min(Math.max(options.limit, 1), MOST_LIMIT) : DEFAULT_LIMIT;
  const timeout = Number.isFinite(options.timeout_ms) ? Math.min(Math.max(Number(options.timeout_ms), 1000), MOST_TIMEOUT) : DEFAULT_TIMEOUT;
  const run = new Run(base, { ...options, limit }, caller, tool);
  /** @type {string | null} */
  let recordedInto = null;
  if (options.record === true && !recording.recording(caller)) {
    const started = recording.recordTool({ action: 'start' }, undefined, caller);
    if (!started.ok) return started;
    recordedInto = started.text.replace(/^Recording into (.*?)\. .*$/s, '$1');
  }
  const started = Date.now();
  /** @type {unknown} */
  let value = null;
  /** @type {string | null} */
  let error = null;
  /** @type {NodeJS.Timeout | undefined} */
  let timer;
  try {
    const sandbox = vm.createContext({
      page: new ScriptPage(run, base, null),
      browser: { newContext: (/** @type {Json} */ given) => run.newContext(given) },
      check: (/** @type {unknown} */ name, /** @type {unknown} */ ok, /** @type {unknown} */ detail) => {
        run.checks.push({ name: String(name), ok: Boolean(ok), detail: detail === undefined ? '' : typeof detail === 'string' ? detail : JSON.stringify(plain(detail)) });
        return Boolean(ok);
      },
      log: (/** @type {unknown[]} */ ...parts) => {
        if (run.logs.length < MOST_LOGS) run.logs.push(parts.map((part) => (typeof part === 'string' ? part : JSON.stringify(plain(part)))).join(' '));
      },
      sleep: (/** @type {number} */ ms) => pause(Math.min(Math.max(Number(ms) || 0, 0), MOST_TIMEOUT)),
      parallel: (/** @type {unknown[]} */ items, /** @type {(item: unknown, index: number) => Promise<unknown>} */ work, /** @type {number} */ most) =>
        parallel(Array.from(items), work, Number.isInteger(most) ? Math.min(Math.max(most, 1), MOST_LIMIT) : limit),
      URL,
      JSON,
      Math,
      Date,
      console: { log: (/** @type {unknown[]} */ ...parts) => run.logs.length < MOST_LOGS && run.logs.push(parts.map(String).join(' ')) },
    });
    const body = vm.runInContext(`(async () => {\n${script}\n})()`, sandbox, { timeout: Math.min(timeout, 5000), filename: 'browser_run.js' });
    const late = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`the script ran longer than ${timeout} ms`)), timeout);
    });
    value = plain(await Promise.race([body, late]));
  } catch (thrown) {
    error = thrown instanceof Error ? thrown.message : String(thrown);
  } finally {
    clearTimeout(timer);
    run.over = true;
    await run.closeAll();
  }
  if (recordedInto) recording.recordTool({ action: 'stop' }, undefined, caller);
  const failedChecks = run.checks.filter((held) => !held.ok).length;
  const ok = error === null && failedChecks === 0;
  const answer = {
    ok,
    value,
    error,
    checks: run.checks,
    passed: run.checks.length - failedChecks,
    failed: failedChecks,
    logs: run.logs,
    steps: run.steps,
    ms: Date.now() - started,
    recipe: picked.recipe || (typeof args.save === 'string' ? recipeName(args.save) : null),
    recording: recordedInto,
  };
  let text = JSON.stringify(answer);
  if (text.length > MOST_ANSWER) text = JSON.stringify({ ...answer, value: `[the value was cut: ${text.length} characters]`, logs: answer.logs.slice(-20) });
  return { ok, text, image: null };
}

module.exports = { configure, opened, runTool, listRecipes, recipeName, parallel, Slots };
