//! What the server holds: the sessions and the connections watching them.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use pm_acp::{Agent, Chat, Event};
use pm_protocol::{ServerMessage, SessionInfo};

use crate::sessions::{self, Meta};

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
}

/// Everything the server shares between its threads.
pub(crate) struct Core {
    /// The sessions, by number.
    pub(crate) sessions: Mutex<HashMap<u64, Session>>,
    /// The number the next session gets.
    next: AtomicU64,
    /// The clients that are connected.
    pub(crate) connections: Mutex<Vec<Arc<Connection>>>,
}

impl Core {
    /// A server with no sessions and no clients.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            connections: Mutex::new(Vec::new()),
        })
    }

    /// The number for a new session.
    pub(crate) fn next_session(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    /// Holds again the sessions kept on disk, without their agents, and numbers new ones after them.
    pub(crate) fn load(&self) {
        let mut highest = 0;
        if let Ok(mut sessions) = self.sessions.lock() {
            for (number, meta, log) in sessions::load_all() {
                let Some(agent) = pm_acp::AGENTS
                    .iter()
                    .copied()
                    .find(|held| held.id == meta.agent)
                else {
                    continue;
                };
                highest = highest.max(number);
                sessions.insert(
                    number,
                    Session {
                        agent,
                        directory: PathBuf::from(&meta.directory),
                        chat: None,
                        log,
                        meta,
                    },
                );
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
                updated: session.meta.updated,
                last_seq: session.log.last().map_or(0, |(seq, _)| *seq),
                running: session.chat.is_some(),
            })
            .collect();
        infos.sort_by_key(|info| info.session);
        infos
    }

    /// Adds `event` to the log of the session numbered `number`, keeps it on disk and tells the clients watching the session.
    pub(crate) fn record(&self, number: u64, event: Event) {
        let seq = {
            let Ok(mut sessions) = self.sessions.lock() else {
                return;
            };
            let Some(session) = sessions.get_mut(&number) else {
                return;
            };
            let seq = session.log.last().map_or(0, |(seq, _)| *seq) + 1;
            session.log.push((seq, event.clone()));
            sessions::append(number, seq, &event);
            if matches!(
                event,
                Event::Prompted { .. } | Event::Finished(_) | Event::Failed(_)
            ) {
                session.meta.updated = now();
                sessions::save_meta(number, &session.meta);
            }
            seq
        };
        self.tell(number, seq, &event);
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

    /// Takes what the agents did since the last look into the sessions' logs and tells the clients.
    pub(crate) fn pump(&self) {
        let mut fresh: Vec<(u64, Event)> = Vec::new();
        if let Ok(mut sessions) = self.sessions.lock() {
            for (number, session) in sessions.iter_mut() {
                let Some(chat) = &session.chat else {
                    continue;
                };
                for event in chat.events() {
                    if matches!(event, Event::Failed(_)) {
                        session.chat = None;
                    }
                    fresh.push((*number, event));
                }
            }
        }
        for (number, event) in fresh {
            self.record(number, event);
        }
    }
}

/// The time now, in milliseconds since the epoch.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}
