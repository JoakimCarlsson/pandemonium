//! One agent: the process, the conversation it is holding and what it says.
//!
//! A session is an agent running in one worktree. Starting it, opening the
//! conversation and logging in where the agent insists on it all happen on
//! the reader thread, so nothing an agent does — including taking a minute to
//! start — ever holds a frame up. What comes back is a queue of [`Event`]s the
//! window drains on the wake that follows.
//!
//! The editor is the client here: the agent reads and writes files through it
//! and asks it before it runs a tool. Reading and writing are answered from
//! this thread; a permission request is not, because the answer is a reader's.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::agent::Agent;
use crate::transport;
use crate::update::{self, Event, Method, Mode, Stop, Tools};

/// The identifier the handshake is sent under.
const HANDSHAKE: i64 = 1;

/// The identifier the first request after the handshake is sent under.
const FIRST_REQUEST: i64 = 2;

/// The version of the protocol this client speaks.
const VERSION: i64 = 1;

/// The error an agent answers with when it will not work unless logged in.
const LOGIN_REQUIRED: i64 = -32000;

/// The error a request for something the editor does not do is refused with.
const NO_SUCH_METHOD: i64 = -32601;

/// The error a request the editor could not carry out is refused with.
const FAILED: i64 = -32603;

/// How much of what an agent writes on its error pipe is kept.
const TROUBLE: usize = 8 * 1024;

/// How a session wakes the window once it has something to say.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// What one request was sent to find out.
#[derive(Clone, Debug)]
enum Sent {
    /// The handshake.
    Handshake,
    /// A login, after which the conversation is opened again.
    Login,
    /// The conversation being opened.
    Open,
    /// A turn.
    Turn,
    /// A change of mode.
    Mode,
}

/// What the agent has said, and what it has not been told yet.
#[derive(Default)]
struct State {
    /// What the agent calls this conversation, once it has opened one.
    id: Option<String>,
    /// Whether a turn is running, and so whether another may be sent.
    busy: bool,
    /// The prompts waiting for the conversation, or for the turn before them.
    queued: Vec<String>,
    /// The requests sent and not yet answered, and what each was for.
    sent: HashMap<i64, Sent>,
    /// The ways of logging in the agent offered in its handshake.
    logins: Vec<Method>,
    /// The modes the session can be put into.
    modes: Vec<Mode>,
    /// The tool calls of this conversation, as they now stand.
    tools: Tools,
    /// What has arrived and not yet been drained.
    events: Vec<Event>,
    /// Whether anything has arrived since the window last looked.
    fresh: bool,
    /// The tail of what the agent has written on its error pipe.
    trouble: String,
    /// The permission requests waiting on a reader, by the ticket each was
    /// put to them under, against the identity the agent asked under.
    parked: HashMap<u64, Value>,
    /// The ticket the next permission request will be put to the reader as.
    ticket: u64,
}

/// An agent the editor is talking to.
pub struct Session {
    /// Which agent this is.
    agent: Agent,
    /// The worktree it is working in.
    root: PathBuf,
    /// The process itself, kept so that it can be ended.
    process: Mutex<Child>,
    /// The pipe messages are written to, shared with the reader thread.
    stdin: Arc<Mutex<ChildStdin>>,
    /// What the agent has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// The identifier the next request will be sent under, shared with the
    /// reader thread so that the two never number one twice.
    next: Arc<AtomicI64>,
}

impl Session {
    /// Starts `agent` in `root`, waking the window through `notify`.
    ///
    /// The handshake goes out here and is answered on the reader thread: a
    /// session is startable in a frame because nothing of it is waited for.
    pub fn start(agent: Agent, root: &Path, notify: Notify) -> std::io::Result<Self> {
        let mut process = agent
            .command()
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = process.stdin.take().expect("stdin was piped");
        let stdout = process.stdout.take().expect("stdout was piped");
        let stderr = process.stderr.take().expect("stderr was piped");
        let state = Arc::new(Mutex::new(State::default()));
        let next = Arc::new(AtomicI64::new(FIRST_REQUEST));

        let session = Self {
            agent,
            root: root.to_path_buf(),
            process: Mutex::new(process),
            stdin: Arc::new(Mutex::new(stdin)),
            state: state.clone(),
            next: next.clone(),
        };
        if let Ok(mut state) = session.state.lock() {
            state.sent.insert(HANDSHAKE, Sent::Handshake);
        }
        session.send(&json!({
            "jsonrpc": "2.0",
            "id": HANDSHAKE,
            "method": "initialize",
            "params": handshake(),
        }));

        let reader = Reader {
            root: root.to_path_buf(),
            state: state.clone(),
            notify,
            replies: Replies {
                stdin: session.stdin.clone(),
            },
            stdout: BufReader::new(stdout),
            next,
        };
        std::thread::spawn(move || reader.run());
        std::thread::spawn(move || watch(BufReader::new(stderr), &state));

        Ok(session)
    }

    /// Which agent this session is running.
    #[must_use]
    pub fn agent(&self) -> Agent {
        self.agent
    }

    /// The worktree it is working in.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Sends `text` as the reader's next turn.
    ///
    /// A prompt sent before the conversation is open, or while the turn
    /// before it is still running, is held until it can go: a reader types
    /// when they have something to say, not when the agent is ready.
    pub fn prompt(&self, text: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone().filter(|_| !state.busy) else {
            state.queued.push(text.to_owned());
            return;
        };
        state.busy = true;
        let request = self.request(&mut state, Sent::Turn, "session/prompt", &turn(&id, text));
        drop(state);
        self.send(&request);
    }

    /// Stops the turn that is running, if one is.
    pub fn cancel(&self) {
        let Ok(state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone() else {
            return;
        };
        drop(state);
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "session/cancel",
            "params": { "sessionId": id },
        }));
    }

    /// Answers the permission request `ask` with the choice `choice` names.
    pub fn allow(&self, ask: u64, choice: &str) {
        self.answer(
            ask,
            &json!({ "outcome": { "outcome": "selected", "optionId": choice } }),
        );
    }

    /// Answers the permission request `ask` by walking away from it.
    pub fn refuse(&self, ask: u64) {
        self.answer(ask, &json!({ "outcome": { "outcome": "cancelled" } }));
    }

    /// Replies to the permission request `ask`, if it is still waiting.
    ///
    /// A request is answered once: a reader who chooses twice, because two
    /// panes drew the same question, is not two answers to the agent.
    fn answer(&self, ask: u64, outcome: &Value) {
        let Some(id) = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.parked.remove(&ask))
        else {
            return;
        };
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": outcome }));
    }

    /// Logs in by the method `method` names, and opens the conversation.
    pub fn login(&self, method: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let request = self.request(
            &mut state,
            Sent::Login,
            "authenticate",
            &json!({ "methodId": method }),
        );
        drop(state);
        self.send(&request);
    }

    /// Puts the session into the mode `mode` names.
    pub fn set_mode(&self, mode: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone() else {
            return;
        };
        let request = self.request(
            &mut state,
            Sent::Mode,
            "session/set_mode",
            &json!({ "sessionId": id, "modeId": mode }),
        );
        drop(state);
        self.send(&request);
    }

    /// Everything the agent has said since this was last asked.
    pub fn drain(&self) -> Vec<Event> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.events))
            .unwrap_or_default()
    }

    /// The modes this session can be put into.
    #[must_use]
    pub fn modes(&self) -> Vec<Mode> {
        self.state
            .lock()
            .map(|state| state.modes.clone())
            .unwrap_or_default()
    }

    /// The tail of what the agent has written on its error pipe.
    ///
    /// An agent that will not start says why here and nowhere else: a missing
    /// key, a version it refuses to run under, a package that would not fetch.
    #[must_use]
    pub fn trouble(&self) -> String {
        self.state
            .lock()
            .map(|state| state.trouble.clone())
            .unwrap_or_default()
    }

    /// Whether anything has arrived since this was last asked.
    pub fn take_fresh(&self) -> bool {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.fresh))
            .unwrap_or_default()
    }

    /// Whether the agent's process is still there.
    pub fn is_running(&self) -> bool {
        self.process
            .lock()
            .map(|mut process| matches!(process.try_wait(), Ok(None)))
            .unwrap_or_default()
    }

    /// One request, numbered and taken down as sent.
    fn request(&self, state: &mut State, sent: Sent, method: &str, params: &Value) -> Value {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        state.sent.insert(id, sent);
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    /// Writes one message to the agent, dropping it if the pipe has gone.
    fn send(&self, message: &Value) {
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = transport::write(&mut *stdin, message);
        }
    }
}

impl Drop for Session {
    /// Ends the agent's process when the session is closed.
    fn drop(&mut self) {
        if let Ok(mut process) = self.process.lock() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

/// The pipe the reader thread writes its own messages on.
struct Replies {
    /// The same handle on the agent's standard input the session writes to.
    stdin: Arc<Mutex<ChildStdin>>,
}

impl Replies {
    /// Writes one message, dropping it if the pipe has gone.
    fn send(&self, message: &Value) {
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = transport::write(&mut *stdin, message);
        }
    }
}

/// The thread reading everything the agent says.
struct Reader {
    /// The worktree the agent is working in.
    root: PathBuf,
    /// What the agent has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// How the window is woken once something has arrived.
    notify: Notify,
    /// The pipe the agent is answered on.
    replies: Replies,
    /// The pipe the agent writes on.
    stdout: BufReader<std::process::ChildStdout>,
    /// The identifier the next request sent from here goes under, shared
    /// with the session.
    next: Arc<AtomicI64>,
}

impl Reader {
    /// Reads until the agent stops talking, and says so when it has.
    fn run(mut self) {
        while let Ok(Some(message)) = transport::read(&mut self.stdout) {
            self.dispatch(&message);
        }
        self.raise(Event::Ended);
    }

    /// Acts on one message: a reply of the agent's, or a request of its own.
    ///
    /// An identity the editor did not hand out is passed back untouched
    /// rather than read: the agent numbers its own requests, and how it does
    /// so is its business.
    fn dispatch(&self, message: &Value) {
        let id = message.get("id").filter(|id| !id.is_null());
        match (id, message["method"].as_str()) {
            (Some(id), Some(method)) => self.serve(id, method, &message["params"]),
            (Some(id), None) => {
                if let Some(id) = id.as_i64() {
                    self.replied(id, message);
                }
            }
            (None, Some("session/update")) => self.updated(&message["params"]["update"]),
            (None, _) => {}
        }
    }

    /// Takes down the reply to one request, and sends what follows from it.
    fn replied(&self, id: i64, message: &Value) {
        let Some(sent) = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.sent.remove(&id))
        else {
            return;
        };
        let failure = message.get("error").filter(|error| !error.is_null());
        match (sent, failure) {
            (Sent::Handshake, None) => self.shook(&message["result"]),
            (Sent::Open, None) => self.opened(&message["result"]),
            (Sent::Open, Some(error)) if error["code"] == json!(LOGIN_REQUIRED) => {
                let logins = self
                    .state
                    .lock()
                    .map(|state| state.logins.clone())
                    .unwrap_or_default();
                self.raise(Event::Login(logins));
            }
            (Sent::Login, None) => self.open(),
            (Sent::Turn, None) => {
                self.raise(Event::Stopped(Stop::read(&message["result"]["stopReason"])));
                self.idle();
            }
            (Sent::Turn, Some(error)) => {
                self.raise(Event::Failed(complaint(error)));
                self.idle();
            }
            (_, Some(error)) => self.raise(Event::Failed(complaint(error))),
            (Sent::Mode, None) => {}
        }
    }

    /// Takes down what the agent can do, and opens the conversation.
    fn shook(&self, result: &Value) {
        if let Ok(mut state) = self.state.lock() {
            state.logins = update::methods(&result["authMethods"]);
        }
        self.open();
    }

    /// Asks the agent to open a conversation over the worktree.
    fn open(&self) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            state.sent.insert(id, Sent::Open);
        }
        self.replies.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/new",
            "params": { "cwd": self.root, "mcpServers": [] },
        }));
    }

    /// Takes down the conversation the agent opened, and starts talking.
    fn opened(&self, result: &Value) {
        let Some(id) = result["sessionId"].as_str().map(str::to_owned) else {
            self.raise(Event::Failed("the agent opened no session".to_owned()));
            return;
        };
        let modes = update::modes(&result["modes"]);
        let current = result["modes"]["currentModeId"].as_str().map(str::to_owned);

        if let Ok(mut state) = self.state.lock() {
            state.id = Some(id);
            state.modes = modes;
            state.events.push(Event::Ready);
            if let Some(current) = current {
                state.events.push(Event::Mode(current));
            }
            state.fresh = true;
        }
        self.wake();
        self.idle();
    }

    /// Lets the next prompt that was held back go, if one was.
    fn idle(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.busy = false;
        let Some(session) = state.id.clone() else {
            return;
        };
        if state.queued.is_empty() {
            return;
        }
        let text = state.queued.remove(0);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        state.busy = true;
        state.sent.insert(id, Sent::Turn);
        drop(state);

        self.replies.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/prompt",
            "params": turn(&session, &text),
        }));
    }

    /// Takes down one thing the agent said during a turn.
    fn updated(&self, update: &Value) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(event) = update::event(update, &mut state.tools) else {
            return;
        };
        state.events.push(event);
        state.fresh = true;
        drop(state);
        self.wake();
    }

    /// Answers one request of the agent's.
    ///
    /// A request the editor does not serve is refused rather than ignored: an
    /// agent left waiting for a reply it asked for stops saying anything at
    /// all, which reads as a hung session rather than a missing feature.
    fn serve(&self, id: &Value, method: &str, params: &Value) {
        match method {
            "fs/read_text_file" => match read(&self.root, params) {
                Ok(content) => self.answer(id, &json!({ "content": content })),
                Err(error) => self.refuse(id, FAILED, &error.to_string()),
            },
            "fs/write_text_file" => match write(&self.root, params) {
                Ok(()) => self.answer(id, &json!({})),
                Err(error) => self.refuse(id, FAILED, &error.to_string()),
            },
            "session/request_permission" => self.park(id, params),
            _ => self.refuse(id, NO_SUCH_METHOD, method),
        }
    }

    /// Puts a permission request to the reader and leaves it unanswered.
    ///
    /// The agent is waiting on this reply, and so it should be: the request
    /// is a question, and a question answered by the editor on the reader's
    /// behalf is a permission that was never asked for.
    fn park(&self, id: &Value, params: &Value) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let ticket = state.ticket;
        let Some(ask) = update::ask(ticket, params, &mut state.tools) else {
            drop(state);
            self.answer(id, &json!({ "outcome": { "outcome": "cancelled" } }));
            return;
        };
        state.ticket += 1;
        state.parked.insert(ticket, id.clone());
        state.events.push(Event::Asked(ask));
        state.fresh = true;
        drop(state);
        self.wake();
    }

    /// Replies to a request of the agent's.
    fn answer(&self, id: &Value, result: &Value) {
        self.replies
            .send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    /// Refuses a request of the agent's.
    fn refuse(&self, id: &Value, code: i64, message: &str) {
        self.replies.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }));
    }

    /// Adds `event` to what the window has yet to see, and wakes it.
    fn raise(&self, event: Event) {
        if let Ok(mut state) = self.state.lock() {
            state.events.push(event);
            state.fresh = true;
        }
        self.wake();
    }

    /// Wakes the window.
    fn wake(&self) {
        (self.notify)();
    }
}

/// Keeps the tail of what the agent writes on its error pipe.
fn watch(stderr: impl BufRead, state: &Mutex<State>) {
    for line in stderr.lines().map_while(Result::ok) {
        let Ok(mut state) = state.lock() else {
            return;
        };
        state.trouble.push_str(&line);
        state.trouble.push('\n');
        if state.trouble.len() > TROUBLE {
            let over = state.trouble.len() - TROUBLE;
            let from = state
                .trouble
                .char_indices()
                .map(|(at, _)| at)
                .find(|at| *at >= over)
                .unwrap_or(state.trouble.len());
            state.trouble = state.trouble.split_off(from);
        }
    }
}

/// The turn `text` comes to, as the agent is asked to take it.
fn turn(session: &str, text: &str) -> Value {
    json!({
        "sessionId": session,
        "prompt": [{ "type": "text", "text": text }],
    })
}

/// What the editor tells an agent about itself when it starts one.
fn handshake() -> Value {
    json!({
        "protocolVersion": VERSION,
        "clientInfo": {
            "name": "pandemonium",
            "title": "Pandemonium",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "clientCapabilities": {
            "fs": { "readTextFile": true, "writeTextFile": true },
            "terminal": false,
        },
    })
}

/// What a file the agent asked for holds, from the line it asked for.
fn read(root: &Path, params: &Value) -> std::io::Result<String> {
    let path = within(root, params)?;
    let text = std::fs::read_to_string(path)?;

    let from = params["line"].as_u64().unwrap_or(1).max(1) as usize - 1;
    let count = params["limit"].as_u64().unwrap_or(u64::MAX) as usize;
    if from == 0 && count == usize::MAX {
        return Ok(text);
    }
    Ok(text
        .lines()
        .skip(from)
        .take(count)
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Writes what the agent asked to be written.
fn write(root: &Path, params: &Value) -> std::io::Result<()> {
    let path = within(root, params)?;
    let content = params["content"].as_str().unwrap_or_default();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

/// The file `params` names, once it is known to be one of the worktree's.
///
/// The agent is a program of the reader's, running as they do, and nothing
/// here stops it opening a file for itself. What this stops is the editor
/// doing it on the agent's behalf: the session was opened over one worktree,
/// so the worktree is the whole of what the editor will read or write
/// through, and a path that climbs out of it is refused rather than followed.
fn within(root: &Path, params: &Value) -> std::io::Result<PathBuf> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| std::io::Error::other("no path"))?;
    let path = cleaned(&root.join(path));

    match path.starts_with(cleaned(root)) {
        true => Ok(path),
        false => Err(std::io::Error::other(format!(
            "{} is outside this session's worktree",
            path.display()
        ))),
    }
}

/// `path` with the steps that go nowhere taken out of it.
///
/// The disk is not asked: a file being written may not exist yet, and one
/// that does may be reached through a link the reader meant to follow. What
/// is resolved here is only the spelling — `.` and the `..` that a path
/// climbs out through.
fn cleaned(path: &Path) -> PathBuf {
    let mut cleaned = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                cleaned.pop();
            }
            part => cleaned.push(part),
        }
    }
    cleaned
}

/// What an agent's error says, in one line.
fn complaint(error: &Value) -> String {
    error["message"]
        .as_str()
        .unwrap_or("the agent refused")
        .to_owned()
}
