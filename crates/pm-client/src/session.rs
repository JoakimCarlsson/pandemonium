//! One connection to a server that is dialled again when it drops.

use std::collections::{HashMap, HashSet};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use pm_protocol::{ClientMessage, Request, Response, ServerMessage, Target, VERSION};
use tungstenite::{Message, WebSocket};

/// How long a dial may take.
const DIAL: Duration = Duration::from_secs(5);

/// How long the connection waits for a message before it looks at what is to be sent.
const TICK: Duration = Duration::from_millis(25);

/// How long to wait before dialling again.
const RETRY: Duration = Duration::from_secs(1);

/// What the connection reports.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// The connection is up and the server let it in.
    Connected,
    /// The connection is down, for this reason, and will be dialled again.
    Disconnected(String),
    /// The answer to the request with this id.
    Response {
        /// The request.
        id: u64,
        /// The answer.
        response: Response,
    },
    /// Another client started a session.
    SessionAdded(pm_protocol::SessionInfo),
    /// Another client closed the session with this number.
    SessionClosed(u64),
    /// Another client changed the projects; these are the projects now.
    ProjectsChanged(Vec<String>),
    /// Something a shell printed, which has not been reported before.
    TerminalOutput {
        /// The shell.
        terminal: u64,
        /// The bytes.
        data: Vec<u8>,
    },
    /// The shell with this number ended.
    TerminalExited(u64),
    /// What a language server finds wrong with a file this client opened.
    Diagnostics {
        /// The folder the file was opened in.
        directory: String,
        /// The file, from the root of the folder.
        path: String,
        /// The version of the text they are about, when the server says.
        version: Option<i32>,
        /// What is wrong.
        diagnostics: Vec<pm_protocol::Diagnostic>,
    },
    /// A program debugged on the server moved on.
    Debug {
        /// The debugging session.
        debug: u64,
        /// What it looks like now.
        view: pm_protocol::DebugView,
        /// What happened since the last message.
        events: Vec<pm_protocol::DebugEvent>,
    },
    /// The languages changed; these are all of them now.
    ExtensionsChanged(Vec<pm_protocol::ExtensionInfo>),
    /// The files of a folder this client watches changed; this is the folder, as the client named it.
    FolderChanged(String),
    /// Something an agent did, numbered within its session.
    Event {
        /// The session.
        session: u64,
        /// The number.
        seq: u64,
        /// What happened.
        event: pm_acp::Event,
    },
}

/// What the owner asks of the connection thread.
enum Order {
    /// Sends the request under this id.
    Send(u64, Request),
    /// Watches the session, which has events numbered up to the second number already.
    Watch(u64, u64),
    /// Watches the shell, whose output up to the second number, in bytes, the caller already has.
    WatchTerminal(u64, u64),
}

/// What the connection thread remembers across reconnections.
#[derive(Default)]
struct Memory {
    /// The number of the last event heard of each session.
    seen: HashMap<u64, u64>,
    /// The sessions being watched.
    watched: HashSet<u64>,
    /// How many bytes of output have been heard of each shell.
    heard: HashMap<u64, u64>,
    /// The shells being watched.
    shells: HashSet<u64>,
    /// The requests sent that have no answer yet.
    waiting: Vec<(u64, Request)>,
}

/// A connection to a server.
pub struct Client {
    /// Orders for the connection thread.
    orders: Sender<Order>,
    /// What the connection thread reports.
    incoming: Receiver<Incoming>,
    /// The id the next request gets.
    next: AtomicU64,
}

impl Client {
    /// Connects to `target` on a thread of its own, and keeps the connection up.
    pub fn connect(target: Target) -> Self {
        let (orders, ordered) = mpsc::channel();
        let (reporter, incoming) = mpsc::channel();
        std::thread::spawn(move || keep(&target, &ordered, &reporter));
        Self {
            orders,
            incoming,
            next: AtomicU64::new(1),
        }
    }

    /// Sends `request` and returns the id its answer will carry.
    pub fn request(&self, request: Request) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let _ = self.orders.send(Order::Send(id, request));
        id
    }

    /// Watches `session`, whose events up to `after` the caller already has, across reconnections.
    pub fn watch(&self, session: u64, after: u64) {
        let _ = self.orders.send(Order::Watch(session, after));
    }

    /// Watches the shell `terminal`, whose output up to the byte `after` the caller already has, across reconnections.
    pub fn watch_terminal(&self, terminal: u64, after: u64) {
        let _ = self.orders.send(Order::WatchTerminal(terminal, after));
    }

    /// What the connection has reported since the last look.
    pub fn incoming(&self) -> Vec<Incoming> {
        let mut found = Vec::new();
        while let Ok(item) = self.incoming.try_recv() {
            found.push(item);
        }
        found
    }
}

/// Sends `message` to the server as one frame of text.
fn send(socket: &mut WebSocket<TcpStream>, message: &ClientMessage) -> Result<(), String> {
    let text = serde_json::to_string(message).map_err(|error| error.to_string())?;
    socket
        .send(Message::text(text))
        .map_err(|error| error.to_string())
}

/// Whether `error` only says that nothing arrived in time.
fn is_quiet(error: &tungstenite::Error) -> bool {
    matches!(
        error,
        tungstenite::Error::Io(io)
            if matches!(io.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
    )
}

/// Dials the server and says hello, returning the connection once the server has let it in.
fn dial(target: &Target) -> Result<WebSocket<TcpStream>, String> {
    let address = target
        .address
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or("the address names no machine")?;
    let stream = TcpStream::connect_timeout(&address, DIAL).map_err(|error| error.to_string())?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(DIAL));
    let url = format!("ws://{}/", target.address);
    let (mut socket, _) = tungstenite::client(url, stream).map_err(|error| error.to_string())?;
    send(
        &mut socket,
        &ClientMessage::Hello {
            version: VERSION,
            token: Some(target.token.clone()),
        },
    )?;
    loop {
        match socket.read() {
            Ok(Message::Text(text)) => {
                return match serde_json::from_str::<ServerMessage>(&text) {
                    Ok(ServerMessage::Ready { .. }) => {
                        let _ = socket.get_mut().set_read_timeout(Some(TICK));
                        Ok(socket)
                    }
                    Ok(ServerMessage::Denied { reason }) => Err(reason),
                    _ => Err("the server did not say it was ready".to_owned()),
                };
            }
            Ok(_) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Keeps a connection to `target`, passing orders to the server and what it says to `reporter`, until the client is dropped.
fn keep(target: &Target, ordered: &Receiver<Order>, reporter: &Sender<Incoming>) {
    let mut memory = Memory::default();
    loop {
        match dial(target) {
            Ok(mut socket) => {
                let _ = reporter.send(Incoming::Connected);
                let gone = converse(&mut socket, ordered, reporter, &mut memory);
                match gone {
                    Some(reason) => {
                        for (id, _) in memory.waiting.drain(..) {
                            let _ = reporter.send(Incoming::Response {
                                id,
                                response: Response::Failed {
                                    message: "the connection was lost".to_owned(),
                                },
                            });
                        }
                        let _ = reporter.send(Incoming::Disconnected(reason));
                    }
                    None => return,
                }
            }
            Err(reason) => {
                let _ = reporter.send(Incoming::Disconnected(reason));
            }
        }
        loop {
            match ordered.recv_timeout(RETRY) {
                Ok(Order::Watch(session, after)) => {
                    memory.watched.insert(session);
                    memory.seen.entry(session).or_insert(after);
                }
                Ok(Order::WatchTerminal(terminal, after)) => {
                    memory.shells.insert(terminal);
                    memory.heard.entry(terminal).or_insert(after);
                }
                Ok(Order::Send(id, _)) => {
                    let _ = reporter.send(Incoming::Response {
                        id,
                        response: Response::Failed {
                            message: "the server cannot be reached".to_owned(),
                        },
                    });
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

/// Sends the request that subscribes to the shell `terminal` from the byte `after`.
fn attach_terminal(
    socket: &mut WebSocket<TcpStream>,
    terminal: u64,
    after: u64,
) -> Result<(), String> {
    send(
        socket,
        &ClientMessage::Request {
            id: 0,
            request: Request::AttachTerminal { terminal, after },
        },
    )
}

/// What of the output `data` that starts at the byte `offset` is new to a client that has heard `heard` bytes, and where it ends.
fn fresh_output(heard: u64, offset: u64, data: Vec<u8>) -> Option<(Vec<u8>, u64)> {
    let end = offset + data.len() as u64;
    if end <= heard {
        return None;
    }
    let skipped = heard.saturating_sub(offset) as usize;
    Some((data[skipped..].to_vec(), end))
}

/// Carries orders to the server and what it says back, until the connection fails, giving the reason, or the client is dropped, giving nothing.
fn converse(
    socket: &mut WebSocket<TcpStream>,
    ordered: &Receiver<Order>,
    reporter: &Sender<Incoming>,
    memory: &mut Memory,
) -> Option<String> {
    for terminal in memory.shells.iter() {
        let after = memory.heard.get(terminal).copied().unwrap_or(0);
        if let Err(reason) = attach_terminal(socket, *terminal, after) {
            return Some(reason);
        }
    }
    for session in memory.watched.iter() {
        let after = memory.seen.get(session).copied().unwrap_or(0);
        let message = ClientMessage::Request {
            id: 0,
            request: Request::Attach {
                session: *session,
                after,
            },
        };
        if let Err(reason) = send(socket, &message) {
            return Some(reason);
        }
    }
    loop {
        loop {
            match ordered.try_recv() {
                Ok(Order::Send(id, request)) => {
                    memory.waiting.push((id, request.clone()));
                    if let Err(reason) = send(socket, &ClientMessage::Request { id, request }) {
                        return Some(reason);
                    }
                }
                Ok(Order::WatchTerminal(terminal, after)) => {
                    let fresh = memory.shells.insert(terminal);
                    let from = *memory.heard.entry(terminal).or_insert(after);
                    if fresh && let Err(reason) = attach_terminal(socket, terminal, from) {
                        return Some(reason);
                    }
                }
                Ok(Order::Watch(session, after)) => {
                    let fresh = memory.watched.insert(session);
                    let from = *memory.seen.entry(session).or_insert(after);
                    if fresh {
                        let message = ClientMessage::Request {
                            id: 0,
                            request: Request::Attach {
                                session,
                                after: from,
                            },
                        };
                        if let Err(reason) = send(socket, &message) {
                            return Some(reason);
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return None,
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(message) = serde_json::from_str::<ServerMessage>(&text) else {
                    continue;
                };
                match message {
                    ServerMessage::Response { id, response } => {
                        memory.waiting.retain(|(held, _)| *held != id);
                        if id != 0 {
                            let _ = reporter.send(Incoming::Response { id, response });
                        }
                    }
                    ServerMessage::Event {
                        session,
                        seq,
                        event,
                    } => {
                        memory.watched.insert(session);
                        let last = memory.seen.entry(session).or_insert(0);
                        if seq > *last {
                            *last = seq;
                            let _ = reporter.send(Incoming::Event {
                                session,
                                seq,
                                event,
                            });
                        }
                    }
                    ServerMessage::TerminalOutput {
                        terminal,
                        offset,
                        data,
                    } => {
                        memory.shells.insert(terminal);
                        let heard = memory.heard.entry(terminal).or_insert(0);
                        if let Some((data, end)) = fresh_output(*heard, offset, data) {
                            *heard = end;
                            let _ = reporter.send(Incoming::TerminalOutput { terminal, data });
                        }
                    }
                    ServerMessage::TerminalExited { terminal } => {
                        memory.shells.remove(&terminal);
                        memory.heard.remove(&terminal);
                        let _ = reporter.send(Incoming::TerminalExited(terminal));
                    }
                    ServerMessage::SessionAdded { info } => {
                        let _ = reporter.send(Incoming::SessionAdded(info));
                    }
                    ServerMessage::SessionClosed { session } => {
                        let _ = reporter.send(Incoming::SessionClosed(session));
                    }
                    ServerMessage::ProjectsChanged { paths } => {
                        let _ = reporter.send(Incoming::ProjectsChanged(paths));
                    }
                    ServerMessage::Diagnostics {
                        directory,
                        path,
                        version,
                        diagnostics,
                    } => {
                        let _ = reporter.send(Incoming::Diagnostics {
                            directory,
                            path,
                            version,
                            diagnostics,
                        });
                    }
                    ServerMessage::Debug {
                        debug,
                        view,
                        events,
                    } => {
                        let _ = reporter.send(Incoming::Debug {
                            debug,
                            view,
                            events,
                        });
                    }
                    ServerMessage::ExtensionsChanged { extensions } => {
                        let _ = reporter.send(Incoming::ExtensionsChanged(extensions));
                    }
                    ServerMessage::FolderChanged { directory } => {
                        let _ = reporter.send(Incoming::FolderChanged(directory));
                    }
                    ServerMessage::Ready { .. } | ServerMessage::Denied { .. } => {}
                }
            }
            Ok(Message::Ping(payload)) => {
                let _ = socket.send(Message::Pong(payload));
            }
            Ok(Message::Close(_)) => return Some("the server closed the connection".to_owned()),
            Ok(_) => {}
            Err(error) if is_quiet(&error) => {}
            Err(error) => return Some(error.to_string()),
        }
    }
}
