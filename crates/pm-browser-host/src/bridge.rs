//! A WebSocket on the loopback address that lets the DevTools page, which is shown inside this program, talk to the page it inspects.
//!
//! The address carries a secret path made for each DevTools page, and only a
//! connection from the DevTools page's own origin is let in, so nothing else on
//! the machine can reach a page's protocol through it.

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Message, accept_hdr};

/// What a connection asks of the program's main thread, which alone may speak to the engine.
pub(crate) enum Wish {
    /// Send this message to the page `target`.
    Send {
        /// The page inspected.
        target: u32,
        /// What the DevTools page wrote.
        message: String,
    },
    /// A tool call on the page `page` waited long enough.
    Late {
        /// The page.
        page: u32,
        /// The call.
        call: u64,
    },
    /// Start telling `sender` what the page `target` says.
    Watch {
        /// The page inspected.
        target: u32,
        /// Where it goes.
        sender: Sender<String>,
    },
}

/// The listening socket and the secret paths it lets in.
pub(crate) struct Bridge {
    /// The port it listens on.
    pub(crate) port: u16,
    /// The page each secret path inspects.
    routes: Arc<Mutex<HashMap<String, u32>>>,
}

/// The origin the DevTools page speaks from.
const ORIGIN: &str = "devtools://devtools";

/// How long a connection waits for a message before it looks for something to send.
const TICK: Duration = Duration::from_millis(15);

impl Bridge {
    /// Starts listening on a free loopback port; what connections ask goes to `wishes`.
    pub(crate) fn start(wishes: Sender<Wish>) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let routes: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
        let held = routes.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (routes, wishes) = (held.clone(), wishes.clone());
                std::thread::spawn(move || serve(stream, &routes, &wishes));
            }
        });
        Ok(Self { port, routes })
    }

    /// Lets the path `secret` inspect the page `target`.
    pub(crate) fn allow(&self, secret: &str, target: u32) {
        if let Ok(mut routes) = self.routes.lock() {
            routes.insert(secret.to_owned(), target);
        }
    }

    /// Stops letting the path `secret` in.
    pub(crate) fn forbid(&self, secret: &str) {
        if let Ok(mut routes) = self.routes.lock() {
            routes.remove(secret);
        }
    }
}

/// A refusal.
fn refused() -> ErrorResponse {
    let mut response = ErrorResponse::new(None);
    *response.status_mut() = StatusCode::FORBIDDEN;
    response
}

/// Serves one connection until it ends.
///
/// The handshake callback has to return the library's own, large, refusal type.
#[allow(clippy::result_large_err)]
fn serve(stream: TcpStream, routes: &Arc<Mutex<HashMap<String, u32>>>, wishes: &Sender<Wish>) {
    let _ = stream.set_nodelay(true);
    let chosen: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let noted = chosen.clone();
    let accepted = accept_hdr(stream, move |request: &Request, response: Response| {
        let from_devtools = request
            .headers()
            .get("origin")
            .and_then(|origin| origin.to_str().ok())
            == Some(ORIGIN);
        let found = routes.lock().ok().and_then(|routes| {
            routes
                .get(request.uri().path().trim_start_matches('/'))
                .copied()
        });
        match (from_devtools, found) {
            (true, Some(page)) => {
                if let Ok(mut noted) = noted.lock() {
                    *noted = Some(page);
                }
                Ok(response)
            }
            _ => Err(refused()),
        }
    });
    let found = chosen.lock().ok().and_then(|chosen| *chosen);
    let (Ok(mut socket), Some(target)) = (accepted, found) else {
        return;
    };
    let (sender, receiver) = channel::<String>();
    if wishes.send(Wish::Watch { target, sender }).is_err() {
        return;
    }
    loop {
        let _ = socket.get_ref().set_read_timeout(Some(TICK));
        match socket.read() {
            Ok(Message::Text(text)) => {
                let message = text.to_string();
                if wishes.send(Wish::Send { target, message }).is_err() {
                    break;
                }
            }
            Ok(Message::Ping(payload)) => {
                let _ = socket.send(Message::Pong(payload));
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(io))
                if matches!(
                    io.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
        while let Ok(message) = receiver.try_recv() {
            if socket.send(Message::text(message)).is_err() {
                return;
            }
        }
    }
}
