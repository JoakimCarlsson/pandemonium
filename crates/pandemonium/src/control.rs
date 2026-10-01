//! Same-user local control socket and terminal client for SSH access.

use std::sync::mpsc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(not(unix))]
use winit::event_loop::EventLoopProxy;

#[cfg(not(unix))]
use crate::app::Wake;

/// The major version of the editor control wire protocol.
pub const VERSION: u32 = 1;

/// One versioned request from a control client.
#[derive(Deserialize, Serialize)]
pub struct WireRequest {
    /// The wire protocol major version.
    pub version: u32,
    /// A client-chosen request identity, echoed in the response.
    pub id: u64,
    /// The operation to perform.
    pub method: String,
    /// The operation's named arguments.
    #[serde(default)]
    pub params: Value,
}

/// A machine-readable failure returned to a control client.
#[derive(Serialize)]
pub struct WireError {
    /// A stable error category.
    pub code: &'static str,
    /// A description suitable for display.
    pub message: String,
}

/// One response to one request on the same connection.
#[derive(Serialize)]
pub struct WireResponse {
    /// The wire protocol major version.
    pub version: u32,
    /// The request identity, or null when the request could not be decoded.
    pub id: Option<u64>,
    /// The operation result when successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// The failure when the operation did not succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WireError>,
}

/// Work for the editor event loop from a control request.
pub enum Operation {
    /// Run a terminal command through the editor's command seam.
    Execute(String),
    /// Read the window's project, session, and agent state.
    Snapshot,
    /// Read one agent's conversation as structured blocks.
    Transcript(usize),
    /// Read one conversation by its stable identity.
    TranscriptId(u64),
    /// Carry out a typed ACP operation in the editor window.
    Agent(AgentOperation),
}

/// A typed ACP operation issued by a remote client.
pub enum AgentOperation {
    /// List agents this editor can start.
    Catalog,
    /// Start a conversation in a checkout or a worktree session.
    Start {
        project: u64,
        session: Option<u64>,
        agent: String,
    },
    /// Read one conversation's status and controls.
    Detail(u64),
    /// Send exact prompt text with optional worktree files.
    Send {
        id: u64,
        text: String,
        files: Vec<String>,
        images: Vec<u64>,
    },
    /// Upload one base64 image chunk for a later prompt.
    UploadImage {
        id: u64,
        image: u64,
        mime_type: String,
        name: Option<String>,
        data: String,
        finish: bool,
    },
    /// Cancel the running turn.
    Cancel(u64),
    /// Answer or refuse an ACP permission request.
    Answer {
        id: u64,
        request: u64,
        choice: Option<String>,
    },
    /// Put a conversation into an offered mode.
    SetMode { id: u64, mode: String },
    /// Set an offered picked or switched knob.
    SetKnob { id: u64, knob: String, value: Value },
    /// Read or refresh saved conversations.
    ListHistory(u64),
    /// Load a saved conversation into the same worktree.
    LoadHistory { id: u64, saved: String },
    /// Start an offered login method.
    Login { id: u64, method: String },
    /// Read a running login terminal.
    LoginRead(u64),
    /// Write bytes to a running login terminal.
    LoginWrite { id: u64, input: String },
    /// Read a terminal stream produced by an ACP tool call.
    Terminal { id: u64, terminal: String },
}

/// A request waiting for the window and the channel its answer returns on.
pub struct Request {
    /// The operation the window applies or reads.
    pub operation: Operation,
    /// Where the window sends its answer.
    pub answer: mpsc::Sender<Result<Value, String>>,
}

#[cfg(unix)]
mod unix {
    use std::fs::{self, DirBuilder};
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::{Arc, Condvar, Mutex, mpsc};
    use std::time::Duration;

    use winit::event_loop::EventLoopProxy;

    use serde_json::{Value, json};

    use crate::app::Wake;
    use crate::config;
    use crate::control::{
        AgentOperation, Operation, Request, VERSION, WireError, WireRequest, WireResponse,
    };

    /// A private socket and commands waiting for the editor event loop.
    pub struct Server {
        /// The socket removed when this window closes.
        path: PathBuf,
        /// Commands received by the listener thread.
        pending: Arc<Mutex<Vec<Request>>>,
        /// The latest window change and clients waiting for one.
        changes: Arc<(Mutex<u64>, Condvar)>,
    }

    impl Server {
        /// Opens the private local socket and starts accepting control commands.
        pub fn start(proxy: EventLoopProxy<Wake>) -> io::Result<Self> {
            let path = socket_path()?;
            if path.exists() {
                if UnixStream::connect(&path).is_ok() {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        "editor already running",
                    ));
                }
                if !fs::symlink_metadata(&path)?.file_type().is_socket() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "control path is not a socket",
                    ));
                }
                fs::remove_file(&path)?;
            }
            let listener = UnixListener::bind(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            let pending = Arc::new(Mutex::new(Vec::new()));
            let queue = pending.clone();
            let changes = Arc::new((Mutex::new(0), Condvar::new()));
            let revisions = changes.clone();
            std::thread::spawn(move || {
                for connection in listener.incoming().flatten() {
                    let queue = queue.clone();
                    let proxy = proxy.clone();
                    let revisions = revisions.clone();
                    std::thread::spawn(move || serve(connection, queue, revisions, proxy));
                }
            });
            Ok(Self {
                path,
                pending,
                changes,
            })
        }

        /// Takes commands for the window to run on its own event loop.
        pub fn take(&self) -> Vec<Request> {
            self.pending
                .lock()
                .map(|mut queue| std::mem::take(&mut *queue))
                .unwrap_or_default()
        }

        /// Advances the state revision and wakes clients waiting for changes.
        pub fn changed(&self) {
            let (revision, changed) = &*self.changes;
            if let Ok(mut revision) = revision.lock() {
                *revision = revision.wrapping_add(1);
                changed.notify_all();
            }
        }

        /// Returns the current state revision.
        pub fn revision(&self) -> u64 {
            self.changes.0.lock().map_or(0, |revision| *revision)
        }
    }

    impl Drop for Server {
        /// Removes this window's socket when the event loop exits.
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    /// Receives bounded JSON requests and writes one response for each.
    fn serve(
        mut stream: UnixStream,
        pending: Arc<Mutex<Vec<Request>>>,
        changes: Arc<(Mutex<u64>, Condvar)>,
        proxy: EventLoopProxy<Wake>,
    ) {
        let Ok(copy) = stream.try_clone() else { return };
        let mut reader = BufReader::new(copy);
        loop {
            let mut line = String::new();
            let read = (&mut reader).take(65_537).read_line(&mut line);
            let malformed =
                !matches!(&read, Ok(size) if *size > 0 && *size <= 65_536 && line.ends_with('\n'));
            let response = match read {
                Ok(0) => break,
                Ok(size) if size <= 65_536 && line.ends_with('\n') => {
                    dispatch(&line, &pending, &changes, &proxy)
                }
                _ => WireResponse {
                    version: VERSION,
                    id: None,
                    result: None,
                    error: Some(WireError {
                        code: "invalid_request",
                        message: "request must be one JSON line of at most 65536 bytes".to_owned(),
                    }),
                },
            };
            if serde_json::to_writer(&mut stream, &response).is_err()
                || stream.write_all(b"\n").is_err()
                || malformed
            {
                break;
            }
        }
    }

    /// Validates a wire request and routes it to the editor event loop.
    fn dispatch(
        line: &str,
        pending: &Arc<Mutex<Vec<Request>>>,
        changes: &Arc<(Mutex<u64>, Condvar)>,
        proxy: &EventLoopProxy<Wake>,
    ) -> WireResponse {
        let decoded = serde_json::from_str::<WireRequest>(line);
        let request = match decoded {
            Ok(request) => request,
            Err(error) => {
                return WireResponse {
                    version: VERSION,
                    id: None,
                    result: None,
                    error: Some(WireError {
                        code: "invalid_request",
                        message: error.to_string(),
                    }),
                };
            }
        };
        let id = Some(request.id);
        let result = if request.version != VERSION {
            Err(WireError {
                code: "unsupported_version",
                message: format!("protocol version {VERSION} is required"),
            })
        } else {
            route(request, pending, changes, proxy)
        };
        match result {
            Ok(result) => WireResponse {
                version: VERSION,
                id,
                result: Some(result),
                error: None,
            },
            Err(error) => WireResponse {
                version: VERSION,
                id,
                result: None,
                error: Some(error),
            },
        }
    }

    /// Interprets a supported method and waits for the editor when needed.
    fn route(
        request: WireRequest,
        pending: &Arc<Mutex<Vec<Request>>>,
        changes: &Arc<(Mutex<u64>, Condvar)>,
        proxy: &EventLoopProxy<Wake>,
    ) -> Result<Value, WireError> {
        if !request.params.is_null() && !request.params.is_object() {
            return Err(invalid("params must be an object"));
        }
        if request.method == "system.hello" {
            return Ok(json!({
                "protocol": VERSION,
                "methods": ["system.hello", "control.execute", "state.snapshot", "state.watch", "agent.transcript", "agent.catalog", "agent.start", "agent.detail", "agent.send", "agent.image.upload", "agent.cancel", "agent.answer", "agent.mode.set", "agent.knob.set", "agent.history.list", "agent.history.load", "agent.login", "agent.login.read", "agent.login.write", "agent.terminal"]
            }));
        }
        if request.method == "state.watch" {
            let after = number(&request.params, "after_revision")?;
            let timeout = match request.params.get("timeout_ms") {
                Some(timeout) => timeout
                    .as_u64()
                    .ok_or_else(|| invalid("timeout_ms must be an unsigned integer"))?,
                None => 30_000,
            }
            .min(30_000);
            let (revision, changed) = &**changes;
            let current = revision.lock().map_err(|_| unavailable())?;
            if after > *current {
                return Err(invalid("after_revision is ahead of the editor"));
            }
            let (current, _) = changed
                .wait_timeout_while(current, Duration::from_millis(timeout), |current| {
                    *current <= after
                })
                .map_err(|_| unavailable())?;
            if *current <= after {
                return Ok(json!({ "changed": false, "revision": *current }));
            }
        }
        let operation = match request.method.as_str() {
            "control.execute" => Operation::Execute(string(&request.params, "line")?.to_owned()),
            "state.snapshot" | "state.watch" => Operation::Snapshot,
            "agent.transcript" => {
                if request.params.get("id").is_some() {
                    Operation::TranscriptId(number(&request.params, "id")?)
                } else {
                    let index = usize::try_from(number(&request.params, "agent")?)
                        .map_err(|_| invalid("agent index is too large"))?;
                    Operation::Transcript(index)
                }
            }
            "agent.catalog" => Operation::Agent(AgentOperation::Catalog),
            "agent.start" => Operation::Agent(AgentOperation::Start {
                project: number(&request.params, "project_id")?,
                session: optional_number(&request.params, "session_id")?,
                agent: string(&request.params, "agent")?.to_owned(),
            }),
            "agent.detail" => {
                Operation::Agent(AgentOperation::Detail(number(&request.params, "id")?))
            }
            "agent.send" => Operation::Agent(AgentOperation::Send {
                id: number(&request.params, "id")?,
                text: string(&request.params, "text")?.to_owned(),
                files: strings(&request.params, "files")?,
                images: numbers(&request.params, "images")?,
            }),
            "agent.image.upload" => Operation::Agent(AgentOperation::UploadImage {
                id: number(&request.params, "id")?,
                image: number(&request.params, "image")?,
                mime_type: string(&request.params, "mime_type")?.to_owned(),
                name: optional_string(&request.params, "name")?,
                data: string(&request.params, "data")?.to_owned(),
                finish: request
                    .params
                    .get("finish")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("finish must be a boolean"))?,
            }),
            "agent.cancel" => {
                Operation::Agent(AgentOperation::Cancel(number(&request.params, "id")?))
            }
            "agent.answer" => Operation::Agent(AgentOperation::Answer {
                id: number(&request.params, "id")?,
                request: number(&request.params, "request_id")?,
                choice: optional_string(&request.params, "choice_id")?,
            }),
            "agent.mode.set" => Operation::Agent(AgentOperation::SetMode {
                id: number(&request.params, "id")?,
                mode: string(&request.params, "mode")?.to_owned(),
            }),
            "agent.knob.set" => Operation::Agent(AgentOperation::SetKnob {
                id: number(&request.params, "id")?,
                knob: string(&request.params, "knob")?.to_owned(),
                value: request
                    .params
                    .get("value")
                    .cloned()
                    .ok_or_else(|| invalid("value is required"))?,
            }),
            "agent.history.list" => {
                Operation::Agent(AgentOperation::ListHistory(number(&request.params, "id")?))
            }
            "agent.history.load" => Operation::Agent(AgentOperation::LoadHistory {
                id: number(&request.params, "id")?,
                saved: string(&request.params, "saved")?.to_owned(),
            }),
            "agent.login" => Operation::Agent(AgentOperation::Login {
                id: number(&request.params, "id")?,
                method: string(&request.params, "method")?.to_owned(),
            }),
            "agent.login.read" => {
                Operation::Agent(AgentOperation::LoginRead(number(&request.params, "id")?))
            }
            "agent.login.write" => Operation::Agent(AgentOperation::LoginWrite {
                id: number(&request.params, "id")?,
                input: string(&request.params, "input")?.to_owned(),
            }),
            "agent.terminal" => Operation::Agent(AgentOperation::Terminal {
                id: number(&request.params, "id")?,
                terminal: string(&request.params, "terminal")?.to_owned(),
            }),
            _ => {
                return Err(WireError {
                    code: "unknown_method",
                    message: format!("unknown method: {}", request.method),
                });
            }
        };
        let (answer, received) = mpsc::channel();
        pending
            .lock()
            .map_err(|_| unavailable())?
            .push(Request { operation, answer });
        proxy.send_event(Wake::Control).map_err(|_| unavailable())?;
        received
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| unavailable())?
            .map_err(|message| WireError {
                code: "operation_failed",
                message,
            })
    }

    /// Reads a required unsigned integer parameter.
    fn number(params: &Value, name: &str) -> Result<u64, WireError> {
        params
            .get(name)
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid(&format!("{name} must be an unsigned integer")))
    }

    /// Reads a required string parameter.
    fn string<'a>(params: &'a Value, name: &str) -> Result<&'a str, WireError> {
        params
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(&format!("{name} must be a string")))
    }

    /// Reads an optional unsigned integer parameter.
    fn optional_number(params: &Value, name: &str) -> Result<Option<u64>, WireError> {
        match params.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value
                .as_u64()
                .map(Some)
                .ok_or_else(|| invalid(&format!("{name} must be an unsigned integer or null"))),
        }
    }

    /// Reads an optional string parameter.
    fn optional_string(params: &Value, name: &str) -> Result<Option<String>, WireError> {
        match params.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value
                .as_str()
                .map(|value| Some(value.to_owned()))
                .ok_or_else(|| invalid(&format!("{name} must be a string or null"))),
        }
    }

    /// Reads an optional array of strings.
    fn strings(params: &Value, name: &str) -> Result<Vec<String>, WireError> {
        match params.get(name) {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid(&format!("{name} must contain only strings")))
                })
                .collect(),
            _ => Err(invalid(&format!("{name} must be an array of strings"))),
        }
    }

    /// Reads an optional array of unsigned integers.
    fn numbers(params: &Value, name: &str) -> Result<Vec<u64>, WireError> {
        match params.get(name) {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_u64().ok_or_else(|| {
                        invalid(&format!("{name} must contain only unsigned integers"))
                    })
                })
                .collect(),
            _ => Err(invalid(&format!(
                "{name} must be an array of unsigned integers"
            ))),
        }
    }

    /// Returns an invalid-parameter error.
    fn invalid(message: &str) -> WireError {
        WireError {
            code: "invalid_params",
            message: message.to_owned(),
        }
    }

    /// Returns an editor-unavailable error.
    fn unavailable() -> WireError {
        WireError {
            code: "unavailable",
            message: "editor did not answer".to_owned(),
        }
    }

    /// Returns the private socket path, creating its owner-only directory.
    fn socket_path() -> io::Result<PathBuf> {
        let home = config::home()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "editor home unavailable"))?;
        let directory = home.join("control");
        if !directory.exists() {
            DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&directory)?;
        }
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.uid() != fs::metadata(&home)?.uid()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "control directory must be private",
            ));
        }
        Ok(directory.join("editor.sock"))
    }

    /// Sends one JSON line to the editor and returns its JSON response line.
    fn send_raw(line: &str) -> Result<String, String> {
        let path = socket_path().map_err(|error| error.to_string())?;
        let mut stream =
            UnixStream::connect(path).map_err(|error| format!("editor unavailable: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(45)))
            .map_err(|error| error.to_string())?;
        stream
            .write_all(format!("{line}\n").as_bytes())
            .map_err(|error| error.to_string())?;
        let mut answer = String::new();
        BufReader::new(stream)
            .read_line(&mut answer)
            .map_err(|error| error.to_string())?;
        if answer.is_empty() {
            return Err("editor closed the control connection".to_owned());
        }
        Ok(answer)
    }

    /// Sends a terminal command over versioned JSON and returns its text.
    fn send(line: &str) -> Result<String, String> {
        let request = WireRequest {
            version: VERSION,
            id: 1,
            method: "control.execute".to_owned(),
            params: json!({ "line": line }),
        };
        let request = serde_json::to_string(&request).map_err(|error| error.to_string())?;
        let answer = send_raw(&request)?;
        let answer = serde_json::from_str::<Value>(&answer).map_err(|error| error.to_string())?;
        if let Some(error) = answer.get("error") {
            return Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("control request failed")
                .to_owned());
        }
        answer
            .get("result")
            .and_then(|result| result.get("text"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "editor returned an invalid command response".to_owned())
    }

    /// Relays JSON lines between an SSH session and the private socket.
    fn proxy_stdio() {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            match send_raw(&line) {
                Ok(answer) => print!("{answer}"),
                Err(error) => {
                    eprintln!("{error}");
                    break;
                }
            }
        }
    }

    /// Runs one command, or reads commands interactively from the SSH terminal.
    pub fn client(args: Vec<String>) {
        if args.as_slice() == ["--stdio"] {
            proxy_stdio();
            return;
        }
        if !args.is_empty() {
            match send(&args.join(" ")) {
                Ok(text) => println!("{text}"),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        println!("Pandemonium control. Type help for commands, quit to leave.");
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if matches!(line.trim(), "quit" | "exit") {
                break;
            }
            match send(&line) {
                Ok(text) => println!("{text}"),
                Err(error) => eprintln!("{error}"),
            }
        }
    }
}

#[cfg(unix)]
pub use unix::{Server, client};

#[cfg(not(unix))]
/// No Unix control socket exists on this platform.
pub struct Server;

#[cfg(not(unix))]
impl Server {
    /// Reports that the local SSH control socket is unavailable here.
    pub fn start(_proxy: EventLoopProxy<Wake>) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "SSH control requires Unix",
        ))
    }

    /// No commands can arrive on this platform.
    pub fn take(&self) -> Vec<Request> {
        Vec::new()
    }

    /// No state watcher is available on this platform.
    pub fn changed(&self) {}

    /// No state revision is available on this platform.
    pub fn revision(&self) -> u64 {
        0
    }
}

#[cfg(not(unix))]
/// Reports that the terminal control client needs a Unix host.
pub fn client(_args: Vec<String>) {
    eprintln!("SSH control requires Unix");
    std::process::exit(1);
}
