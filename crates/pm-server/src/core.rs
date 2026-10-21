//! What the server holds: the sessions and the connections watching them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use pm_acp::{Agent, Chat, Event};
use pm_protocol::{ServerMessage, SessionInfo};

use crate::debugs::Debugging;
use crate::documents::Documents;
use crate::folders::Folders;
use crate::languages::Installs;
use crate::queue::Waiting;
use crate::sessions::{self, Meta};
use crate::terminals::Terminal;

/// One client connected to the server.
pub(crate) struct Connection {
    /// Where messages for the client go.
    pub(crate) outbox: Sender<ServerMessage>,
    /// The sessions the client watches.
    pub(crate) attached: Mutex<HashSet<u64>>,
}

/// One session: an agent working in a folder, and everything it has done.
pub(crate) struct Session {
    /// The agent that runs it.
    pub(crate) agent: Agent,
    /// The folder it works in.
    pub(crate) directory: PathBuf,
    /// The running agent, or nothing when it stopped and has yet to be started again.
    pub(crate) chat: Option<Chat>,
    /// What the agent has done, numbered from one.
    pub(crate) log: Vec<(u64, Event)>,
    /// What the session is, apart from what its agent did.
    pub(crate) meta: Meta,
    /// Whether its agent is on a turn.
    pub(crate) busy: bool,
    /// The prompts waiting for the turn to end, the next one first.
    pub(crate) queue: VecDeque<Waiting>,
    /// The id the next waiting prompt gets.
    pub(crate) next_queued: u64,
}

impl Session {
    /// A session of `agent` working in `directory`, described by `meta`, that has done what `log` holds and has no agent running.
    pub(crate) fn new(
        agent: Agent,
        directory: PathBuf,
        log: Vec<(u64, Event)>,
        meta: Meta,
    ) -> Self {
        Self {
            agent,
            directory,
            chat: None,
            log,
            meta,
            busy: false,
            queue: VecDeque::new(),
            next_queued: 1,
        }
    }

    /// Starts the agent in the session's folder, without what the reader turned off, and gives it the settings the reader chose.
    pub(crate) fn start_agent(&mut self, settings: &[(String, String)]) {
        let (tools, launch) = crate::library::given(self.agent, &self.directory);
        let chat = Chat::start(self.agent, self.directory.clone(), tools, launch);
        for (setting, value) in settings {
            chat.set(setting, value);
        }
        self.chat = Some(chat);
    }

    /// The number the next event noted in the log of this session gets.
    pub(crate) fn next_seq(&self) -> u64 {
        self.log.last().map_or(0, |(seq, _)| *seq) + 1
    }

    /// Whether the log ends on a turn handed to the agent that never ended.
    pub(crate) fn turn_open(&self) -> bool {
        self.log.iter().fold(false, |open, (_, event)| match event {
            Event::Prompted { .. } => true,
            Event::Finished(_) | Event::Failed(_) => false,
            _ => open,
        })
    }

    /// Ends the turn the log leaves open, numbered `number`, as stopped for `reason`, and returns what it noted; nothing when no turn is open.
    pub(crate) fn end_open_turn(&mut self, number: u64, reason: &str) -> Told {
        if !self.turn_open() {
            return Told::new();
        }
        self.busy = false;
        [Event::Halted, Event::Finished(Some(reason.to_owned()))]
            .into_iter()
            .map(|event| (number, self.note(number, &event), event))
            .collect()
    }

    /// Whether `event` only says again what the last context usage in the log said.
    pub(crate) fn repeats(&self, event: &Event) -> bool {
        matches!(event, Event::Used(_))
            && self
                .log
                .iter()
                .rev()
                .find(|(_, held)| matches!(held, Event::Used(_)))
                .is_some_and(|(_, held)| held == event)
    }

    /// Adds `event` to the log of this session, numbered `number`, keeps it on disk and returns the number it got.
    pub(crate) fn note(&mut self, number: u64, event: &Event) -> u64 {
        let seq = self.next_seq();
        self.log.push((seq, event.clone()));
        sessions::append(number, seq, event);
        if let Event::Titled(title) = event {
            self.meta.title = title.clone();
            sessions::save_meta(number, &self.meta);
        }
        if matches!(
            event,
            Event::Prompted { .. } | Event::Finished(_) | Event::Failed(_)
        ) {
            self.meta.updated = now();
            sessions::save_meta(number, &self.meta);
        }
        seq
    }
}

/// Why a turn the server was working on when it stopped ended.
const INTERRUPTED: &str = "the server stopped while the agent was working";

/// Why a turn whose agent is no longer running ended when it was stopped.
pub(crate) const NOT_RUNNING: &str = "the agent was no longer running";

/// Events noted in a session's log and yet to be told to the clients: the session, the number and the event.
pub(crate) type Told = Vec<(u64, u64, Event)>;

/// Everything the server shares between its threads.
pub(crate) struct Core {
    /// The sessions, by number.
    pub(crate) sessions: Mutex<HashMap<u64, Session>>,
    /// The number the next session gets.
    next: AtomicU64,
    /// The clients that are connected.
    pub(crate) connections: Mutex<Vec<Arc<Connection>>>,
    /// The shells, by number.
    pub(crate) terminals: Mutex<HashMap<u64, Arc<Terminal>>>,
    /// The number the next shell gets.
    next_terminal: AtomicU64,
    /// The programs being debugged, by number.
    pub(crate) debugs: Mutex<HashMap<u64, Arc<Debugging>>>,
    /// The number the next debugging session gets.
    next_debug: AtomicU64,
    /// The files clients have open for their language servers.
    pub(crate) documents: Documents,
    /// What the server knows of installing languages.
    pub(crate) installs: Installs,
    /// The folders clients watch.
    pub(crate) folders: Folders,
}

impl Core {
    /// A server with no sessions and no clients.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            connections: Mutex::new(Vec::new()),
            terminals: Mutex::new(HashMap::new()),
            next_terminal: AtomicU64::new(1),
            debugs: Mutex::new(HashMap::new()),
            next_debug: AtomicU64::new(1),
            documents: Documents::new(),
            installs: Installs::default(),
            folders: Folders::new(),
        })
    }

    /// The number for a new session.
    pub(crate) fn next_session(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// The number for a new shell.
    pub(crate) fn next_terminal(&self) -> u64 {
        self.next_terminal.fetch_add(1, Ordering::Relaxed)
    }

    /// The number for a new debugging session.
    pub(crate) fn next_debug(&self) -> u64 {
        self.next_debug.fetch_add(1, Ordering::Relaxed)
    }

    /// Holds again the sessions kept on disk, without their agents, and numbers new ones after them.
    pub(crate) fn load(&self) {
        let mut highest = 0;
        if let Ok(mut sessions) = self.sessions.lock() {
            for (number, meta, log) in sessions::load_all() {
                let Some(agent) = crate::handler::agent_named(&meta.agent) else {
                    continue;
                };
                highest = highest.max(number);
                let mut session = Session::new(agent, PathBuf::from(&meta.directory), log, meta);
                session.forget_queue(number);
                session.end_open_turn(number, INTERRUPTED);
                sessions.insert(number, session);
            }
        }
        self.next.store(highest + 1, Ordering::Relaxed);
    }

    /// Sends `message` to every connected client except `except`.
    pub(crate) fn broadcast(&self, message: &ServerMessage, except: Option<&Arc<Connection>>) {
        let connections = self
            .connections
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default();
        for connection in connections {
            if except.is_some_and(|skipped| Arc::ptr_eq(skipped, &connection)) {
                continue;
            }
            let _ = connection.outbox.send(message.clone());
        }
    }

    /// What the server knows about the session numbered `number`.
    pub(crate) fn info(&self, number: u64) -> Option<SessionInfo> {
        self.infos().into_iter().find(|info| info.session == number)
    }

    /// What the server knows about each session, oldest first.
    pub(crate) fn infos(&self) -> Vec<SessionInfo> {
        let Ok(sessions) = self.sessions.lock() else {
            return Vec::new();
        };
        let mut infos: Vec<SessionInfo> = sessions
            .iter()
            .map(|(number, session)| SessionInfo {
                session: *number,
                agent: session.meta.agent.clone(),
                repository: session.meta.repository.clone(),
                directory: session.meta.directory.clone(),
                branch: session.meta.branch.clone(),
                head: session.meta.head.clone(),
                base: session.meta.base.clone(),
                title: session.meta.title.clone(),
                closed: session.meta.closed,
                updated: session.meta.updated,
                last_seq: session.log.last().map_or(0, |(seq, _)| *seq),
                running: session.chat.is_some(),
            })
            .collect();
        infos.sort_by_key(|info| info.session);
        infos
    }

    /// Tells the clients watching each session about the events noted in `told`.
    pub(crate) fn tell_all(&self, told: Told) {
        for (session, seq, event) in told {
            self.tell(session, seq, &event);
        }
    }

    /// Tells the clients watching `session` about `event`, numbered `seq`.
    fn tell(&self, session: u64, seq: u64, event: &Event) {
        let connections = self
            .connections
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default();
        for connection in connections {
            let watching = connection
                .attached
                .lock()
                .is_ok_and(|attached| attached.contains(&session));
            if watching {
                let _ = connection.outbox.send(ServerMessage::Event {
                    session,
                    seq,
                    event: event.clone(),
                });
            }
        }
    }

    /// Takes what the agents did since the last look into the sessions' logs, hands the agents that ended a turn what waits for them and tells the clients.
    pub(crate) fn pump(&self) {
        let mut told = Told::new();
        if let Ok(mut sessions) = self.sessions.lock() {
            for (number, session) in sessions.iter_mut() {
                let Some(chat) = &session.chat else {
                    continue;
                };
                for event in chat.events() {
                    if session.repeats(&event) {
                        continue;
                    }
                    told.push((*number, session.note(*number, &event), event.clone()));
                    match event {
                        Event::Failed(_) => {
                            session.chat = None;
                            session.busy = false;
                            told.extend(session.drop_queue(*number));
                        }
                        Event::Finished(_) => {
                            session.busy = false;
                            told.extend(session.advance(*number));
                        }
                        _ => {}
                    }
                }
            }
        }
        self.tell_all(told);
    }
}

/// The time now, in milliseconds since the epoch.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}
