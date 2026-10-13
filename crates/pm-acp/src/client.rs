//! A conversation with an agent's process: requests and their answers.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::standing::{Login, Way};

/// The error code an agent answers with when it wants a login first.
pub(crate) const LOGIN_REQUIRED: i64 = -32000;

/// An agent's process, spoken to line by line.
pub(crate) struct Client {
    /// The process.
    child: Child,
    /// What the process is written to.
    stdin: ChildStdin,
    /// The messages the process wrote, one per line.
    lines: Receiver<Value>,
    /// The id the next request is sent under.
    next: u64,
}

/// What an agent answered to one request.
pub(crate) enum Answer {
    /// The request worked, with this result.
    Result(Value),
    /// The request failed, with this code and message.
    Error(i64, String),
}

impl Client {
    /// Starts `command` with its standard streams piped, in the reader's home directory.
    pub(crate) fn start(mut command: Command) -> Result<Self, String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(std::env::home_dir().unwrap_or_else(|| ".".into()))
            .spawn()
            .map_err(|error| format!("could not start: {error}"))?;
        let stdin = child.stdin.take().ok_or("no input pipe")?;
        let stdout = child.stdout.take().ok_or("no output pipe")?;
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str::<Value>(&line)
                    && sender.send(message).is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            lines,
            next: 1,
        })
    }

    /// Sends `method` with `params` and waits up to `patience` for its answer.
    pub(crate) fn ask(
        &mut self,
        method: &str,
        params: &Value,
        patience: Duration,
    ) -> Result<Answer, String> {
        let id = self.next;
        self.next += 1;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{request}").map_err(|error| format!("could not write: {error}"))?;
        self.stdin
            .flush()
            .map_err(|error| format!("could not write: {error}"))?;
        let deadline = Instant::now() + patience;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = match self.lines.recv_timeout(left) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => return Err("did not answer in time".to_owned()),
                Err(RecvTimeoutError::Disconnected) => return Err("stopped".to_owned()),
            };
            if message["id"] != json!(id) || message.get("method").is_some() {
                continue;
            }
            return Ok(match message.get("error") {
                Some(error) => Answer::Error(
                    error["code"].as_i64().unwrap_or_default(),
                    error["message"].as_str().unwrap_or("failed").to_owned(),
                ),
                None => Answer::Result(message["result"].clone()),
            });
        }
    }

    /// Introduces the app to the agent and returns what the agent says it is.
    pub(crate) fn initialize(&mut self, patience: Duration) -> Result<Value, String> {
        match self.ask(
            "initialize",
            &json!({ "protocolVersion": 1, "clientCapabilities": {} }),
            patience,
        )? {
            Answer::Result(result) => Ok(result),
            Answer::Error(_, message) => Err(message),
        }
    }
}

impl Drop for Client {
    /// Ends the process, and collects it off the caller's thread.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The logins an agent's `authMethods` offer.
pub(crate) fn logins(methods: &Value) -> Vec<Login> {
    methods
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|method| {
            let way = match method["type"].as_str() {
                None | Some("agent") => Way::Asked,
                Some("terminal") => Way::Terminal {
                    args: method["args"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|arg| arg.as_str().map(str::to_owned))
                        .collect(),
                },
                Some(_) => return None,
            };
            Some(Login {
                id: method["id"].as_str()?.to_owned(),
                name: method["name"].as_str().unwrap_or("Log in").to_owned(),
                way,
            })
        })
        .collect()
}
