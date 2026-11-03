//! A conversation with an agent's process: requests and their answers.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::standing::{Login, Way};

/// The error code an agent answers with when it wants a login first.
pub(crate) const LOGIN_REQUIRED: i64 = -32000;

/// An agent's process, spoken to line by line.
pub(crate) struct Client {
    /// The process, shared with whoever may have to end it from another thread.
    child: Arc<Mutex<Child>>,
    /// What the process is written to.
    stdin: ChildStdin,
    /// The messages the process wrote, one per line.
    lines: Receiver<Value>,
    /// The id the next request is sent under.
    next: u64,
    /// The last of what the process wrote to its error stream.
    errors: Arc<Mutex<String>>,
}

/// How many characters of the error stream are kept to explain a stop.
const ERROR_TAIL: usize = 400;

/// What listening to an agent for a moment came to.
pub(crate) enum Heard {
    /// The agent wrote this message.
    Message(Value),
    /// The agent wrote nothing in the time given.
    Silence,
    /// The agent's output is closed: it has stopped.
    Closed,
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
    pub(crate) fn start(command: Command) -> Result<Self, String> {
        Self::start_in(command, &std::env::home_dir().unwrap_or_else(|| ".".into()))
    }

    /// Starts `command` with its standard streams piped, in `directory`.
    pub(crate) fn start_in(mut command: Command, directory: &Path) -> Result<Self, String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(directory)
            .spawn()
            .map_err(|error| format!("could not start: {error}"))?;
        let stdin = child.stdin.take().ok_or("no input pipe")?;
        let stdout = child.stdout.take().ok_or("no output pipe")?;
        let errors = Arc::new(Mutex::new(String::new()));
        if let Some(stderr) = child.stderr.take() {
            let kept = Arc::clone(&errors);
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Ok(mut kept) = kept.lock() {
                        kept.push_str(&line);
                        kept.push('\n');
                        let extra = kept.chars().count().saturating_sub(ERROR_TAIL);
                        let cut = kept.char_indices().nth(extra).map_or(0, |(at, _)| at);
                        kept.drain(..cut);
                    }
                }
            });
        }
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
            child: Arc::new(Mutex::new(child)),
            stdin,
            lines,
            next: 1,
            errors,
        })
    }

    /// Why the process stopped: its last words on the error stream, when it said any.
    fn stopped(&self) -> String {
        std::thread::sleep(Duration::from_millis(200));
        let said = self.errors.lock().map(|kept| kept.trim().to_owned());
        match said {
            Ok(said) if !said.is_empty() => format!("stopped: {said}"),
            _ => "stopped".to_owned(),
        }
    }

    /// The process, for ending it from another thread.
    pub(crate) fn process(&self) -> Arc<Mutex<Child>> {
        Arc::clone(&self.child)
    }

    /// Writes `message` to the agent as one line.
    pub(crate) fn send(&mut self, message: &Value) -> Result<(), String> {
        writeln!(self.stdin, "{message}").map_err(|error| format!("could not write: {error}"))?;
        self.stdin
            .flush()
            .map_err(|error| format!("could not write: {error}"))
    }

    /// Sends `method` with `params` without waiting, and returns the id it went out under.
    pub(crate) fn request(&mut self, method: &str, params: &Value) -> Result<u64, String> {
        let id = self.next;
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        Ok(id)
    }

    /// Sends `notification` without an id, so that nothing answers it.
    pub(crate) fn notify(&mut self, method: &str, params: &Value) -> Result<(), String> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    /// Answers the request the agent made under `id` with `result`.
    pub(crate) fn respond(&mut self, id: &Value, result: &Value) -> Result<(), String> {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }

    /// Refuses the request the agent made under `id`, for `message`.
    pub(crate) fn refuse(&mut self, id: &Value, code: i64, message: &str) -> Result<(), String> {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }))
    }

    /// The next message the agent wrote, waiting up to `patience` for one.
    pub(crate) fn receive(&mut self, patience: Duration) -> Heard {
        match self.lines.recv_timeout(patience) {
            Ok(message) => Heard::Message(message),
            Err(RecvTimeoutError::Timeout) => Heard::Silence,
            Err(RecvTimeoutError::Disconnected) => Heard::Closed,
        }
    }

    /// Sends `method` with `params` and waits up to `patience` for its answer.
    pub(crate) fn ask(
        &mut self,
        method: &str,
        params: &Value,
        patience: Duration,
    ) -> Result<Answer, String> {
        let id = self.request(method, params)?;
        let deadline = Instant::now() + patience;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = match self.lines.recv_timeout(left) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout) => return Err("did not answer in time".to_owned()),
                Err(RecvTimeoutError::Disconnected) => return Err(self.stopped()),
            };
            if message["id"] != json!(id) || message.get("method").is_some() {
                continue;
            }
            return Ok(answer(&message));
        }
    }

    /// Introduces the app to the agent and returns what the agent says it is.
    pub(crate) fn initialize(&mut self, patience: Duration) -> Result<Value, String> {
        match self.ask(
            "initialize",
            &json!({ "protocolVersion": 1, "clientCapabilities": { "terminal": true, "elicitation": { "form": {} }, "auth": { "terminal": true }, "subagents": {}, "_meta": { "jetbrains": { "air": { "version": 1, "capabilities": ["asyncTasks"] } } } } }),
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
        end(&self.child);
    }
}

/// Ends the process `child` and collects it.
pub(crate) fn end(child: &Mutex<Child>) {
    if let Ok(mut child) = child.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// What a response message says: its result, or its error.
pub(crate) fn answer(message: &Value) -> Answer {
    match message.get("error") {
        Some(error) => Answer::Error(
            error["code"].as_i64().unwrap_or_default(),
            error["message"].as_str().unwrap_or("failed").to_owned(),
        ),
        None => Answer::Result(message["result"].clone()),
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
