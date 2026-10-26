//! One client's connection: the handshake, then requests in and messages out.

use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pm_protocol::{ClientMessage, ServerMessage, VERSION};
use tungstenite::protocol::frame::Frame;
use tungstenite::protocol::frame::coding::{Data, OpCode};
use tungstenite::{Message, WebSocket};

use crate::core::{Connection, Core};
use crate::handler::handle;

/// How long a client has to say hello after connecting.
const HELLO_WAIT: Duration = Duration::from_secs(5);

/// How long the connection waits for a message before it looks at what is to be sent.
const TICK: Duration = Duration::from_millis(25);

/// How long the connection waits for a message while it has been sending, so what comes next leaves at once.
const BRISK: Duration = Duration::from_millis(1);

/// How long after its last message the connection keeps waiting [`BRISK`]ly.
const BRISK_FOR: Duration = Duration::from_secs(1);

/// The most bytes sent in one frame, within the client's default 16 MiB frame limit.
const FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Sends one JSON text message, fragmenting large responses to fit the client's frame limit.
fn send(socket: &mut WebSocket<TcpStream>, message: &ServerMessage) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    if text.len() <= FRAME_BYTES {
        return socket.send(Message::text(text)).is_ok();
    }
    let bytes = tungstenite::Bytes::from(text);
    for start in (0..bytes.len()).step_by(FRAME_BYTES) {
        let end = (start + FRAME_BYTES).min(bytes.len());
        let opcode = if start == 0 {
            Data::Text
        } else {
            Data::Continue
        };
        let frame = Frame::message(
            bytes.slice(start..end),
            OpCode::Data(opcode),
            end == bytes.len(),
        );
        if socket.send(Message::Frame(frame)).is_err() {
            return false;
        }
    }
    true
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
    let mut last_sent: Option<Instant> = None;
    'serving: loop {
        while let Ok(message) = pending.try_recv() {
            if !send(&mut socket, &message) {
                break 'serving;
            }
            last_sent = Some(Instant::now());
        }
        let wait = match last_sent {
            Some(sent) if sent.elapsed() < BRISK_FOR => BRISK,
            _ => TICK,
        };
        let _ = socket.get_ref().set_read_timeout(Some(wait));
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

/// Regression coverage for messages larger than a client's frame limit.
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Large repository inspections arrive intact without changing the client's default size limits.
    #[test]
    fn sends_large_repository_inspection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let message = ServerMessage::Response {
            id: 1,
            response: pm_protocol::Response::Inspected {
                changes: vec![
                    serde_json::from_value(serde_json::json!({
                        "path": "results.txt",
                        "diff": "é".repeat(9 * 1024 * 1024),
                        "staged": false,
                    }))
                    .unwrap(),
                ],
                commits: None,
                branch: Some("main".into()),
                head: Some("89b6c4a".into()),
                tracking: Default::default(),
                operation: None,
                repositories: Vec::new(),
            },
        };
        let expected = message.clone();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            assert!(send(&mut socket, &message));
            assert!(send(
                &mut socket,
                &ServerMessage::ProjectsChanged { paths: Vec::new() }
            ));
        });
        let stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (mut socket, _) = tungstenite::client(format!("ws://{address}/"), stream).unwrap();
        let text = socket.read().unwrap().into_text().unwrap();
        assert_eq!(
            serde_json::from_str::<ServerMessage>(&text).unwrap(),
            expected
        );
        let text = socket.read().unwrap().into_text().unwrap();
        assert_eq!(
            serde_json::from_str::<ServerMessage>(&text).unwrap(),
            ServerMessage::ProjectsChanged { paths: Vec::new() }
        );
        server.join().unwrap();
    }
}
