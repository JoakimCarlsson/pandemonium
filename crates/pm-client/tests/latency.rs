//! How long a request takes to reach a server and its answer to come back, over a loopback connection.

use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pm_client::{Client, Incoming};
use pm_protocol::{ClientMessage, Request, Response, ServerMessage, Target, VERSION};
use tungstenite::Message;

/// Serves one connection: lets the client in, then answers every request at once.
fn serve(listener: TcpListener) {
    let (stream, _) = listener.accept().expect("a client connects");
    let _ = stream.set_nodelay(true);
    let mut socket = tungstenite::accept(stream).expect("the handshake completes");
    let reply = |socket: &mut tungstenite::WebSocket<std::net::TcpStream>,
                 message: ServerMessage| {
        let text = serde_json::to_string(&message).expect("a message serializes");
        socket
            .send(Message::text(text))
            .expect("the client listens");
    };
    while let Ok(Message::Text(text)) = socket.read() {
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Hello { .. }) => reply(
                &mut socket,
                ServerMessage::Ready {
                    version: VERSION,
                    server: "test".to_owned(),
                },
            ),
            Ok(ClientMessage::Request { id, .. }) => reply(
                &mut socket,
                ServerMessage::Response {
                    id,
                    response: Response::Done,
                },
            ),
            _ => {}
        }
    }
}

/// Waits for the answer to `id`, and says how long it took to arrive.
fn answered(client: &Client, id: u64, since: Instant) -> Duration {
    loop {
        for item in client.incoming() {
            if matches!(item, Incoming::Response { id: held, .. } if held == id) {
                return since.elapsed();
            }
        }
        std::thread::sleep(Duration::from_micros(50));
    }
}

/// A request sent while the connection is quiet is answered without waiting for the connection to look at its orders.
#[test]
fn a_request_after_a_quiet_spell_is_answered_quickly() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port is free");
    let address = listener
        .local_addr()
        .expect("the port is known")
        .to_string();
    std::thread::spawn(move || serve(listener));
    let client = Client::connect(
        Target {
            address,
            token: String::new(),
        },
        Arc::new(|| {}),
    );
    let first = client.request(Request::Projects);
    answered(&client, first, Instant::now());
    let mut times = Vec::new();
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(37));
        let started = Instant::now();
        let id = client.request(Request::Projects);
        times.push(answered(&client, id, started).as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    eprintln!(
        "request to answer after a quiet spell: p50={:.2} p95={:.2} max={:.2} ms",
        times[20], times[37], times[39]
    );
    assert!(times[20] < 5.0, "the median was {:.2} ms", times[20]);
}
