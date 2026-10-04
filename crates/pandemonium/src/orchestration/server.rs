//! Authenticated loopback Streamable HTTP, handed to the window over a bounded queue.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use pm_acp::{McpServer, Reach};
use serde_json::{Value, json};

use super::tools;
use crate::agent::{McpFactory, TalkId};

/// Maximum concurrent transport requests, including requests awaiting creation.
const CONNECTIONS: usize = 16;
/// Maximum bytes in one incoming JSON message.
const BODY_LIMIT: usize = 32768;
/// The protocol versions understood by this stateless server.
const VERSIONS: [&str; 3] = ["2025-03-26", "2025-06-18", "2025-11-25"];

/// A caller-authenticated invocation or cancellation for the window.
pub(crate) struct Call {
    /// The conversation bound to this connection's secret.
    pub caller: TalkId,
    /// The JSON-RPC request identity.
    pub id: Value,
    /// The requested tool, or `notifications/cancelled`.
    pub name: String,
    /// The validated JSON object supplied by the client.
    pub arguments: Value,
    /// The response channel, absent for cancellation notifications.
    pub reply: Option<mpsc::SyncSender<Result<Value, String>>>,
    /// Whether transport timeout has made this invocation unusable.
    pub abandoned: Arc<AtomicBool>,
}

impl Call {
    /// Replies once without blocking when a disconnected client stopped reading.
    pub fn answer(&self, result: Result<Value, String>) {
        if let Some(reply) = self.reply.as_ref() {
            let _ = reply.try_send(result);
        }
    }
}

/// Shared transport state, independent of editor and GPU state.
struct Shared {
    /// Tokens identifying exactly one live editor conversation each.
    tokens: Mutex<BTreeMap<String, TalkId>>,
    /// The bounded queue the window drains.
    send: mpsc::SyncSender<Call>,
    /// How incoming requests wake the window.
    wake: pm_acp::Notify,
    /// Whether the listener still belongs to a window.
    alive: AtomicBool,
    /// Connections holding transport resources now.
    connections: AtomicUsize,
}

/// One editor-owned server listening exclusively on the loopback interface.
pub(crate) struct Server {
    /// The OS-selected port of this window's listener.
    port: u16,
    /// Shared listener and token state.
    shared: Arc<Shared>,
    /// Calls waiting for the window.
    receive: mpsc::Receiver<Call>,
}

impl Server {
    /// Opens a bounded local MCP endpoint and starts its accept thread.
    pub fn start(wake: pm_acp::Notify) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let (send, receive) = mpsc::sync_channel(CONNECTIONS);
        let shared = Arc::new(Shared {
            tokens: Mutex::new(BTreeMap::new()),
            send,
            wake,
            alive: AtomicBool::new(true),
            connections: AtomicUsize::new(0),
        });
        let held = shared.clone();
        std::thread::spawn(move || accept(listener, held));
        Ok(Self {
            port,
            shared,
            receive,
        })
    }

    /// Makes a factory that issues fresh, conversation-bound bearer credentials.
    pub fn factory(&self) -> McpFactory {
        let shared = self.shared.clone();
        let port = self.port;
        Arc::new(move |caller| {
            let secret = token()?;
            if let Ok(mut tokens) = shared.tokens.lock() {
                tokens.retain(|_, held| *held != caller);
                tokens.insert(secret.clone(), caller);
            }
            Ok(McpServer {
                name: "pandemonium-sessions".to_owned(),
                reach: Reach::Http {
                    url: format!("http://127.0.0.1:{port}/mcp"),
                    headers: vec![("Authorization".to_owned(), format!("Bearer {secret}"))],
                },
                description: "Editor-owned project-scoped session orchestration".to_owned(),
                website: String::new(),
                enabled: true,
            })
        })
    }

    /// Removes secrets whose conversations have been closed.
    pub fn retain(&self, ids: &[TalkId]) {
        if let Ok(mut tokens) = self.shared.tokens.lock() {
            tokens.retain(|_, caller| ids.contains(caller));
        }
    }

    /// Takes the invocations waiting for the event loop.
    pub fn drain(&self) -> Vec<Call> {
        self.receive.try_iter().collect()
    }
}

impl Drop for Server {
    /// Stops accepting requests when the owning window goes away.
    fn drop(&mut self) {
        self.shared.alive.store(false, Ordering::Release);
    }
}

/// Makes an opaque bearer credential from the operating system's random source.
fn token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Accepts a bounded number of short-lived HTTP connections until shutdown.
fn accept(listener: TcpListener, shared: Arc<Shared>) {
    while shared.alive.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if shared.connections.fetch_add(1, Ordering::AcqRel) >= CONNECTIONS {
                    shared.connections.fetch_sub(1, Ordering::AcqRel);
                    let _ = respond(&mut stream, 503, None);
                    continue;
                }
                let shared = shared.clone();
                std::thread::spawn(move || {
                    let _ = serve(&mut stream, &shared);
                    shared.connections.fetch_sub(1, Ordering::AcqRel);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(_) => break,
        }
    }
}

/// Reads one strictly bounded HTTP request and dispatches its JSON-RPC message.
fn serve(stream: &mut TcpStream, shared: &Shared) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(&mut *stream);
    let mut line = String::new();
    reader.by_ref().take(4096).read_line(&mut line)?;
    let request = line
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut length = None;
    let mut credential = None;
    let mut invalid = false;
    let mut read = line.len();
    loop {
        line.clear();
        let bytes = reader.by_ref().take(4096).read_line(&mut line)?;
        read += bytes;
        if bytes == 0 || read > 8192 {
            invalid = true;
            break;
        }
        if line == "\r\n" {
            break;
        }
        let Some((key, value)) = line.trim().split_once(':') else {
            invalid = true;
            break;
        };
        let value = value.trim();
        match key.to_ascii_lowercase().as_str() {
            "content-length" => {
                if length.is_some() {
                    invalid = true;
                }
                length = value.parse::<usize>().ok();
            }
            "authorization" => credential = value.strip_prefix("Bearer ").map(str::to_owned),
            "origin" => invalid = true,
            "transfer-encoding" => invalid = true,
            "mcp-protocol-version" => invalid |= !VERSIONS.contains(&value),
            "host" => invalid |= !value.starts_with("127.0.0.1:"),
            _ => {}
        }
    }
    let caller = credential.and_then(|secret| shared.tokens.lock().ok()?.get(&secret).copied());
    if invalid || request.len() != 3 || request[1] != "/mcp" {
        return respond(reader.get_mut(), 400, None);
    }
    let Some(caller) = caller else {
        return respond(reader.get_mut(), 403, None);
    };
    if request[0] != "POST" {
        return respond(reader.get_mut(), 405, None);
    }
    let Some(length) = length.filter(|length| *length <= BODY_LIMIT) else {
        return respond(reader.get_mut(), 413, None);
    };
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => return respond(reader.get_mut(), 400, None),
    };
    let id = message["id"].clone();
    if !message.is_object()
        || message["jsonrpc"] != "2.0"
        || !message["method"].is_string()
        || !(id.is_null() || id.is_string() || id.is_i64() || id.is_u64())
    {
        return respond(
            reader.get_mut(),
            400,
            Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "Invalid JSON-RPC request"}}),
            ),
        );
    }
    let method = message["method"].as_str().unwrap_or_default();
    if id.is_null() {
        if method == "notifications/cancelled" {
            let _ = shared.send.try_send(Call {
                caller,
                id: message["params"]["requestId"].clone(),
                name: method.to_owned(),
                arguments: json!({}),
                reply: None,
                abandoned: Arc::new(AtomicBool::new(false)),
            });
            (shared.wake)();
        }
        return respond(reader.get_mut(), 202, None);
    }
    let result = match method {
        "initialize" => {
            let requested = message["params"]["protocolVersion"]
                .as_str()
                .unwrap_or_default();
            let version = if VERSIONS.contains(&requested) {
                requested
            } else {
                VERSIONS[1]
            };
            json!({"protocolVersion": version, "capabilities": {"tools": {}}, "serverInfo": {"name": "pandemonium-sessions", "version": env!("CARGO_PKG_VERSION")}, "instructions": "Session tools are project-scoped and bounded. Delegation never authorizes commit, push or landing. Busy messages are queued."})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools::definitions()}),
        "tools/call" => invoke(shared, caller, &id, &message["params"]),
        _ => {
            return respond(
                reader.get_mut(),
                200,
                Some(
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Unknown MCP method"}}),
                ),
            );
        }
    };
    respond(
        reader.get_mut(),
        200,
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result})),
    )
}

/// Waits for a window-serviced tool and marks timed-out work for rollback.
fn invoke(shared: &Shared, caller: TalkId, id: &Value, params: &Value) -> Value {
    let (reply, receive) = mpsc::sync_channel(1);
    let abandoned = Arc::new(AtomicBool::new(false));
    let call = Call {
        caller,
        id: id.clone(),
        name: params["name"].as_str().unwrap_or_default().to_owned(),
        arguments: params.get("arguments").cloned().unwrap_or(json!({})),
        reply: Some(reply),
        abandoned: abandoned.clone(),
    };
    let result = match shared.send.try_send(call) {
        Ok(()) => {
            (shared.wake)();
            receive.recv_timeout(Duration::from_secs(90)).unwrap_or_else(|_| {
                abandoned.store(true, Ordering::Release);
                (shared.wake)();
                Err("Editor request timed out; pending creation will be rolled back. Retry after the editor responds.".to_owned())
            })
        }
        Err(_) => Err("The editor's MCP request queue is full; retry later".to_owned()),
    };
    match result {
        Ok(value) => {
            json!({"content": [{"type": "text", "text": value.to_string()}], "structuredContent": value, "isError": false})
        }
        Err(message) => json!({"content": [{"type": "text", "text": message}], "isError": true}),
    }
}

/// Writes one JSON response or empty HTTP status and closes the connection.
fn respond(stream: &mut TcpStream, status: u16, value: Option<Value>) -> std::io::Result<()> {
    let body = value.map(|value| value.to_string()).unwrap_or_default();
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        403 => "Forbidden",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        503 => "Service Unavailable",
        _ => "Bad Request",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
