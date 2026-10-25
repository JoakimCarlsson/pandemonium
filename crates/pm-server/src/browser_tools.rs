//! The browser's tools, offered to agents as an MCP server on the loopback address and run by a window.
//!
//! An agent calls a tool over HTTP; the server sends the call to the client that
//! last said it can run them, waits for the answer, and gives it back.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use pm_protocol::ServerMessage;
use serde_json::{Value, json};

use crate::core::Connection;

/// How long a call waits for a window to answer.
const PATIENCE: Duration = Duration::from_secs(90);

/// The most bytes of a request that are read.
const MOST: usize = 1 << 20;

/// What a window answered a call with.
pub(crate) struct Reply {
    /// Whether the tool worked.
    pub(crate) ok: bool,
    /// What it says.
    pub(crate) text: String,
    /// A picture it took, as PNG.
    pub(crate) image: Vec<u8>,
}

/// What the server knows of the tools.
struct Tools {
    /// The address agents are given and the token that lets them in, once the endpoint listens.
    endpoint: OnceLock<(String, String)>,
    /// The connections that can run the tools, oldest first.
    runners: Mutex<Vec<Arc<Connection>>>,
    /// The calls waiting for an answer.
    pending: Mutex<HashMap<u64, Sender<Reply>>>,
    /// The number the next call gets.
    next: AtomicU64,
}

/// The one set of tools of this server.
static TOOLS: LazyLock<Tools> = LazyLock::new(|| Tools {
    endpoint: OnceLock::new(),
    runners: Mutex::new(Vec::new()),
    pending: Mutex::new(HashMap::new()),
    next: AtomicU64::new(1),
});

/// The address and token of the tools, when some window can run them, to be given to an agent when it starts.
pub(crate) fn offered() -> Option<(String, String)> {
    let runners = TOOLS.runners.lock().ok()?;
    if runners.is_empty() {
        return None;
    }
    TOOLS.endpoint.get().cloned()
}

/// Notes that `connection` can, or can no longer, run the tools, and starts the endpoint when the first one can.
pub(crate) fn set_available(connection: &Arc<Connection>, available: bool) {
    if available {
        ensure_started();
    }
    if let Ok(mut runners) = TOOLS.runners.lock() {
        runners.retain(|held| !Arc::ptr_eq(held, connection));
        if available {
            runners.push(connection.clone());
        }
    }
}

/// Forgets `connection`, which went away.
pub(crate) fn forget(connection: &Arc<Connection>) {
    set_available(connection, false);
}

/// Hands the answer of a window to the call waiting for it.
pub(crate) fn answer(call: u64, reply: Reply) {
    let sender = TOOLS.pending.lock().ok().and_then(|mut p| p.remove(&call));
    if let Some(sender) = sender {
        let _ = sender.send(reply);
    }
}

/// Starts listening, once.
fn ensure_started() {
    if TOOLS.endpoint.get().is_some() {
        return;
    }
    let mut secret = [0u8; 24];
    if getrandom::fill(&mut secret).is_err() {
        return;
    }
    let token: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
        return;
    };
    let Ok(local) = listener.local_addr() else {
        return;
    };
    if TOOLS
        .endpoint
        .set((
            format!("http://127.0.0.1:{}/mcp", local.port()),
            token.clone(),
        ))
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let token = token.clone();
            std::thread::spawn(move || serve(stream, &token));
        }
    });
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

/// Writes an HTTP answer with `status` and `body`, and ends the connection.
fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

/// Serves one request of an agent.
fn serve(mut stream: TcpStream, token: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || !line.starts_with("POST /mcp") {
        respond(&mut stream, "404 Not Found", "{}");
        return;
    }
    let (mut length, mut allowed) = (0usize, false);
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.parse().unwrap_or(0),
            "authorization" => {
                allowed = value
                    .strip_prefix("Bearer ")
                    .is_some_and(|given| same(given, token));
            }
            _ => {}
        }
    }
    if !allowed {
        respond(&mut stream, "401 Unauthorized", "{}");
        return;
    }
    if length > MOST {
        respond(&mut stream, "413 Payload Too Large", "{}");
        return;
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        respond(&mut stream, "400 Bad Request", "{}");
        return;
    };
    match handle(&message) {
        Some(answer) => respond(&mut stream, "200 OK", &answer.to_string()),
        None => respond(&mut stream, "202 Accepted", "{}"),
    }
}

/// The answer to one JSON-RPC message, or nothing when it is a notification.
fn handle(message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let result = match message["method"].as_str().unwrap_or_default() {
        "initialize" => json!({
            "protocolVersion": message["params"]["protocolVersion"]
                .as_str()
                .unwrap_or("2025-03-26"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "pandemonium-browser", "version": env!("CARGO_PKG_VERSION") },
        }),
        "tools/list" => json!({ "tools": definitions() }),
        "tools/call" => call_tool(&message["params"]),
        "ping" => json!({}),
        other => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("no method {other}") },
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// Runs the tool a `tools/call` names in a window and describes what came of it.
fn call_tool(params: &Value) -> Value {
    let Some(tool) = params["name"].as_str() else {
        return failure("no tool was named");
    };
    if !definitions()
        .iter()
        .any(|definition| definition["name"] == tool)
    {
        return failure(&format!("there is no tool {tool}"));
    }
    let arguments = params["arguments"].to_string();
    match run(tool, &arguments) {
        Ok(reply) => {
            let mut content = vec![json!({ "type": "text", "text": reply.text })];
            if !reply.image.is_empty() {
                content.push(json!({
                    "type": "image",
                    "data": base64(&reply.image),
                    "mimeType": "image/png",
                }));
            }
            json!({ "content": content, "isError": !reply.ok })
        }
        Err(reason) => failure(&reason),
    }
}

/// A tool result that says the call failed, for `reason`.
fn failure(reason: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": reason }], "isError": true })
}

/// Sends the call to the window that can run it and waits for the answer.
fn run(tool: &str, arguments: &str) -> Result<Reply, String> {
    let runner = TOOLS
        .runners
        .lock()
        .ok()
        .and_then(|runners| runners.last().cloned())
        .ok_or("no window with the embedded browser turned on is connected")?;
    let call = TOOLS.next.fetch_add(1, Ordering::Relaxed);
    let (sender, receiver) = channel();
    if let Ok(mut pending) = TOOLS.pending.lock() {
        pending.insert(call, sender);
    }
    let sent = runner.outbox.send(ServerMessage::BrowserCall {
        call,
        tool: tool.to_owned(),
        arguments: arguments.to_owned(),
    });
    if sent.is_err() {
        if let Ok(mut pending) = TOOLS.pending.lock() {
            pending.remove(&call);
        }
        return Err("the window with the browser went away".to_owned());
    }
    receiver.recv_timeout(PATIENCE).map_err(|_| {
        if let Ok(mut pending) = TOOLS.pending.lock() {
            pending.remove(&call);
        }
        "the browser did not answer in time".to_owned()
    })
}

/// `bytes` as standard base64.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = chunk.iter().enumerate().fold(0u32, |held, (place, byte)| {
            held | (u32::from(*byte) << (16 - 8 * place))
        });
        for place in 0..4 {
            if place <= chunk.len() {
                text.push(char::from(
                    TABLE[((triple >> (18 - 6 * place)) & 63) as usize],
                ));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// A tool the agents are offered: its name, what it does and what it takes.
fn tool(name: &str, description: &str, properties: &Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

/// The tools the agents are offered.
fn definitions() -> Vec<Value> {
    let target = json!({
        "ref": { "type": "integer", "description": "A number from the last browser_snapshot." },
        "selector": { "type": "string", "description": "A CSS selector." },
    });
    vec![
        tool(
            "browser_navigate",
            "Open an address in the browser pane and wait for it to load.",
            &json!({ "url": { "type": "string" } }),
            &["url"],
        ),
        tool(
            "browser_snapshot",
            "Describe the page: its address, title and a numbered list of the elements that can be clicked or typed in, with their text.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_screenshot",
            "Take a picture of what the browser pane shows.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_click",
            "Click an element, given by `ref` or `selector`, or a point given by `x` and `y` in the page.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "x": { "type": "number" },
                "y": { "type": "number" },
            }),
            &[],
        ),
        tool(
            "browser_type",
            "Type text into an element, given by `ref` or `selector`; `submit` presses Enter afterwards.",
            &json!({
                "ref": target["ref"],
                "selector": target["selector"],
                "text": { "type": "string" },
                "submit": { "type": "boolean" },
            }),
            &["text"],
        ),
        tool(
            "browser_evaluate",
            "Run a JavaScript expression in the page and give back its value as JSON.",
            &json!({ "expression": { "type": "string" } }),
            &["expression"],
        ),
        tool(
            "browser_console",
            "The messages and errors the page wrote to its console since it was opened.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_network",
            "The requests the page made since it was opened, with their status.",
            &json!({}),
            &[],
        ),
        tool(
            "browser_wait",
            "Wait until `text` is on the page or `selector` matches, or `timeout_ms` passes.",
            &json!({
                "text": { "type": "string" },
                "selector": { "type": "string" },
                "timeout_ms": { "type": "integer" },
            }),
            &[],
        ),
    ]
}
