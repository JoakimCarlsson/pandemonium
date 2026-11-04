//! One client's connection: the handshake, then requests in and messages out.

use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pm_protocol::{ClientMessage, ServerMessage, VERSION};
use tungstenite::protocol::frame::Frame;
use tungstenite::protocol::frame::coding::{Data, OpCode};
use tungstenite::{Message, WebSocket};

use crate::core::{Connection, Core, Outbox};
use crate::handler::handle;

/// How long a client has to say hello after connecting.
const HELLO_WAIT: Duration = Duration::from_secs(5);

/// How long the connection waits for a message before it looks at what is to be sent, while the client is greeting and has not yet been handed over to being woken.
const TICK: Duration = Duration::from_millis(25);

/// How long the connection waits before it tries again to write what the network would not take at once.
const FLUSH_RETRY: Duration = Duration::from_millis(2);

/// The poll token of the client's socket becoming readable.
const SOCKET: mio::Token = mio::Token(0);

/// The poll token of a message being queued for the client.
const QUEUED: mio::Token = mio::Token(1);

/// The most bytes sent in one frame, within the client's default 16 MiB frame limit.
const FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Sends one JSON text message, fragmenting large responses to fit the client's frame limit.
fn send(socket: &mut WebSocket<TcpStream>, message: &ServerMessage) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    if text.len() <= FRAME_BYTES {
        return accepted(socket.send(Message::text(text)));
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
        if !accepted(socket.send(Message::Frame(frame))) {
            return false;
        }
    }
    true
}

/// Whether sending went well: a network that will not take all of it yet is not a failure, since the rest is kept and written by the next flush.
fn accepted(sent: Result<(), tungstenite::Error>) -> bool {
    match sent {
        Ok(()) => true,
        Err(error) => is_quiet(&error),
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

/// Makes the socket of `socket` take no waiting, and gives the poll that wakes when the client has sent something, with the handle the poll watches, which must be kept for as long as the poll is.
fn prepare(socket: &WebSocket<TcpStream>) -> Option<(mio::Poll, mio::net::TcpStream)> {
    let stream = socket.get_ref();
    stream.set_nonblocking(true).ok()?;
    let mut watched = mio::net::TcpStream::from_std(stream.try_clone().ok()?);
    let poll = mio::Poll::new().ok()?;
    poll.registry()
        .register(&mut watched, SOCKET, mio::Interest::READABLE)
        .ok()?;
    Some((poll, watched))
}

/// Serves the client on `stream` until it goes away. The thread sleeps until the client says something or a message is queued for it.
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
    let Some((mut poll, _watched)) = prepare(&socket) else {
        return;
    };
    let connection = Arc::new(Connection {
        outbox: Outbox::new(outbox),
        attached: Mutex::new(std::collections::HashSet::new()),
    });
    if let Ok(waker) = mio::Waker::new(poll.registry(), QUEUED) {
        connection.outbox.wake_with(Arc::new(waker));
    }
    if let Ok(mut connections) = core.connections.lock() {
        connections.push(connection.clone());
    }
    let mut events = mio::Events::with_capacity(4);
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
            Err(error) if is_quiet(&error) => {
                let unsent = matches!(socket.flush(), Err(ref error) if is_quiet(error));
                let _ = poll.poll(&mut events, unsent.then_some(FLUSH_RETRY));
            }
            Err(_) => break,
        }
    }
    if let Ok(mut connections) = core.connections.lock() {
        connections.retain(|held| !Arc::ptr_eq(held, &connection));
    }
    crate::handler::release_queues(core, &connection);
    crate::browser_tools::forget(&connection);
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

    /// A request sent after a quiet spell is answered by the real connection without either side waiting for a tick.
    #[test]
    fn answers_a_request_after_a_quiet_spell_quickly() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let core = Core::new();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve(&core, stream, "token");
        });
        let client = pm_client::Client::connect(
            pm_protocol::Target {
                address,
                token: "token".to_owned(),
            },
            Arc::new(|| {}),
        );
        let mut times = Vec::new();
        for round in 0..41 {
            std::thread::sleep(Duration::from_millis(37));
            let started = Instant::now();
            let id = client.request(pm_protocol::Request::Choices);
            'waiting: loop {
                for item in client.incoming() {
                    if matches!(item, pm_client::Incoming::Response { id: held, .. } if held == id)
                    {
                        break 'waiting;
                    }
                }
                std::thread::sleep(Duration::from_micros(50));
            }
            if round > 0 {
                times.push(started.elapsed().as_secs_f64() * 1000.0);
            }
        }
        times.sort_by(f64::total_cmp);
        eprintln!(
            "request to answer through the server: p50={:.2} p95={:.2} max={:.2} ms",
            times[20], times[37], times[39]
        );
        assert!(times[20] < 5.0, "the median was {:.2} ms", times[20]);
    }
}
