//! JSON-RPC with a language server over its standard input and output: requests matched to their answers, notifications both ways, and the server's own requests answered.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

/// How many of the last lines the server wrote to its error output are kept, to say why it stopped.
const KEPT_LINES: usize = 20;

/// What answers the server's own requests and takes in its notifications: given the method and the parameters, it returns the result of a request.
pub(crate) type Handler = Arc<dyn Fn(&str, Value) -> Value + Send + Sync>;

/// The answers being waited for, by the id of the request.
type Waiting = Arc<Mutex<HashMap<i64, Sender<Result<Value, String>>>>>;

/// A language server process and the conversation with it.
pub(crate) struct Rpc {
    /// Where messages to the server are written.
    input: Arc<Mutex<ChildStdin>>,
    /// The id the next request gets.
    next: AtomicI64,
    /// The requests sent that have no answer yet.
    waiting: Waiting,
    /// The process.
    child: Mutex<Child>,
    /// Whether the server is still talking.
    alive: Arc<AtomicBool>,
    /// The last lines it wrote to its error output.
    said: Arc<Mutex<VecDeque<String>>>,
}

/// Writes `message` to the server, framed with its length.
fn send(input: &Mutex<ChildStdin>, message: &Value) -> Result<(), String> {
    let body = serde_json::to_string(message).map_err(|error| error.to_string())?;
    let mut input = input
        .lock()
        .map_err(|_| "the language server is gone".to_owned())?;
    write!(input, "Content-Length: {}\r\n\r\n{body}", body.len())
        .and_then(|()| input.flush())
        .map_err(|error| format!("could not write to the language server: {error}"))
}

/// Reads one message from the server, or nothing once it stopped.
fn receive(output: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if output.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    output.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// The path the server `program` runs with: the folders `first` that exist, then `base`, or when there is none the machine's own path with the usual places programs install themselves added, so that a server started from a desktop session finds the tools it runs, such as `go` for a Go server.
fn path_with(
    program: &Path,
    first: &[PathBuf],
    base: Option<std::ffi::OsString>,
) -> Option<std::ffi::OsString> {
    let existing = first.iter().filter(|folder| folder.is_dir()).cloned();
    let machine: Vec<PathBuf> =
        std::env::split_paths(&base.unwrap_or_else(|| pm_tools::path_beside(program))).collect();
    std::env::join_paths(existing.chain(machine)).ok()
}

impl Rpc {
    /// Starts `program` with `arguments` in `folder`, with the folders `first` put first on its path and the variables `env` set, and talks to it on threads of its own, `handler` answering what it asks and taking in what it tells.
    pub(crate) fn start(
        program: &Path,
        arguments: &[String],
        folder: &Path,
        first: &[PathBuf],
        env: &BTreeMap<String, String>,
        handler: Handler,
    ) -> Result<Arc<Self>, String> {
        let mut command = Command::new(program);
        command
            .args(arguments)
            .current_dir(folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in env.iter().filter(|(name, _)| *name != "PATH") {
            command.env(name, value);
        }
        if let Some(path) = path_with(
            program,
            first,
            env.get("PATH").map(std::ffi::OsString::from),
        ) {
            command.env("PATH", path);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", program.display()))?;
        let input = Arc::new(Mutex::new(
            child.stdin.take().ok_or("the server has no input")?,
        ));
        let output = child.stdout.take().ok_or("the server has no output")?;
        let errors = child
            .stderr
            .take()
            .ok_or("the server has no error output")?;
        let rpc = Arc::new(Self {
            input: input.clone(),
            next: AtomicI64::new(1),
            waiting: Arc::new(Mutex::new(HashMap::new())),
            child: Mutex::new(child),
            alive: Arc::new(AtomicBool::new(true)),
            said: Arc::new(Mutex::new(VecDeque::new())),
        });
        let said = rpc.said.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(errors).lines().map_while(Result::ok) {
                if let Ok(mut said) = said.lock() {
                    said.push_back(line);
                    while said.len() > KEPT_LINES {
                        said.pop_front();
                    }
                }
            }
        });
        let (waiting, alive) = (rpc.waiting.clone(), rpc.alive.clone());
        std::thread::spawn(move || {
            let mut output = BufReader::new(output);
            while let Some(message) = receive(&mut output) {
                Self::take(&input, &waiting, &handler, message);
            }
            alive.store(false, Ordering::Relaxed);
            if let Ok(mut waiting) = waiting.lock() {
                for (_, answer) in waiting.drain() {
                    let _ = answer.send(Err("the language server stopped".to_owned()));
                }
            }
        });
        Ok(rpc)
    }

    /// Takes in one message from the server: an answer goes to whoever waits for it, a request is answered by `handler`, and a notification is handed to it.
    fn take(input: &Mutex<ChildStdin>, waiting: &Waiting, handler: &Handler, message: Value) {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        match (method, id) {
            (Some(method), Some(id)) => {
                let result = handler(
                    method,
                    message.get("params").cloned().unwrap_or(Value::Null),
                );
                let _ = send(
                    input,
                    &json!({"jsonrpc": "2.0", "id": id, "result": result}),
                );
            }
            (Some(method), None) => {
                handler(
                    method,
                    message.get("params").cloned().unwrap_or(Value::Null),
                );
            }
            (None, Some(id)) => {
                let Some(number) = id.as_i64() else {
                    return;
                };
                let answer = waiting
                    .lock()
                    .ok()
                    .and_then(|mut held| held.remove(&number));
                if let Some(answer) = answer {
                    let outcome = match message.get("error") {
                        Some(error) => Err(error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("the language server failed")
                            .to_owned()),
                        None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = answer.send(outcome);
                }
            }
            (None, None) => {}
        }
    }

    /// Asks the server `method` with `params` and waits up to `patience` for the answer.
    pub(crate) fn request(
        &self,
        method: &str,
        params: Value,
        patience: Duration,
    ) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (answer, answered) = mpsc::channel();
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.insert(id, answer);
        }
        let sent = send(
            &self.input,
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        );
        if let Err(reason) = sent {
            self.forget(id);
            return Err(reason);
        }
        match answered.recv_timeout(patience) {
            Ok(outcome) => outcome,
            Err(_) => {
                self.forget(id);
                let _ = self.notify("$/cancelRequest", json!({"id": id}));
                Err(format!(
                    "the language server did not answer {method} in time"
                ))
            }
        }
    }

    /// Stops waiting for the answer to the request `id`.
    fn forget(&self, id: i64) {
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.remove(&id);
        }
    }

    /// Tells the server `method` with `params`, expecting no answer.
    pub(crate) fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        send(
            &self.input,
            &json!({"jsonrpc": "2.0", "method": method, "params": params}),
        )
    }

    /// Whether the server is still talking.
    pub(crate) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// The last thing the server wrote to its error output, to say why it stopped.
    pub(crate) fn last_words(&self) -> Option<String> {
        self.said.lock().ok()?.back().cloned()
    }

    /// Ends the server's process.
    pub(crate) fn kill(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Asks the server to stop, then ends its process after `grace` whether it did or not.
    pub(crate) fn stop(&self, grace: Duration) {
        if self.is_alive() {
            let _ = self.request("shutdown", Value::Null, grace);
            let _ = self.notify("exit", Value::Null);
        }
        let deadline = std::time::Instant::now() + grace;
        while std::time::Instant::now() < deadline {
            let exited = self
                .child
                .lock()
                .ok()
                .and_then(|mut child| child.try_wait().ok().flatten())
                .is_some();
            if exited {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.kill();
    }
}
