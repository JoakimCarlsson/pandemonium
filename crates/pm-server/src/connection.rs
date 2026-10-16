//! One client's connection: the handshake, then requests in and messages out.

use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pm_protocol::{ClientMessage, ServerMessage, VERSION};
use tungstenite::{Message, WebSocket};

use crate::core::{Connection, Core};
use crate::handler::handle;

/// How long a client has to say hello after connecting.
const HELLO_WAIT: Duration = Duration::from_secs(5);

/// How long the connection waits for a message before it looks at what is to be sent.
const TICK: Duration = Duration::from_millis(25);

/// Sends `message` to the client as one frame of text.
fn send(socket: &mut WebSocket<TcpStream>, message: &ServerMessage) -> bool {
    match serde_json::to_string(message) {
        Ok(text) => socket.send(Message::text(text)).is_ok(),
        Err(_) => false,
    }
}

/// Whether `error` only says that nothing arrived in time.
fn is_quiet(error: &tungstenite::Error) -> bool {
    matches!(
        error,
        tungstenite::Error::Io(io)
            if matches!(io.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
    )
}

/// Whether the two texts are the same, taking as long to say so whatever they hold.
fn same(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

/// Reads the first message of the connection and lets the client in when it is a hello with the right token.
fn greet(socket: &mut WebSocket<TcpStream>, token: &str) -> bool {
    let started = Instant::now();
    loop {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let hello = serde_json::from_str::<ClientMessage>(&text);
                return match hello {
                    Ok(ClientMessage::Hello {
                        version,
                        token: given,
                    }) => {
                        if version != VERSION {
                            let reason =
                                format!("the server speaks protocol {VERSION}, not {version}");
                            send(socket, &ServerMessage::Denied { reason });
                            false
                        } else if !given.as_deref().is_some_and(|given| same(given, token)) {
                            let reason = "the token is wrong".to_owned();
                            send(socket, &ServerMessage::Denied { reason });
                            false
                        } else {
                            send(
                                socket,
                                &ServerMessage::Ready {
                                    version: VERSION,
                                    server: env!("CARGO_PKG_VERSION").to_owned(),
                                },
                            )
                        }
                    }
                    _ => false,
                };
            }
            Ok(Message::Close(_)) => return false,
            Ok(_) => {}
            Err(error) if is_quiet(&error) => {
                if started.elapsed() > HELLO_WAIT {
                    return false;
                }
            }
            Err(_) => return false,
        }
    }
}

/// Serves the client on `stream` until it goes away.
pub(crate) fn serve(core: &Arc<Core>, stream: TcpStream, token: &str) {
    let _ = stream.set_read_timeout(Some(TICK));
    let _ = stream.set_nodelay(true);
    let Ok(mut socket) = tungstenite::accept(stream) else {
        return;
    };
    if !greet(&mut socket, token) {
        return;
    }
    let (outbox, pending) = mpsc::channel::<ServerMessage>();
    let connection = Arc::new(Connection {
        outbox,
        attached: Mutex::new(std::collections::HashSet::new()),
    });
    if let Ok(mut connections) = core.connections.lock() {
        connections.push(connection.clone());
    }
    'serving: loop {
        while let Ok(message) = pending.try_recv() {
            if !send(&mut socket, &message) {
                break 'serving;
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                if let Ok(ClientMessage::Request { id, request }) =
                    serde_json::from_str::<ClientMessage>(&text)
                {
                    let (core, connection) = (core.clone(), connection.clone());
                    std::thread::spawn(move || {
                        let response = handle(&core, &connection, request);
                        let _ = connection
                            .outbox
                            .send(ServerMessage::Response { id, response });
                    });
                }
            }
            Ok(Message::Ping(payload)) => {
                let _ = socket.send(Message::Pong(payload));
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(error) if is_quiet(&error) => {}
            Err(_) => break,
        }
    }
    if let Ok(mut connections) = core.connections.lock() {
        connections.retain(|held| !Arc::ptr_eq(held, &connection));
    }
    core.documents.forget(&connection);
    core.folders.forget(&connection);
}
