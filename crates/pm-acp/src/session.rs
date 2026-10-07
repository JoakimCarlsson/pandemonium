//! One agent: the process, the conversation it is holding and what it says.
//!
//! A session is an agent running in one worktree. Starting it, opening the
//! conversation and logging in where the agent insists on it all happen on
//! the reader thread, so nothing an agent does — including taking a minute to
//! start — ever holds a frame up. What comes back is a queue of [`Event`]s the
//! window drains on the wake that follows.
//!
//! The editor is the client here: the agent reads and writes files through
//! it, runs commands in its terminals and asks it before it runs a tool. None
//! of those is answered from this thread. A permission is the reader's to
//! give, and a file or a terminal is the window's: each goes up as an event
//! under a ticket and comes back down through the session.
//!
//! Nothing is written to the agent from the window's thread or the reader's
//! either. Both hand what they have to say to the writer thread, which builds
//! it, serializes it and writes it in the order it was handed over: an agent
//! slow to read its pipe holds up that thread, never a frame, and never the
//! reader it is waiting to be read by.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::agent::Agent;
use crate::attachment::Attachment;
use crate::elicitation::{self, Reply};
use crate::limits::Meter;
use crate::mcp;
use crate::process::{self, Containment};
use crate::request::{self, Answer, Request, Shape};
use crate::subagent::Subagents;
use crate::transport;
use crate::update::{self, Event, Knob, Method, Mode, Setting, Status, Stop, Tools};

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

/// The error a request that makes no sense is refused with.
const INVALID: i64 = -32602;

/// How much of what an agent writes on its error pipe is kept.
const TROUBLE: usize = 8 * 1024;

/// How long the agent's process group has to end before it is killed.
const END_WITHIN: Duration = Duration::from_secs(2);

/// How often account limits are refreshed while a session stays open.
const LIMIT_REFRESH: Duration = Duration::from_secs(60);

/// How a session wakes the window once it has something to say.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// How a process opens its first conversation after the handshake.
#[derive(Default)]
struct Opening {
    /// MCP servers belonging only to this connection.
    servers: Vec<mcp::McpServer>,
    /// The native conversation to fork instead of opening or loading.
    fork: Option<String>,
    /// The provider message through which the fork retains history.
    fork_message: Option<String>,
    /// The conversation to restore, when supplied.
    resume: Option<String>,
    /// Whether a missing saved conversation falls back to a new one.
    resume_fallback: bool,
    /// Whether restoration skips replaying the transcript.
    quiet: bool,
    /// Whether the client explicitly requests login before opening a conversation.
    login_first: bool,
}

/// The conversation an adapter is asked to open after negotiation.
pub enum Conversation {
    /// A fresh conversation.
    New,
    /// Login before a fresh conversation.
    Login,
    /// Restore a saved conversation, falling back when unavailable.
    Restore(String),
    /// Load an exact saved conversation with replay.
    Load(String),
    /// Resume without replay, falling back when unavailable.
    Reconnect(String),
    /// Resume an exact saved conversation without replay.
    ReconnectExact(String),
    /// Fork an advertised native conversation.
    Fork(String),
    /// Fork through one native Claude agent message.
    ForkAt(String, String),
}

impl Conversation {
    /// The requested opening behavior before adapter negotiation.
    fn opening(self) -> Opening {
        let mut opening = Opening::default();
        match self {
            Self::New => {}
            Self::Login => opening.login_first = true,
            Self::Restore(id) | Self::Reconnect(id) => {
                opening.resume = Some(id);
                opening.resume_fallback = true;
            }
            Self::Load(id) | Self::ReconnectExact(id) => opening.resume = Some(id),
            Self::Fork(id) => opening.fork = Some(id),
            Self::ForkAt(id, message) => {
                opening.fork = Some(id);
                opening.fork_message = Some(message);
            }
        }
        opening
    }
}

/// What one request was sent to find out.
#[derive(Clone, Debug)]
enum Sent {
    /// The handshake.
    Handshake,
    /// A native copy of another conversation.
    Fork(String),
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
    /// The agent's own request for its plan's limits.
    Limits,
    /// A logout, after which the conversation is opened again.
    Logout,
    /// The saved session of this name being forgotten.
    Delete(String),
}

/// A request of the agent's that the window has yet to answer.
struct Owed {
    /// The identity the agent asked under.
    id: Value,
    /// What it asked for.
    request: Request,
    /// What the answer has to be shaped into.
    shape: Shape,
}

/// One message on its way to the agent, as it is handed to the writer.
///
/// A message that costs something to build is handed over as what it is
/// built from, so that reading a file or shaping a terminal's output happens
/// on the writer thread rather than on whichever thread had it to say.
enum Outgoing {
    /// A message ready to be written as it stands.
    Message(Value),
    /// Closes the agent's input after earlier messages have been written.
    Close {
        /// Notifies the shutdown worker once the input pipe is closed.
        closed: Sender<()>,
    },
    /// A turn, whose content is read and built only once it is written.
    Turn {
        /// The identifier the request goes under.
        id: i64,
        /// What the agent calls the conversation.
        session: String,
        /// What the reader sent.
        prompt: Prompt,
        /// Whether the agent takes a file's contents rather than a link.
        embeds: bool,
    },
    /// The window's answer to a file or terminal request of the agent's.
    Answer {
        /// The request being answered.
        owed: Box<Owed>,
        /// The most output the terminal it names will keep, where it set one.
        limit: Option<usize>,
        /// What the window came back with.
        answer: Answer,
    },
}

impl Outgoing {
    /// The message this comes to on the wire.
    fn build(self) -> Value {
        match self {
            Self::Message(message) => message,
            Self::Close { .. } => unreachable!("a close is handled before building"),
            Self::Turn {
                id,
                session,
                prompt,
                embeds,
            } => json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "session/prompt",
                "params": turn(&session, &prompt, embeds),
            }),
            Self::Answer {
                owed,
                limit,
                answer,
            } => match request::reply(&owed.request, &owed.shape, limit, answer) {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": owed.id, "result": result }),
                Err(trouble) => json!({
                    "jsonrpc": "2.0",
                    "id": owed.id,
                    "error": { "code": FAILED, "message": trouble },
                }),
            },
        }
    }
}

/// A turn waiting for the agent, including context attached to its text.
struct Prompt {
    /// Whether the editor must prepare this queued prompt before wire delivery.
    deferred: bool,
    /// The words the reader sent.
    text: String,
    /// Files and images sent with those words.
    attachments: Vec<Attachment>,
}

/// What the agent has said, and what it has not been told yet.
#[derive(Default)]
struct State {
    /// MCP servers belonging only to this connection.
    servers: Vec<mcp::McpServer>,
    /// The source of a fork awaiting negotiation.
    fork: Option<String>,
    /// The provider message through which the fork retains history.
    fork_message: Option<String>,
    /// Whether the adapter advertises native conversation forking.
    forks: bool,
    /// Whether the negotiated adapter implements Claude message fork points.
    fork_points: bool,
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
    /// Whether the agent can take a conversation up again without replaying it.
    resumes: bool,
    /// Whether the agent wants to be told a conversation is finished with.
    closes: bool,
    /// Whether the agent can forget a saved session.
    deletes: bool,
    /// Whether the agent can be logged out.
    logouts: bool,
    /// Whether the conversation to take up again is taken up without a replay.
    quiet: bool,
    /// The tool servers the conversation was opened with, and what became of each.
    mcp: Vec<mcp::Offered>,
    /// Whether a turn is running, and so whether another may be sent.
    busy: bool,
    /// The prompts waiting for the conversation, or for the turn before them.
    queued: Vec<Prompt>,
    /// Whether the agent accepts image content in prompts.
    images: bool,
    /// Whether the agent accepts a file's contents in a prompt, rather than
    /// only a link to it.
    embeds: bool,
    /// The ways other than a started program the agent can reach a tool server.
    transports: mcp::Transports,
    /// The requests sent and not yet answered, and what each was for.
    sent: HashMap<i64, Sent>,
    /// Whether the initial handshake must offer login before opening a conversation.
    login_first: bool,
    /// The ways of logging in the agent offered in its handshake.
    logins: Vec<Method>,
    /// The modes the session can be put into.
    modes: Vec<Mode>,
    /// The mode the window has last been told the session is in.
    mode: Option<String>,
    /// What the session can be set to, as the agent last offered it.
    knobs: Vec<Knob>,
    /// What has arrived and not yet been drained.
    events: Vec<Event>,
    /// Whether anything has arrived since the window last looked.
    fresh: bool,
    /// The tail of what the agent has written on its error pipe.
    trouble: String,
    /// The permission requests waiting on a reader, by the ticket each was
    /// put to them under, against the identity the agent asked under.
    parked: HashMap<u64, Value>,
    /// The file and terminal requests waiting on the window, by the ticket
    /// each was raised under.
    owed: HashMap<u64, Owed>,
    /// The most output each terminal's agent will keep, by the terminal's
    /// name, for the terminals that set one.
    limits: HashMap<String, usize>,
}

/// An agent the editor is talking to.
pub struct Session {
    /// Which agent this is.
    agent: Agent,
    /// The worktree it is working in.
    root: PathBuf,
    /// The process itself, kept so that it can be ended, until it has been.
    process: Mutex<Option<Child>>,
    /// The operating system container for every process the agent starts.
    containment: Containment,
    /// Where messages for the agent are handed to the writer thread.
    outbox: Sender<Outgoing>,
    /// What the agent has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// The identifier the next request will be sent under, shared with the
    /// reader thread so that the two never number one twice.
    next: Arc<AtomicI64>,
    /// How the window is woken when the session has something to say of its
    /// own, without having been told it by the agent.
    notify: Notify,
    /// Keeps periodic limit refreshes alive until this session closes.
    limit_polling: Option<Sender<()>>,
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
        Self::open(agent, root, env, Opening::default(), notify)
    }

    /// Starts one agent with caller-owned MCP servers and the requested opening behavior.
    pub fn configured(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        conversation: Conversation,
        servers: Vec<mcp::McpServer>,
        notify: Notify,
    ) -> std::io::Result<Self> {
        let quiet = matches!(
            conversation,
            Conversation::Reconnect(_) | Conversation::ReconnectExact(_)
        );
        let mut opening = conversation.opening();
        opening.quiet = quiet;
        opening.servers = servers;
        Self::open(agent, root, env, opening, notify)
    }

    /// Starts the agent and offers its login methods before opening any conversation.
    pub fn start_login(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(
            agent,
            root,
            env,
            Opening {
                login_first: true,
                ..Opening::default()
            },
            notify,
        )
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
        Self::open(
            agent,
            root,
            env,
            Opening {
                resume: Some(id.to_owned()),
                resume_fallback: true,
                quiet: false,
                ..Opening::default()
            },
            notify,
        )
    }

    /// Starts `agent` in `root` and carries on the conversation `id` names,
    /// for a window that still holds what was said in it.
    ///
    /// Nothing is replayed, because the window has the transcript already:
    /// the agent is asked to resume the conversation, which restores its
    /// context and says nothing. An agent that cannot do that opens a new
    /// conversation and says so with [`Event::Fresh`].
    pub fn reconnect(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        id: &str,
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(
            agent,
            root,
            env,
            Opening {
                resume: Some(id.to_owned()),
                resume_fallback: true,
                quiet: true,
                ..Opening::default()
            },
            notify,
        )
    }

    /// Reconnects an exact conversation without ever substituting a fresh one.
    pub fn reconnect_exact(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        id: &str,
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(
            agent,
            root,
            env,
            Opening {
                resume: Some(id.to_owned()),
                quiet: true,
                ..Opening::default()
            },
            notify,
        )
    }

    /// Loads `id` exactly, reporting failure when that saved session is gone.
    pub fn load(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        id: &str,
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(
            agent,
            root,
            env,
            Opening {
                resume: Some(id.to_owned()),
                resume_fallback: false,
                quiet: false,
                ..Opening::default()
            },
            notify,
        )
    }

    /// Starts `agent` in `root` with the requested conversation or login behavior.
    ///
    /// The `env` is the worktree's own, so what the agent runs — a dev
    /// server, a test that binds a port — is the session's rather than
    /// whatever the machine's environment named.
    fn open(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        opening: Opening,
        notify: Notify,
    ) -> std::io::Result<Self> {
        let mut command = agent.command();
        command
            .current_dir(root)
            .envs(env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        process::configure(&mut command);
        let mut process = command.spawn()?;
        let containment = match Containment::new(&process) {
            Ok(containment) => containment,
            Err(error) => {
                let _ = process.kill();
                std::thread::spawn(move || process.wait());
                return Err(error);
            }
        };

        let stdin = process.stdin.take().expect("stdin was piped");
        let stdout = process.stdout.take().expect("stdout was piped");
        let stderr = process.stderr.take().expect("stderr was piped");
        let state = Arc::new(Mutex::new(State::default()));
        let next = Arc::new(AtomicI64::new(FIRST_REQUEST));
        let (outbox, pending) = mpsc::channel();

        let measurement = Measurement {
            meter: Meter::of(agent, env),
            state: state.clone(),
            notify: notify.clone(),
            next: next.clone(),
            replies: Replies {
                outbox: outbox.clone(),
            },
            measuring: Arc::new(AtomicBool::new(false)),
            connected: Arc::new(AtomicBool::new(true)),
        };
        let session = Self {
            agent,
            root: root.to_path_buf(),
            process: Mutex::new(Some(process)),
            containment,
            outbox: outbox.clone(),
            state: state.clone(),
            next: next.clone(),
            notify: notify.clone(),
            limit_polling: Some(measurement.poll()),
        };
        if let Ok(mut state) = session.state.lock() {
            state.sent.insert(HANDSHAKE, Sent::Handshake);
            state.servers = opening.servers;
            state.fork = opening.fork;
            state.fork_message = opening.fork_message;
            state.resume = opening.resume;
            state.resume_fallback = opening.resume_fallback;
            state.quiet = opening.quiet;
            state.login_first = opening.login_first;
        }
        session.send(json!({
            "jsonrpc": "2.0",
            "id": HANDSHAKE,
            "method": "initialize",
            "params": handshake(),
        }));

        let reader = Reader {
            root: root.to_path_buf(),
            state: state.clone(),
            notify,
            replies: Replies { outbox },
            stdout: BufReader::new(stdout),
            next,
            tools: Tools::new(),
            shells: std::collections::BTreeMap::new(),
            subagents: Subagents::default(),
            ticket: 0,
            terminals: 0,
            measurement,
        };
        std::thread::spawn(move || write(stdin, &pending));
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

    /// Whether the adapter can fork this idle, open conversation natively.
    pub fn can_fork(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.forks && state.id.is_some() && !state.busy)
    }

    /// Whether an idle native fork can retain history through a selected message.
    pub fn can_fork_at(&self) -> bool {
        self.can_fork()
            && self
                .state
                .lock()
                .is_ok_and(|state| state.fork_points && state.loads)
    }

    /// Starts a separate process and asks it to fork the whole source conversation.
    ///
    /// Negotiation is repeated in the destination process; failure never opens a fresh session.
    pub fn fork(
        agent: Agent,
        root: &Path,
        env: &[(String, String)],
        source: &str,
        notify: Notify,
    ) -> std::io::Result<Self> {
        Self::open(
            agent,
            root,
            env,
            Opening {
                fork: Some(source.to_owned()),
                ..Opening::default()
            },
            notify,
        )
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
        self.send(request);
    }

    /// The tool servers this conversation was opened with, and which of them
    /// the agent could not be given.
    pub fn mcp_servers(&self) -> Vec<mcp::Offered> {
        self.state
            .lock()
            .map(|state| state.mcp.clone())
            .unwrap_or_default()
    }

    /// Whether this agent can forget a saved session.
    pub fn can_delete(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.deletes)
    }

    /// Asks the agent to forget the saved session `id`, which it says with
    /// [`Event::Deleted`] once it has.
    pub fn delete_session(&self, id: &str) {
        if !self.can_delete() {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let request = self.request(
            &mut state,
            Sent::Delete(id.to_owned()),
            "session/delete",
            &json!({ "sessionId": id }),
        );
        drop(state);
        self.send(request);
    }

    /// Whether this agent can be logged out.
    pub fn can_logout(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.logouts)
    }

    /// Logs the agent out, then opens a conversation again, which asks for a
    /// login where the agent wants one.
    pub fn logout(&self) {
        if !self.can_logout() {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let request = self.request(&mut state, Sent::Logout, "logout", &json!({}));
        drop(state);
        self.send(request);
    }

    /// Tells the agent the conversation is finished with, if it wants to be told.
    fn finish(&self) {
        let Ok(state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone().filter(|_| state.closes) else {
            return;
        };
        drop(state);
        self.send(json!({
            "jsonrpc": "2.0",
            "id": self.next.fetch_add(1, Ordering::Relaxed),
            "method": "session/close",
            "params": { "sessionId": id },
        }));
    }

    /// Sends `text` as the reader's next turn.
    ///
    /// A prompt sent before the conversation is open, or while the turn
    /// before it is still running, is held until it can go: a reader types
    /// when they have something to say, not when the agent is ready.
    ///
    /// Nothing of the prompt is built here: the files it embeds are read on
    /// the writer thread, and it is handed over while the state is still
    /// held, so no other message can be written ahead of it out of turn.
    pub fn prompt(&self, text: &str, attachments: Vec<Attachment>) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let prompt = Prompt {
            deferred: false,
            text: text.to_owned(),
            attachments,
        };
        let Some(session) = state.id.clone().filter(|_| !state.busy) else {
            state.queued.push(prompt);
            return;
        };
        state.busy = true;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        state.sent.insert(id, Sent::Turn);
        let _ = self.outbox.send(Outgoing::Turn {
            id,
            session,
            prompt,
            embeds: state.embeds,
        });
    }

    /// Queues an editor-owned follow-up for normal delivery after a successful turn.
    pub fn queue_prompt(&self, text: &str, limit: usize) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "The agent prompt queue is unavailable")?;
        if state.queued.len() >= limit {
            return Err("The target's pending message limit was reached".to_owned());
        }
        state.queued.push(Prompt {
            deferred: true,
            text: text.to_owned(),
            attachments: Vec::new(),
        });
        Ok(())
    }

    /// Discards pending prompts when cancellation or failure prevents safe delivery.
    pub fn clear_prompts(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.queued.clear();
            state
                .events
                .retain(|event| !matches!(event, Event::PromptReady(_)));
        }
    }

    /// Whether this agent has advertised image prompt support.
    pub fn can_image(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.images)
    }

    /// Stops the turn that is running, if one is.
    pub fn cancel(&self) {
        self.clear_prompts();
        let Ok(state) = self.state.lock() else {
            return;
        };
        let Some(id) = state.id.clone() else {
            return;
        };
        drop(state);
        self.send(json!({
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

    /// Answers the elicitation `ticket` was raised under with `reply`.
    pub fn reply(&self, ticket: u64, reply: &Reply) {
        self.answer(ticket, &reply.wire());
    }

    /// Answers the file or terminal request `ticket` was raised under.
    ///
    /// A request is answered once; an answer to one that is no longer owed
    /// is dropped. The answer is shaped and written on the writer thread, so
    /// a whole file or a terminal's output costs the caller nothing to hand
    /// back.
    pub fn answer_request(&self, ticket: u64, answer: Answer) {
        if let Some((owed, limit)) = self.take_owed(ticket) {
            let _ = self.outbox.send(Outgoing::Answer {
                owed: Box::new(owed),
                limit,
                answer,
            });
        }
    }

    /// Answers the request raised under `ticket` with what `answer` comes
    /// to, worked out on a thread of its own so that reading or writing a
    /// file for the agent never holds the window up.
    pub fn answer_request_later(
        &self,
        ticket: u64,
        answer: impl FnOnce() -> Answer + Send + 'static,
    ) {
        let Some((owed, limit)) = self.take_owed(ticket) else {
            return;
        };
        let outbox = self.outbox.clone();
        std::thread::spawn(move || {
            let _ = outbox.send(Outgoing::Answer {
                owed: Box::new(owed),
                limit,
                answer: answer(),
            });
        });
    }

    /// Takes the request raised under `ticket` off what the agent is owed,
    /// with the most output its terminal said it wants back, if it is still
    /// owed at all.
    fn take_owed(&self, ticket: u64) -> Option<(Owed, Option<usize>)> {
        let mut state = self.state.lock().ok()?;
        let owed = state.owed.remove(&ticket)?;
        let limit = match &owed.request {
            Request::Output { terminal } => state.limits.get(terminal).copied(),
            Request::Release { terminal } => state.limits.remove(terminal),
            _ => None,
        };
        Some((owed, limit))
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
        self.send(json!({ "jsonrpc": "2.0", "id": id, "result": outcome }));
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
        self.send(request);
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
        self.send(request);
        (self.notify)();
    }

    /// Whether any model or mode option requests still await their adapter response.
    pub fn is_configuring(&self) -> bool {
        self.state.lock().is_ok_and(|state| {
            state
                .sent
                .values()
                .any(|sent| matches!(sent, Sent::Knob(_) | Sent::Mode(_)))
        })
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
        self.send(request);
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
        self.process.lock().is_ok_and(|mut process| {
            process
                .as_mut()
                .is_some_and(|process| matches!(process.try_wait(), Ok(None)))
        })
    }

    /// One request, numbered and taken down as sent.
    fn request(&self, state: &mut State, sent: Sent, method: &str, params: &Value) -> Value {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        state.sent.insert(id, sent);
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    /// Hands one message to the writer, dropping it if the writer has gone.
    fn send(&self, message: Value) {
        let _ = self.outbox.send(Outgoing::Message(message));
    }
}

impl Drop for Session {
    /// Ends the agent's process group when the session is closed.
    ///
    /// The writer closes stdin after cancellation; a worker gives the group
    /// a short grace period before killing it and reaping the direct child.
    fn drop(&mut self) {
        self.limit_polling.take();
        self.cancel();
        self.finish();
        let (closed, closing) = mpsc::channel();
        let _ = self.outbox.send(Outgoing::Close { closed });
        let Some(mut process) = self
            .process
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        else {
            return;
        };
        let containment = std::mem::replace(&mut self.containment, Containment::empty());
        process::finish(std::thread::spawn(move || {
            let _ = closing.recv_timeout(Duration::from_millis(100));
            containment.terminate();
            std::thread::sleep(END_WITHIN);
            containment.kill();
            let _ = process.kill();
            let _ = process.wait();
        }));
    }
}

/// Writes what is handed over in `pending` to the agent's `stdin`, in order,
/// until every sender has gone or the pipe has.
fn write(mut stdin: ChildStdin, pending: &Receiver<Outgoing>) {
    for outgoing in pending {
        if let Outgoing::Close { closed } = outgoing {
            drop(stdin);
            let _ = closed.send(());
            return;
        }
        if transport::write(&mut stdin, &outgoing.build()).is_err() {
            return;
        }
    }
}

/// Where the reader thread hands its own messages to the writer.
#[derive(Clone)]
struct Replies {
    /// The same way to the writer thread the session hands its messages to.
    outbox: Sender<Outgoing>,
}

impl Replies {
    /// Hands one message to the writer, dropping it if the writer has gone.
    fn send(&self, message: Value) {
        let _ = self.outbox.send(Outgoing::Message(message));
    }
}

/// The shared account-limit refresh seam for reader events and periodic polling.
#[derive(Clone)]
struct Measurement {
    /// Where this agent's plan limits come from.
    meter: Meter,
    /// The conversation receiving the measurements.
    state: Arc<Mutex<State>>,
    /// Wakes the window when new limits arrive.
    notify: Notify,
    /// Numbers requests alongside the session and reader.
    next: Arc<AtomicI64>,
    /// Sends requests through the agent's writer thread.
    replies: Replies,
    /// Prevents overlapping reads from disk or network.
    measuring: Arc<AtomicBool>,
    /// Whether the agent's reader is still connected.
    connected: Arc<AtomicBool>,
}

impl Measurement {
    /// Starts periodic refreshes, stopping when the returned sender is dropped.
    fn poll(&self) -> Sender<()> {
        let (keep, stopped) = mpsc::channel();
        if self.meter.asks().is_some() || self.meter.reads().is_some() {
            let measurement = self.clone();
            std::thread::spawn(move || {
                while matches!(
                    stopped.recv_timeout(LIMIT_REFRESH),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    if !measurement.connected.load(Ordering::Acquire) {
                        break;
                    }
                    measurement.refresh();
                }
            });
        }
        keep
    }

    /// Reads account limits off the UI thread or requests them through the agent.
    fn refresh(&self) {
        if !self.connected.load(Ordering::Acquire) {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.id.is_none() {
            return;
        }
        if let Some(method) = self.meter.asks() {
            if state.sent.values().any(|sent| matches!(sent, Sent::Limits)) {
                return;
            }
            let id = self.next.fetch_add(1, Ordering::Relaxed);
            state.sent.insert(id, Sent::Limits);
            self.replies.send(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": {},
            }));
        }
        drop(state);
        let Some(read) = self.meter.reads() else {
            return;
        };
        if self.measuring.swap(true, Ordering::AcqRel) {
            return;
        }
        let state = self.state.clone();
        let notify = self.notify.clone();
        let measuring = self.measuring.clone();
        std::thread::spawn(move || {
            if let Some(limits) = read() {
                raise(&state, &notify, Event::Limited(limits));
            }
            measuring.store(false, Ordering::Release);
        });
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
    /// The tool calls of this conversation, as they now stand.
    tools: Tools,
    /// Background shell handles and the commands that launched them.
    shells: std::collections::BTreeMap<String, String>,
    /// Child session lifetimes and the cards receiving their updates.
    subagents: Subagents,
    /// The ticket the next request will be put to the reader or the window
    /// as.
    ticket: u64,
    /// How many terminals the agent has started, which is what names the
    /// next one.
    terminals: u64,
    /// Refreshes account limits at session opening, turn completion and every minute.
    measurement: Measurement,
}

impl Reader {
    /// Reads until the agent stops talking, and says so when it has.
    fn run(mut self) {
        while let Ok(Some(message)) = transport::read(&mut self.stdout) {
            self.dispatch(&message);
        }
        self.measurement.connected.store(false, Ordering::Release);
        for event in update::halt(&mut self.tools, None, Status::Disconnected) {
            self.raise(event);
        }
        self.raise(Event::Ended);
    }

    /// Acts on one message: a reply of the agent's, or a request of its own.
    ///
    /// An identity the editor did not hand out is passed back untouched
    /// rather than read: the agent numbers its own requests, and how it does
    /// so is its business.
    fn dispatch(&mut self, message: &Value) {
        let id = message.get("id").filter(|id| !id.is_null());
        match (id, message["method"].as_str()) {
            (Some(id), Some(method)) => self.serve(id, method, &message["params"]),
            (Some(id), None) => {
                if let Some(id) = id.as_i64() {
                    self.replied(id, message);
                }
            }
            (None, Some("session/update")) => self.updated(&message["params"]),
            (None, Some(method)) if method.starts_with("_x.ai/") => {
                self.extended(&message["params"])
            }
            (None, Some("elicitation/complete")) => {
                let id = message["params"]["elicitationId"].as_str();
                self.raise(Event::Concluded(id.unwrap_or_default().to_owned()));
            }
            (None, _) => {}
        }
    }

    /// Takes down the reply to one request, and sends what follows from it.
    fn replied(&mut self, id: i64, message: &Value) {
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
            (Sent::Fork(source), None) => {
                let result = &message["result"];
                match result["sessionId"]
                    .as_str()
                    .filter(|id| !id.is_empty() && *id != source)
                {
                    Some(destination) => self.activate_fork(destination, result),
                    None => self.raise(Event::Failed(
                        "the agent returned no independent fork identity".to_owned(),
                    )),
                }
            }
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
            (Sent::Open | Sent::Resume, Some(error)) if error["code"] == json!(LOGIN_REQUIRED) => {
                self.offer_login();
            }
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
            (Sent::Login, None) => self.open(),
            (Sent::Logout, None) => {
                if let Ok(mut state) = self.state.lock() {
                    state.id = None;
                    state.busy = false;
                    state.quiet = false;
                }
                self.raise(Event::LoggedOut);
                self.open();
            }
            (Sent::Logout, Some(error)) => self.raise(Event::Failed(complaint(error))),
            (Sent::Delete(id), None) => self.raise(Event::Deleted(id)),
            (Sent::Delete(_), Some(error)) => self.raise(Event::Failed(complaint(error))),
            (Sent::Turn, None) => {
                let stop = Stop::read(&message["result"]["stopReason"]);
                if stop == Stop::Cancelled {
                    for event in update::halt(&mut self.tools, None, Status::Cancelled) {
                        self.raise(event);
                    }
                }
                if stop != Stop::EndTurn
                    && let Ok(mut state) = self.state.lock()
                {
                    state.queued.clear();
                }
                for event in update::outliving(&self.tools, &mut self.shells) {
                    self.raise(event);
                }
                self.raise(Event::Stopped(stop));
                self.idle();
                self.measurement.refresh();
            }
            (Sent::Turn, Some(error)) => {
                if let Ok(mut state) = self.state.lock() {
                    state.queued.clear();
                }
                if error["code"] == json!(LOGIN_REQUIRED) {
                    self.offer_login();
                } else {
                    self.raise(Event::Failed(complaint(error)));
                }
                self.idle();
                self.measurement.refresh();
            }
            (Sent::Limits, None) => {
                if let Some(limits) = self.measurement.meter.answered(&message["result"]) {
                    self.raise(Event::Limited(limits));
                }
            }
            (Sent::Limits, Some(_)) => {}
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
            (Sent::Mode(_), None) => self.wake(),
            (Sent::Knob(_), None) => {
                self.knobbed(update::knobs(&message["result"]["configOptions"]));
                self.wake();
            }
        }
    }

    /// Offers sign-in and discards queued prompts after an authentication failure.
    fn offer_login(&self) {
        let logins = self
            .state
            .lock()
            .map(|mut state| {
                state.queued.clear();
                state.busy = false;
                state.logins.clone()
            })
            .unwrap_or_default();
        self.raise(Event::Login(logins));
    }

    /// Takes down what the agent can do, and opens the conversation.
    fn shook(&self, result: &Value) {
        let logins = update::methods(&result["authMethods"]);
        let capabilities = &result["agentCapabilities"];
        let prompts = &capabilities["promptCapabilities"];
        if let Ok(mut state) = self.state.lock() {
            state.logins = logins;
            state.loads = capabilities["loadSession"] == json!(true);
            let sessions = &capabilities["sessionCapabilities"];
            state.lists = sessions["list"].is_object();
            state.forks = sessions["fork"].is_object();
            state.fork_points = claude_fork_points(&result["agentInfo"]);
            state.resumes = sessions["resume"].is_object();
            state.closes = sessions["close"].is_object();
            state.deletes = sessions["delete"].is_object();
            state.logouts = capabilities["auth"]["logout"].is_object();
            state.images = prompts["image"] == json!(true);
            state.embeds = prompts["embeddedContext"] == json!(true);
            state.transports = mcp::Transports::of(&capabilities["mcpCapabilities"]);
        }
        let login = self.state.lock().ok().and_then(|mut state| {
            std::mem::take(&mut state.login_first).then(|| state.logins.clone())
        });
        match login {
            Some(methods) => self.raise(Event::Login(methods)),
            None => self.open(),
        }
    }

    /// Opens the conversation: the one that was left, or a new one.
    fn open(&self) {
        let quiet = self.state.lock().is_ok_and(|state| state.quiet);
        let transports = self
            .state
            .lock()
            .map(|state| state.transports)
            .unwrap_or_default();
        let scoped = self
            .state
            .lock()
            .map(|state| state.servers.clone())
            .unwrap_or_default();
        let (servers, plan) = mcp::offer(transports, scoped);
        if let Ok(mut state) = self.state.lock() {
            state.mcp = plan;
        }
        let fork = self.state.lock().ok().and_then(|state| state.fork.clone());
        if let Some(source) = fork {
            if !self.state.lock().is_ok_and(|state| state.forks) {
                self.raise(Event::Failed(
                    "this agent does not advertise native session/fork".to_owned(),
                ));
                return;
            }
            let message = self
                .state
                .lock()
                .ok()
                .and_then(|state| state.fork_message.clone());
            let mut params = json!({
                "sessionId": source,
                "cwd": self.root,
                "mcpServers": servers,
            });
            if let Some(message) = message {
                if !self
                    .state
                    .lock()
                    .is_ok_and(|state| state.fork_points && state.loads)
                {
                    self.raise(Event::Failed(
                        "this agent cannot fork at a selected message".to_owned(),
                    ));
                    return;
                }
                params["_meta"] = json!({"jetbrains": {"air": {"fork": {
                    "version": 1,
                    "messageId": message,
                }}}});
            }
            self.ask(Sent::Fork(source.clone()), "session/fork", &params);
            return;
        }
        let resumed = match self.state.lock() {
            Ok(state) => state.resume.clone().filter(|_| {
                state.loads || ((state.quiet || !state.resume_fallback) && state.resumes)
            }),
            Err(_) => None,
        };
        let resumes = self.state.lock().is_ok_and(|state| state.resumes);
        match resumed {
            Some(resumed) => self.ask(
                Sent::Resume,
                match resumes && (quiet || !self.state.lock().is_ok_and(|state| state.loads)) {
                    true => "session/resume",
                    false => "session/load",
                },
                &json!({
                    "sessionId": resumed,
                    "cwd": self.root,
                    "mcpServers": servers,
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
            None => {
                if quiet {
                    self.raise(Event::Fresh);
                }
                self.ask(
                    Sent::Open,
                    "session/new",
                    &json!({ "cwd": self.root, "mcpServers": servers }),
                )
            }
        }
    }

    /// Attaches the fork's destination before announcing readiness or releasing prompts.
    ///
    /// Adapters may return a saved, detached fork. Resume it where supported, or
    /// load it when replay is the only attachment contract; never open a replacement.
    fn activate_fork(&self, destination: &str, result: &Value) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.fork = None;
        let attach = state.resumes || state.loads;
        if attach {
            state.resume = Some(destination.to_owned());
            state.resume_fallback = false;
            state.quiet = !state.loads;
        }
        drop(state);
        match attach {
            true => self.open(),
            false => self.opened(result),
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
            .send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
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
        self.measurement.refresh();
    }

    /// Lets the next prompt that was held back go, if one was.
    ///
    /// Like a prompt the window sends, it is handed to the writer while the
    /// state is held, so the turn it opens is the one written next.
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
        if prompt.deferred {
            state.events.push(Event::PromptReady(prompt.text));
            state.fresh = true;
            drop(state);
            self.wake();
            return;
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        state.busy = true;
        state.sent.insert(id, Sent::Turn);
        let _ = self.replies.outbox.send(Outgoing::Turn {
            id,
            session,
            prompt,
            embeds: state.embeds,
        });
    }

    /// Takes down one thing the agent said during a turn.
    ///
    /// The update is read before the state is taken, which is held only for
    /// as long as it takes to add what it came to.
    fn updated(&mut self, params: &Value) {
        let mut events = self.subagents.events(params, &mut self.tools);
        events.extend(update::background(&params["update"], &mut self.shells));
        self.deliver(events);
    }

    /// Takes down what an agent says of its background work in a notice of
    /// its own, outside the protocol's updates.
    fn extended(&mut self, params: &Value) {
        let events = update::background(&params["update"], &mut self.shells);
        self.deliver(events);
    }

    /// Hands what was read to the window, and wakes it.
    fn deliver(&mut self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        for event in events {
            match &event {
                Event::Mode(mode) => state.mode = Some(mode.clone()),
                Event::Knobs(knobs) => state.knobs = knobs.clone(),
                _ => {}
            }
            state.events.push(event);
        }
        state.fresh = true;
        drop(state);
        self.wake();
    }

    /// Answers one request of the agent's.
    ///
    /// A request the editor does not serve is refused rather than ignored: an
    /// agent left waiting for a reply it asked for stops saying anything at
    /// all, which reads as a hung session rather than a missing feature.
    fn serve(&mut self, id: &Value, method: &str, params: &Value) {
        match method {
            "session/request_permission" => self.park(id, params),
            "elicitation/create" => self.question(id, params),
            "fs/read_text_file" | "fs/write_text_file" => self.owe(id, method, params),
            method if method.starts_with("terminal/") => self.owe(id, method, params),
            _ => self.refuse(id, NO_SUCH_METHOD, method),
        }
    }

    /// Hands a file or terminal request to the window and leaves it owed.
    ///
    /// A request that cannot be read — a path outside the worktree, a run
    /// with no command — is refused here, and never reaches the window. It is
    /// read before the state is taken, which is held only to leave it owed.
    fn owe(&mut self, id: &Value, method: &str, params: &Value) {
        let terminal = format!("terminal-{}", self.terminals + 1);
        let (request, shape) = match request::read(&self.root, method, params, terminal) {
            Ok(read) => read,
            Err(trouble) => {
                let code = match trouble == method {
                    true => NO_SUCH_METHOD,
                    false => INVALID,
                };
                return self.refuse(id, code, &trouble);
            }
        };
        let ticket = self.ticket;
        self.ticket += 1;
        if matches!(request, Request::Run(_)) {
            self.terminals += 1;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if let (Request::Run(run), Shape::Tail { limit: Some(limit) }) = (&request, &shape) {
            state.limits.insert(run.terminal.clone(), *limit);
        }
        state.owed.insert(
            ticket,
            Owed {
                id: id.clone(),
                request: request.clone(),
                shape,
            },
        );
        state.events.push(Event::Requested(ticket, request));
        state.fresh = true;
        drop(state);
        self.wake();
    }

    /// Puts a permission request to the reader and leaves it unanswered.
    ///
    /// The agent is waiting on this reply, and so it should be: the request
    /// is a question, and a question answered by the editor on the reader's
    /// behalf is a permission that was never asked for.
    ///
    /// The request is read before the state is taken, which is held only to
    /// leave it parked.
    fn park(&mut self, id: &Value, params: &Value) {
        let ticket = self.ticket;
        let mut params = params.clone();
        if let Some(session) = params["sessionId"].as_str().map(str::to_owned) {
            self.subagents.parent(&session, &mut params["toolCall"]);
        }
        let Some(ask) = update::ask(ticket, &params, &mut self.tools) else {
            self.answer(id, &json!({ "outcome": { "outcome": "cancelled" } }));
            return;
        };
        self.ticket += 1;
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.parked.insert(ticket, id.clone());
        state.events.push(Event::Asked(ask));
        state.fresh = true;
        drop(state);
        self.wake();
    }

    /// Puts an elicitation to the reader and leaves it unanswered.
    ///
    /// Like a permission, it is parked until the reader replies: the agent
    /// asked because it cannot go on without. One that cannot be read is
    /// refused here and never reaches the window.
    fn question(&mut self, id: &Value, params: &Value) {
        let ticket = self.ticket;
        let elicitation = match elicitation::read(ticket, params) {
            Ok(elicitation) => elicitation,
            Err(trouble) => return self.refuse(id, INVALID, &trouble),
        };
        self.ticket += 1;
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.parked.insert(ticket, id.clone());
        state.events.push(Event::Elicited(elicitation));
        state.fresh = true;
        drop(state);
        self.wake();
    }

    /// Replies to a request of the agent's.
    fn answer(&self, id: &Value, result: &Value) {
        self.replies
            .send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    /// Refuses a request of the agent's.
    fn refuse(&self, id: &Value, code: i64, message: &str) {
        self.replies.send(json!({
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
        raise(&self.state, &self.notify, event);
    }

    /// Wakes the window.
    fn wake(&self) {
        (self.notify)();
    }
}

/// Adds `event` to what the window has yet to see in `state`, and wakes it
/// through `notify`.
fn raise(state: &Mutex<State>, notify: &Notify, event: Event) {
    if let Ok(mut state) = state.lock() {
        state.events.push(event);
        state.fresh = true;
    }
    notify();
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
            "terminal": true,
            "auth": { "terminal": true },
            "elicitation": { "form": {}, "url": {} },
            "subagents": {},
            "_meta": { "jetbrains": { "air": { "version": 1, "capabilities": ["asyncTasks"] } } },
        },
    })
}

/// What an agent's error says, in one line.
fn complaint(error: &Value) -> String {
    error["message"]
        .as_str()
        .unwrap_or("the agent refused")
        .to_owned()
}

/// Recognizes Claude adapter 0.71.0 and later, which implement AIR fork points.
fn claude_fork_points(info: &Value) -> bool {
    if info["name"].as_str() != Some("@agentclientprotocol/claude-agent-acp") {
        return false;
    }
    let Some(version) = info["version"]
        .as_str()
        .filter(|version| !version.contains('-'))
    else {
        return false;
    };
    let numbers: Option<Vec<u64>> = version.split('.').map(|part| part.parse().ok()).collect();
    numbers.is_some_and(|numbers| numbers.len() == 3 && numbers.as_slice() >= [0, 71, 0].as_slice())
}
