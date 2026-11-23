'use strict';

/**
 * Recordings of what agents do with the browser, one per agent session: `browser_record` starts and stops the
 * recording of the session that calls it, and while one is under way every browser tool call of that session is kept
 * as a step, with what it was given, what it answered and a screenshot taken after it, in a folder of its own with a
 * readable summary. What is typed into password fields and arguments named like secrets are never kept, and password
 * fields are blurred in the screenshots.
 */

const fs = require('fs');
const path = require('path');
const secrets = require('./secrets');

/** @typedef {import('./page').Page} Page */
/** @typedef {import('./types').Json} Json */
/** @typedef {import('./types').ToolOutcome} ToolOutcome */
/** @typedef {import('./types').RecordedStep} RecordedStep */

/** The most steps one recording keeps; the recording stops itself past it. */
const MOST_STEPS = 500;

/** The most characters of a tool's arguments a step keeps. */
const MOST_ARGUMENTS = 2000;

/** The most characters of a tool's answer a step keeps. */
const MOST_ANSWER = 600;

/** How long the screenshot after a step may take, in milliseconds. */
const SHOT_TIME = 5000;

/** The tools that are never recorded as steps. */
const UNRECORDED = new Set(['browser_record']);

/**
 * A recording under way.
 * @typedef {object} Recording
 * @property {string} folder Where its files go.
 * @property {number} started When it started, in milliseconds since the epoch.
 * @property {string} session The key of the agent session it records, as the window names it; empty when the window names none.
 * @property {string} name What the session is called, for the summary.
 * @property {RecordedStep[]} steps Its steps, in the order they ended.
 * @property {number} taken How many step numbers were handed out, which calls running side by side never share.
 */

/**
 * Who called a tool: the agent session the window says made the call.
 * @typedef {object} Caller
 * @property {string} session The session's key, unique in the window; empty when the window names none.
 * @property {string} name What the session is called.
 */

/** @type {Map<string, Recording>} The recordings under way, by the key of the session each records. */
const active = new Map();

/** What stands in for a secret in a recorded step. */
const REDACTED = '[redacted]';

/** Argument names whose values are secrets, kept out of every recording whatever the tool. */
const SECRET_NAME = /pass(word|phrase|wd|code)?$|^pass|token|secret|api[-_]?key|credential|private[-_]?key|^otp$|cookie|auth/i;

/** The tools that type into fields, whose typed text is a secret when the field is one. */
const TYPING = new Set(['browser_type', 'browser_fill_form']);

/** The caller of calls whose window names none. */
const NOBODY = { session: '', name: '' };

/**
 * Text cut to at most `most` characters.
 * @param {string} text The text.
 * @param {number} most The most characters to keep.
 * @returns {string} The text, with `…` where it was cut.
 */
function cut(text, most) {
  return text.length > most ? `${text.slice(0, most)}…` : text;
}

/**
 * A time as a name a folder can have.
 * @param {number} at Milliseconds since the epoch.
 * @returns {string} Such as `2026-11-22T10-04-05`.
 */
function stamp(at) {
  return new Date(at).toISOString().slice(0, 19).replace(/:/g, '-');
}

/**
 * Text as a part of a folder's name: letters, digits and dashes, at most 40 of them.
 * @param {string} text The text, such as a session's title.
 * @returns {string} The part, or empty when nothing of it is left.
 */
function slug(text) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40)
    .replace(/-+$/, '');
}

/**
 * Where recordings go when the agent names no folder: `recordings` in the helper's own data folder.
 * @returns {string} The folder.
 */
function defaultBase() {
  const { app } = require('electron');
  return path.join(app.getPath('userData'), 'recordings');
}

/**
 * The caller a tool call names, made safe: strings only.
 * @param {Partial<Caller> | undefined} caller What the window said of the caller.
 * @returns {Caller} The caller.
 */
function callerOf(caller) {
  if (!caller) return NOBODY;
  return {
    session: typeof caller.session === 'string' ? caller.session : '',
    name: typeof caller.name === 'string' ? caller.name : '',
  };
}

/**
 * The readable summary of a recording: a heading, then one numbered line per step with its tool, arguments, outcome, address and picture.
 * @param {Recording} recording The recording.
 * @param {boolean} finished Whether it was stopped.
 * @returns {string} The summary, as Markdown.
 */
function summary(recording, finished) {
  const head = [
    `# Browser recording ${stamp(recording.started)}`,
    '',
    ...(recording.name || recording.session ? [`Session: ${recording.name || recording.session}`, ''] : []),
    `${recording.steps.length} step${recording.steps.length === 1 ? '' : 's'}${finished ? '' : ', still recording'}. Times are UTC.`,
    '',
  ];
  const lines = recording.steps.map((step) => {
    const outcome = step.ok ? 'ok' : 'failed';
    const picture = step.picture ? ` ![step ${step.index}](${path.basename(step.picture)})` : '';
    const answer = step.text.replace(/\s+/g, ' ').trim();
    return `${step.index}. ${new Date(step.at).toISOString().slice(11, 19)} \`${step.tool}\` ${step.arguments} — ${outcome}: ${cut(answer, 200)} (${step.url || 'no address'})${picture}`;
  });
  return [...head, ...lines, ''].join('\n');
}

/**
 * Writes a recording's summary and its steps as JSON into its folder.
 * @param {Recording} recording The recording.
 * @param {boolean} finished Whether it was stopped.
 * @returns {void}
 */
function save(recording, finished) {
  fs.writeFileSync(path.join(recording.folder, 'summary.md'), summary(recording, finished));
  fs.writeFileSync(path.join(recording.folder, 'steps.json'), `${JSON.stringify(recording.steps, null, 1)}\n`);
}

/**
 * Starts a recording of the calls of `caller`'s session in `folder`, or in a new folder under `base` named by the time and the session.
 * @param {unknown} folder The folder the agent named, which must be absolute, or nothing.
 * @param {string | undefined} base Where recordings go when no folder is named; the helper's data folder when not given.
 * @param {Caller} caller The session the recording is of.
 * @returns {ToolOutcome} The answer.
 */
function start(folder, base, caller) {
  const own = active.get(caller.session);
  if (own) return { ok: false, text: `A recording of this session is already under way in ${own.folder}, with ${own.steps.length} steps; stop it first.`, image: null };
  if (folder !== undefined && (typeof folder !== 'string' || !path.isAbsolute(folder))) {
    return { ok: false, text: 'give folder as an absolute path, or leave it out', image: null };
  }
  const started = Date.now();
  const named = [stamp(started), slug(caller.name), slug(caller.session)].filter(Boolean).join('-');
  const chosen = typeof folder === 'string' ? path.resolve(folder) : path.join(base === undefined ? defaultBase() : base, named);
  for (const other of active.values()) {
    if (other.folder === chosen) return { ok: false, text: `Another session records into ${chosen}; name another folder.`, image: null };
  }
  try {
    fs.mkdirSync(chosen, { recursive: true });
  } catch (error) {
    return { ok: false, text: `the folder ${chosen} could not be made: ${error instanceof Error ? error.message : error}`, image: null };
  }
  const recording = { folder: chosen, started, session: caller.session, name: caller.name, steps: [], taken: 0 };
  active.set(caller.session, recording);
  save(recording, false);
  return {
    ok: true,
    text: `Recording into ${chosen}. Every browser tool call of this session from now on is kept with its arguments (secrets typed into password fields and arguments named like passwords or tokens are redacted) and a screenshot taken after it, with password fields blurred, until browser_record with action "stop". Other sessions are not recorded here. The reader sees the steps in the browser pane.`,
    image: null,
  };
}

/**
 * Stops the recording of `caller`'s session and says what it holds.
 * @param {Caller} caller The session.
 * @returns {ToolOutcome} The answer, with the summary.
 */
function stop(caller) {
  const recording = active.get(caller.session);
  if (!recording) return { ok: false, text: 'No recording of this session is under way.', image: null };
  active.delete(caller.session);
  save(recording, true);
  return { ok: true, text: `Recording stopped. Its files are in ${recording.folder}: summary.md, steps.json and one picture per step.\n\n${summary(recording, true)}`, image: null };
}

/**
 * `browser_record`: starts, stops or describes the recording of the calling session.
 * @param {Json} args `action` (`start`, `stop` or `status`) and, for `start`, `folder`.
 * @param {string} [base] Where recordings go when no folder is named; the helper's data folder when not given.
 * @param {Partial<Caller>} [caller] The session that called; the one the window names no session for when not given.
 * @returns {ToolOutcome} The answer.
 */
function recordTool(args, base, caller) {
  const who = callerOf(caller);
  const action = args.action === undefined ? 'status' : args.action;
  if (action === 'start') return start(args.folder, base, who);
  if (action === 'stop') return stop(who);
  if (action === 'status') {
    const own = active.get(who.session);
    const text = own ? `Recording into ${own.folder}: ${own.steps.length} steps so far.` : 'No recording of this session is under way.';
    return { ok: true, text, image: null };
  }
  return { ok: false, text: 'give action as start, stop or status', image: null };
}

/**
 * A value with every part whose name says it is a secret replaced by {@link REDACTED}, however deep.
 * @param {unknown} value The value.
 * @returns {unknown} The value without secrets.
 */
function withoutNamedSecrets(value) {
  if (Array.isArray(value)) return value.map(withoutNamedSecrets);
  if (!value || typeof value !== 'object') return value;
  /** @type {Record<string, unknown>} */
  const out = {};
  for (const [key, held] of Object.entries(value)) {
    out[key] = SECRET_NAME.test(key) && held !== null && held !== undefined && held !== '' ? REDACTED : withoutNamedSecrets(held);
  }
  return out;
}

/**
 * The arguments of a call as a recording keeps them: as JSON text, without what was typed into a secret field and
 * without any argument whose name says it is a secret.
 * @param {string} tool The tool.
 * @param {string} argumentsText What the agent gave it, as JSON text.
 * @param {boolean[]} secrets For `browser_type`, whether its field is secret; for `browser_fill_form`, whether each of its fields is.
 * @returns {string} The arguments to keep.
 */
function redacted(tool, argumentsText, secrets) {
  /** @type {any} */
  let args;
  try {
    args = JSON.parse(argumentsText);
  } catch (_) {
    return '(arguments that are not JSON, not kept)';
  }
  if (!args || typeof args !== 'object' || Array.isArray(args)) return JSON.stringify(withoutNamedSecrets(args));
  const kept = /** @type {Record<string, any>} */ (withoutNamedSecrets(args));
  if (tool === 'browser_type' && secrets[0] && typeof kept.text === 'string') kept.text = REDACTED;
  if (tool === 'browser_fill_form' && Array.isArray(kept.fields)) {
    kept.fields = kept.fields.map((field, at) => (secrets[at] && field && typeof field === 'object' && typeof field.value === 'string' ? { ...field, value: REDACTED } : field));
  }
  return JSON.stringify(kept);
}

/**
 * Takes a screenshot of the page as it is, with its secret fields blurred, unless a dialog stops it, it takes too long
 * or the fields could not be blurred.
 * @param {Page} page The page.
 * @returns {Promise<Buffer | null>} The picture as PNG, or nothing.
 */
async function shoot(page) {
  const cdp = page.cdp;
  if (!cdp || (page.dialogs && page.dialogs.pending)) return null;
  /** @type {NodeJS.Timeout | undefined} */
  let timer;
  const late = new Promise((resolve) => {
    timer = setTimeout(() => resolve(null), SHOT_TIME);
  });
  const taken = secrets.masked(page, () => cdp.call('Page.captureScreenshot', { format: 'png' }).catch(() => null));
  const shot = await Promise.race([taken, late]).finally(() => clearTimeout(timer));
  if (!shot || !shot.masked) return null;
  const reply = shot.value;
  return reply && typeof reply.data === 'string' ? Buffer.from(reply.data, 'base64') : null;
}

/**
 * Whether the calls of `caller`'s session are recorded, or with no caller, whether any session's are.
 * @param {Partial<Caller>} [caller] The session.
 * @returns {boolean} Whether they are.
 */
function recording(caller) {
  return caller === undefined ? active.size > 0 : active.has(callerOf(caller).session);
}

/**
 * Whether the fields a call types into must be checked for secrets before it runs: it types, and its session is recorded.
 * @param {string} tool The tool.
 * @param {Partial<Caller>} [caller] The session that called.
 * @returns {boolean} Whether they must.
 */
function typesSecrets(tool, caller) {
  return TYPING.has(tool) && recording(caller);
}

/**
 * Keeps a tool call as a step of the recording of the session that made it, with a screenshot of the page after it.
 * @param {Page | null} page The page it ran on, or nothing when it ran on none, as `browser_tabs` may; the step then has no picture.
 * @param {string} tool The tool.
 * @param {string} argumentsText What the agent gave it, as JSON text.
 * @param {ToolOutcome} outcome What it came to.
 * @param {Partial<Caller>} [caller] The session that made the call; the one the window names no session for when not given.
 * @param {boolean[]} [secret] For the tools that type, whether each field they typed into holds a secret, whose text is then not kept.
 * @returns {Promise<RecordedStep | null>} The step, or nothing when the session is not recorded or the tool is not.
 */
async function afterTool(page, tool, argumentsText, outcome, caller, secret = []) {
  const who = callerOf(caller);
  const recorded = active.get(who.session);
  if (!recorded || UNRECORDED.has(tool)) return null;
  recorded.taken += 1;
  const index = recorded.taken;
  const picture = page ? await shoot(page) : null;
  let file = '';
  if (picture) {
    file = path.join(recorded.folder, `${String(index).padStart(3, '0')}-${tool.replace(/^browser_/, '')}.png`);
    try {
      fs.writeFileSync(file, picture);
    } catch (_) {
      file = '';
    }
  }
  /** @type {RecordedStep} */
  const step = {
    folder: recorded.folder,
    index,
    tool,
    arguments: cut(redacted(tool, argumentsText, secret), MOST_ARGUMENTS),
    ok: outcome.ok,
    text: cut(outcome.text, MOST_ANSWER),
    at: Date.now(),
    page: page ? page.id : 0,
    url: page && page.contents && !page.contents.isDestroyed() ? page.contents.getURL() : '',
    picture: file,
    session: recorded.session,
    session_name: recorded.name,
  };
  recorded.steps.push(step);
  try {
    save(recorded, false);
  } catch (_) {
    return step;
  }
  if (recorded.steps.length >= MOST_STEPS && active.get(who.session) === recorded) stop(who);
  return step;
}

module.exports = { recordTool, afterTool, recording, typesSecrets, redacted, summary, MOST_STEPS, REDACTED };
