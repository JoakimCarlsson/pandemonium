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
use crate::attachment::Attachment;
use crate::transport;
use crate::update::{self, Event, Knob, Method, Mode, Setting, Stop, Tools};

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
    /// A conversation from a launch before this one being taken up again.
    Resume,
    /// One page of saved sessions, requested after `cursor` where present.
    List(Option<String>),
    /// A turn.
    Turn,
    /// A change of mode, from the mode the session was in before it.
    Mode(Option<String>),
    /// A knob being set, from the knobs as they stood before it.
    Knob(Vec<Knob>),
}

/// A turn waiting for the agent, including context attached to its text.
struct Prompt {
    /// The words the reader sent.
    text: String,
    /// Files and images sent with those words.
    attachments: Vec<Attachment>,
}

/// What the agent has said, and what it has not been told yet.
#[derive(Default)]
struct State {
    /// What the agent calls this conversation, once it has opened one.
    id: Option<String>,
    /// The conversation to take up again, before one has been opened.
    resume: Option<String>,
    /// Whether a failed load should open a fresh session for layout recovery.
    resume_fallback: bool,
    /// Whether the agent said it can take a conversation up again.
    loads: bool,
    /// Whether the agent can list its saved sessions.
    lists: bool,
    /// Whether a turn is running, and so whether another may be sent.
    busy: bool,
    /// The prompts waiting for the conversation, or for the turn before them.
    queued: Vec<Prompt>,
    /// Whether the agent accepts image content in prompts.
    images: bool,
    /// Whether the agent accepts a file's contents in a prompt, rather than
    /// only a link to it.
    embeds: bool,
    /// The requests sent and not yet answered, and what each was for.
    sent: HashMap<i64, Sent>,
    /// The ways of logging in the agent offered in its handshake.
    logins: Vec<Method>,
    /// The modes the session can be put into.
    modes: Vec<Mode>,
    /// The mode the window has last been told the session is in.
    mode: Option<String>,
    /// What the session can be set to, as the agent last offered it.
    knobs: Vec<Knob>,
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
    /// How the window is woken when the session has something to say of its
    /// own, without having been told it by the agent.
    notify: Notify,
}

impl Session {
    /// Starts `agent` in `root`, waking the window through `notify`.
    ///
    /// The handshake goes out here and is answered on the reader thread: a
    /// session is startable in a frame because nothing of it is waited for.
    pub fn start(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(agent, root, env, None, false, notify)
    }

    /// Starts `agent` in `root` and takes the conversation `id` names up again.
    ///
    /// A window comes back as the last launch left it, and a session is one
    /// of the things it comes back to: the agent is started again and asked
    /// for the conversation it was holding, which it replays. An agent that
    /// cannot do that opens a new conversation instead, because a pane with a
    /// fresh agent in it is nearer to what the reader left than no pane.
    pub fn resume(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        id: &str,
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(agent, root, env, Some(id.to_owned()), true, notify)
    }

    /// Loads `id` exactly, reporting failure when that saved session is gone.
    pub fn load(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        id: &str,
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(agent, root, env, Some(id.to_owned()), false, notify)
    }

    /// Starts `agent` in `root`, taking up `resume` where there is one.
    ///
    /// The `env` is the worktree's own, so what the agent runs — a dev
    /// server, a test that binds a port — is the session's rather than
    /// whatever the machine's environment named.
    fn open(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        resume: Option<String>,
        resume_fallback: bool,
        notify: Notify,
    ) -> std::io::Result<Self> {
        let mut process = agent
            .command()
            .current_dir(root)
            .envs(env.iter().map(|(name, value)| (name, value)))
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
            notify: notify.clone(),
        };
        if let Ok(mut state) = session.state.lock() {
            state.sent.insert(HANDSHAKE, Sent::Handshake);
            state.resume = resume;
            state.resume_fallback = resume_fallback;
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

    /// What the agent calls this conversation, once it has opened one.
    ///
    /// This is what a launch writes down and hands back to [`Self::resume`],
    /// and it is the agent's name for the conversation, not the editor's.
    #[must_use]
    pub fn id(&self) -> Option<String> {
        self.state.lock().ok()?.id.clone()
    }

    /// Whether this agent can list previously saved sessions.
    pub fn can_list(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.lists && state.loads)
    }

    /// Asks the agent for saved sessions in this worktree.
    pub fn list_sessions(&self) {
        if !self.can_list() {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let request = self.request(
            &mut state,
            Sent::List(None),
            "session/list",
            &json!({ "cwd": self.root }),
        );
        drop(state);
        self.send(&request);
    }

    /// Sends `text` as the reader's next turn.
    ///
    /// A prompt sent before the conversation is open, or while the turn
    /// before it is still running, is held until it can go: a reader types
    /// when they have something to say, not when the agent is ready.
    pub fn prompt(&self, text: &str, attachments: Vec<Attachment>) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let prompt = Prompt {
            text: text.to_owned(),
            attachments,
        };
        let Some(id) = state.id.clone().filter(|_| !state.busy) else {
            state.queued.push(prompt);
            return;
        };
        state.busy = true;
        let params = turn(&id, &prompt, state.embeds);
        let request = self.request(&mut state, Sent::Turn, "session/prompt", &params);
        drop(state);
        self.send(&request);
    }

    /// Whether this agent has advertised image prompt support.
    pub fn can_image(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.images)
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
    ///
    /// The mode is taken as changed as soon as it is asked for: the agent
    /// answers a set mode with nothing at all, and a reader who has chosen a
    /// mode should see it. An agent that refuses puts back the mode it was in.
    pub fn set_mode(&self, mode: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone() else {
            return;
        };
        let was = state.mode.replace(mode.to_owned());
        state.events.push(Event::Mode(mode.to_owned()));
        state.fresh = true;
        let request = self.request(
            &mut state,
            Sent::Mode(was),
            "session/set_mode",
            &json!({ "sessionId": id, "modeId": mode }),
        );
        drop(state);
        self.send(&request);
        (self.notify)();
    }

    /// Sets the knob `knob` names to the value `value` names.
    ///
    /// Like a mode, a knob is taken as set as soon as it is asked for: the
    /// agent answers with the knobs as they now stand, which replaces this,
    /// and an agent that refuses puts back the knobs as they were.
    pub fn set_knob(&self, knob: &str, value: &str) {
        self.turn_knob(knob, json!({ "value": value }), |setting| {
            if let Setting::Picked { value: set, .. } = setting {
                *set = value.to_owned();
            }
        });
    }

    /// Puts the switch `knob` names on or off.
    pub fn switch_knob(&self, knob: &str, on: bool) {
        self.turn_knob(knob, json!({ "type": "boolean", "value": on }), |setting| {
            if let Setting::Switched(set) = setting {
                *set = on;
            }
        });
    }

    /// Asks for the knob `knob` names to be set, having set it here first.
    ///
    /// `value` is what goes on the wire and `set` is the same change made to
    /// the knob the window is drawing, so that the two never disagree while
    /// the agent is answering.
    fn turn_knob(&self, knob: &str, value: Value, set: impl FnOnce(&mut Setting)) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone() else {
            return;
        };
        let were = state.knobs.clone();
        if let Some(turned) = state.knobs.iter_mut().find(|held| held.id == knob) {
            set(&mut turned.setting);
        }
        let knobs = state.knobs.clone();
        state.events.push(Event::Knobs(knobs));
        state.fresh = true;

        let mut params = json!({ "sessionId": id, "configId": knob });
        if let (Some(params), Some(value)) = (params.as_object_mut(), value.as_object()) {
            params.extend(value.clone());
        }
        let request = self.request(
            &mut state,
            Sent::Knob(were),
            "session/set_config_option",
            &params,
        );
        drop(state);
        self.send(&request);
        (self.notify)();
    }

    /// Everything the agent has said since this was last asked.
    pub fn drain(&self) -> Vec<Event> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.events))
            .unwrap_or_default()
    }

    /// What this session can be set to, as the agent last offered it.
    #[must_use]
    pub fn knobs(&self) -> Vec<Knob> {
        self.state
            .lock()
            .map(|state| state.knobs.clone())
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
            (Sent::Resume, None) => self.resumed(&message["result"]),
            (Sent::List(cursor), None) => {
                let result = &message["result"];
                let next = result["nextCursor"].as_str().map(str::to_owned);
                let more = next
                    .as_ref()
                    .is_some_and(|next| Some(next) != cursor.as_ref());
                self.raise(Event::Listed(update::history(result), more));
                if more {
                    self.ask(
                        Sent::List(next.clone()),
                        "session/list",
                        &json!({ "cwd": self.root, "cursor": next }),
                    );
                }
            }
            (Sent::List(_), Some(error)) => self.raise(Event::ListFailed(complaint(error))),
            (Sent::Resume, Some(_)) => {
                let fallback = self.state.lock().is_ok_and(|state| state.resume_fallback);
                if fallback {
                    if let Ok(mut state) = self.state.lock() {
                        state.resume = None;
                    }
                    self.open();
                } else if let Some(error) = failure {
                    self.raise(Event::Failed(complaint(error)));
                }
            }
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
            (Sent::Mode(was), Some(error)) => {
                self.raise(Event::Failed(complaint(error)));
                if let Some(was) = was {
                    self.moded(&was);
                }
            }
            (Sent::Knob(were), Some(error)) => {
                self.raise(Event::Failed(complaint(error)));
                self.knobbed(were);
            }
            (_, Some(error)) => self.raise(Event::Failed(complaint(error))),
            (Sent::Mode(_), None) => {}
            (Sent::Knob(_), None) => {
                self.knobbed(update::knobs(&message["result"]["configOptions"]));
            }
        }
    }

    /// Takes down what the agent can do, and opens the conversation.
    fn shook(&self, result: &Value) {
        if let Ok(mut state) = self.state.lock() {
            state.logins = update::methods(&result["authMethods"]);
            state.loads = result["agentCapabilities"]["loadSession"] == json!(true);
            state.lists = result["agentCapabilities"]["sessionCapabilities"]["list"].is_object();
            let prompts = &result["agentCapabilities"]["promptCapabilities"];
            state.images = prompts["image"] == json!(true);
            state.embeds = prompts["embeddedContext"] == json!(true);
        }
        self.open();
    }

    /// Opens the conversation: the one that was left, or a new one.
    fn open(&self) {
        let resumed = match self.state.lock() {
            Ok(state) => state.resume.clone().filter(|_| state.loads),
            Err(_) => None,
        };
        match resumed {
            Some(resumed) => self.ask(
                Sent::Resume,
                "session/load",
                &json!({
                    "sessionId": resumed,
                    "cwd": self.root,
                    "mcpServers": [],
                }),
            ),
            None if self
                .state
                .lock()
                .is_ok_and(|state| state.resume.is_some() && !state.resume_fallback) =>
            {
                self.raise(Event::Failed(
                    "this agent cannot load saved sessions".to_owned(),
                ));
            }
            None => self.ask(
                Sent::Open,
                "session/new",
                &json!({ "cwd": self.root, "mcpServers": [] }),
            ),
        }
    }

    /// Takes up the conversation the last launch left, or opens a new one.
    ///
    /// The agent replays what was said before it answers, so by the time this
    /// returns the transcript is already back; what is left is to say which
    /// conversation the prompts now go to.
    fn resumed(&self, result: &Value) {
        let resumed = match self.state.lock() {
            Ok(mut state) => state.resume.take(),
            Err(_) => None,
        };
        let Some(resumed) = resumed else {
            return self.open();
        };
        let mut result = result.clone();
        result["sessionId"] = json!(resumed);
        self.opened(&result);
    }

    /// Sends one request of the reader thread's own.
    fn ask(&self, sent: Sent, method: &str, params: &Value) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            state.sent.insert(id, sent);
        }
        self.replies
            .send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
    }

    /// Takes down the conversation the agent opened, and starts talking.
    ///
    /// An agent says what its session can be set to in one of two ways: as
    /// modes, or as the knobs that took their place and hold the mode among
    /// them. An agent that says both is taken at its newer word, so that one
    /// fact about the session is never shown twice.
    fn opened(&self, result: &Value) {
        let Some(id) = result["sessionId"].as_str().map(str::to_owned) else {
            self.raise(Event::Failed("the agent opened no session".to_owned()));
            return;
        };
        let knobs = update::knobs(&result["configOptions"]);
        let has_knobs = !knobs.is_empty();
        let modes = match has_knobs {
            true => Vec::new(),
            false => update::modes(&result["modes"]),
        };
        let current = result["modes"]["currentModeId"]
            .as_str()
            .filter(|_| !has_knobs)
            .map(str::to_owned);

        if let Ok(mut state) = self.state.lock() {
            state.id = Some(id);
            state.knobs = knobs.clone();
            state.modes = modes;
            state.mode = current.clone();
            state.events.push(Event::Ready);
            if has_knobs {
                state.events.push(Event::Knobs(knobs));
            }
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
        let prompt = state.queued.remove(0);
        let embeds = state.embeds;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        state.busy = true;
        state.sent.insert(id, Sent::Turn);
        drop(state);

        self.replies.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/prompt",
            "params": turn(&session, &prompt, embeds),
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
        match &event {
            Event::Mode(mode) => state.mode = Some(mode.clone()),
            Event::Knobs(knobs) => state.knobs = knobs.clone(),
            _ => {}
        }
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

    /// Says the session is in the mode `mode` names, without asking for it.
    ///
    /// This is the way back from a mode the agent would not take: what it
    /// puts back is what the session was in before the mode was asked for.
    fn moded(&self, mode: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.mode = Some(mode.to_owned());
        }
        self.raise(Event::Mode(mode.to_owned()));
    }

    /// Says what the session is set to, as the agent has just put it.
    ///
    /// This is both the answer to a knob being set and the way back from one
    /// the agent would not set: either way the agent's list is the list.
    fn knobbed(&self, knobs: Vec<Knob>) {
        if knobs.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state.knobs = knobs.clone();
        }
        self.raise(Event::Knobs(knobs));
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

/// The turn `prompt` comes to, as the agent is asked to take it, with the
/// files attached to it embedded where the agent `embeds`.
fn turn(session: &str, prompt: &Prompt, embeds: bool) -> Value {
    let mut content = Vec::new();
    if !prompt.text.is_empty() {
        content.push(json!({ "type": "text", "text": prompt.text }));
    }
    content.extend(
        prompt
            .attachments
            .iter()
            .map(|attachment| attachment.content(embeds)),
    );
    json!({
        "sessionId": session,
        "prompt": content,
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
