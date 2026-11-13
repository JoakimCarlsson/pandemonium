//! A tool server offered to agents over HTTP on the loopback address, speaking just enough of MCP for an agent to list its tools and call them.
//!
//! Each session is given a token of its own, so a call says which session made it
//! and no agent can act for another session. What the tools are and what a call
//! does is left to whoever makes the endpoint.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Value, json};

/// The most bytes of a request that are read.
const MOST: usize = 1 << 20;

/// What a call of an agent of a session does: it is handed the tool's name, its arguments and the session's number, and gives back the MCP result.
pub(crate) type Run = fn(&str, &Value, u64) -> Value;

/// The tokens that let sessions in.
#[derive(Default)]
struct Tokens {
    /// The token each session was given.
    of_session: HashMap<u64, String>,
    /// The session each token was given to.
    of_token: HashMap<String, u64>,
}

/// One tool server: its name, the address it listens on once started, the tokens it gave out, its tools and what calling one does.
pub(crate) struct Endpoint {
    /// The name the server tells agents it goes by.
    name: &'static str,
    /// The address agents are given, once the endpoint listens.
    address: OnceLock<String>,
    /// The token of each session, by session number, and the session of each token.
    tokens: Mutex<Tokens>,
    /// The tools the agents are offered.
    tools: fn() -> Vec<Value>,
    /// What a call of one of them does.
    run: Run,
}

impl Endpoint {
    /// An endpoint called `name` offering what `tools` lists and doing `run` for each call, not listening yet.
    pub(crate) fn new(name: &'static str, tools: fn() -> Vec<Value>, run: Run) -> Self {
        Self {
            name,
            address: OnceLock::new(),
            tokens: Mutex::new(Tokens::default()),
            tools,
            run,
        }
    }

    /// Starts listening, once; later calls do nothing.
    pub(crate) fn start(&'static self) {
        if self.address.get().is_some() {
            return;
        }
        let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
            return;
        };
        let Ok(local) = listener.local_addr() else {
            return;
        };
        if self
            .address
            .set(format!("http://127.0.0.1:{}/mcp", local.port()))
            .is_err()
        {
            return;
        }
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || self.serve(stream));
            }
        });
    }

    /// The address of the endpoint and the token of `session`, to be given to the agent of that session when it starts; nothing until the endpoint listens, or when the system has no randomness to make a token.
    pub(crate) fn offered(&self, session: u64) -> Option<(String, String)> {
        let address = self.address.get()?.clone();
        let mut tokens = self.tokens.lock().ok()?;
        if let Some(token) = tokens.of_session.get(&session) {
            return Some((address, token.clone()));
        }
        let token = fresh_token()?;
        tokens.of_session.insert(session, token.clone());
        tokens.of_token.insert(token.clone(), session);
        Some((address, token))
    }

    /// The session `token` was given to.
    pub(crate) fn session_of(&self, token: &str) -> Option<u64> {
        let tokens = self.tokens.lock().ok()?;
        tokens
            .of_token
            .iter()
            .find(|(known, _)| same(known, token))
            .map(|(_, session)| *session)
    }

    /// Serves one request of an agent.
    fn serve(&self, mut stream: TcpStream) {
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
        let (mut length, mut session) = (0usize, None);
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
                    session = value
                        .strip_prefix("Bearer ")
                        .and_then(|token| self.session_of(token));
                }
                _ => {}
            }
        }
        let Some(session) = session else {
            respond(&mut stream, "401 Unauthorized", "{}");
            return;
        };
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
        match self.handle(&message, session) {
            Some(answer) => respond(&mut stream, "200 OK", &answer.to_string()),
            None => respond(&mut stream, "202 Accepted", "{}"),
        }
    }

    /// The answer to one JSON-RPC message of an agent of `session`, or nothing when it is a notification.
    pub(crate) fn handle(&self, message: &Value, session: u64) -> Option<Value> {
        let id = message.get("id")?.clone();
        let result = match message["method"].as_str().unwrap_or_default() {
            "initialize" => json!({
                "protocolVersion": message["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or("2025-03-26"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": self.name, "version": env!("CARGO_PKG_VERSION") },
            }),
            "tools/list" => json!({ "tools": (self.tools)() }),
            "tools/call" => self.call(&message["params"], session),
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

    /// Runs the tool a `tools/call` of an agent of `session` names, when the endpoint offers one by that name, and gives back what came of it.
    fn call(&self, params: &Value, session: u64) -> Value {
        let Some(tool) = params["name"].as_str() else {
            return failure("no tool was named");
        };
        if !(self.tools)()
            .iter()
            .any(|definition| definition["name"] == tool)
        {
            return failure(&format!("there is no tool {tool}"));
        }
        (self.run)(tool, &params["arguments"], session)
    }
}

/// A token nobody has been given, or nothing when the system has no randomness to make one.
fn fresh_token() -> Option<String> {
    let mut secret = [0u8; 24];
    getrandom::fill(&mut secret).ok()?;
    Some(secret.iter().map(|byte| format!("{byte:02x}")).collect())
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

/// A tool the agents are offered: its name, what it does and what it takes.
pub(crate) fn tool(name: &str, description: &str, properties: &Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

/// A tool result that says the call worked, with `text`.
pub(crate) fn success(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

/// A tool result that says the call failed, for `reason`.
pub(crate) fn failure(reason: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": reason }], "isError": true })
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use serde_json::{Value, json};

    use super::{Endpoint, success, tool};

    /// The one tool of the endpoint the tests use.
    fn echo_tools() -> Vec<Value> {
        vec![tool("echo", "Says the session back.", &json!({}), &[])]
    }

    /// Says which session called.
    fn echo(_: &str, _: &Value, session: u64) -> Value {
        success(&session.to_string())
    }

    /// The endpoint the tests use.
    static ECHO: LazyLock<Endpoint> = LazyLock::new(|| Endpoint::new("echo", echo_tools, echo));

    /// Each session is given a token of its own, which names that session and no other, and the same session keeps its token.
    #[test]
    fn tokens_belong_to_one_session() {
        ECHO.start();
        let (address, first) = ECHO.offered(901).unwrap();
        let (_, second) = ECHO.offered(902).unwrap();
        assert!(address.starts_with("http://127.0.0.1:"));
        assert_ne!(first, second);
        assert_eq!(ECHO.offered(901).unwrap().1, first);
        assert_eq!(ECHO.session_of(&first), Some(901));
        assert_eq!(ECHO.session_of(&second), Some(902));
        assert_eq!(ECHO.session_of("not a token"), None);
    }

    /// A call reaches the tool with the caller's session, and a tool the endpoint does not offer is refused.
    #[test]
    fn calls_reach_the_tool_with_their_session() {
        let called = ECHO
            .handle(
                &json!({ "id": 1, "method": "tools/call", "params": { "name": "echo" } }),
                7,
            )
            .unwrap();
        assert_eq!(called["result"]["content"][0]["text"], "7");
        let refused = ECHO
            .handle(
                &json!({ "id": 2, "method": "tools/call", "params": { "name": "other" } }),
                7,
            )
            .unwrap();
        assert_eq!(refused["result"]["isError"], true);
        assert!(
            ECHO.handle(&json!({ "method": "notifications/initialized" }), 7)
                .is_none()
        );
    }
}
