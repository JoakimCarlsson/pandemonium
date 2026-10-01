//! One multiplexed SSH transport, shared by every project on a host.

use crate::command::{ArcProcess, Process, Spec};
use crate::wire::{self, Frame, Reply, Request};
use crate::{Child, CommandBuilder, Pty, PtyChild, PtyControl, Stdio, Watcher};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

/// The deadline for a control or file operation.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Raw chunks in flight per stream, bounded to four megabytes.
const QUEUED: usize = 512;

/// A stable remote identity whose connection may be replaced explicitly.
pub(crate) struct Remote {
    /// The system SSH alias.
    pub name: String,
    /// The current connection generation.
    connection: Mutex<Option<Arc<Connection>>>,
    /// The private SSH multiplex socket used for interactive authentication.
    control: Mutex<Option<std::path::PathBuf>>,
}
impl Remote {
    /// Validates an SSH destination before it reaches the command line.
    pub fn new(name: &str) -> io::Result<Self> {
        if name.is_empty()
            || name.starts_with('-')
            || name.chars().any(|c| {
                c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | ';' | '`' | '$')
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid SSH host",
            ));
        }
        Ok(Self {
            name: name.to_owned(),
            connection: Mutex::new(None),
            control: Mutex::new(None),
        })
    }
    /// Builds the local SSH login terminal, preserving system SSH configuration.
    pub fn authentication(&self) -> io::Result<CommandBuilder> {
        let directory = std::env::home_dir()
            .ok_or_else(|| io::Error::other("No home directory for SSH control socket"))?
            .join(".cache/pandemonium/ssh");
        std::fs::create_dir_all(&directory)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let socket = directory.join(format!("{}-{nonce:x}", std::process::id()));
        let mut command = CommandBuilder::new("ssh");
        command.args(["-M", "-S"]);
        command.arg(&socket);
        command.args(["-o", "ControlPersist=60", "-T", &self.name, "true"]);
        if let Some(previous) = self.control.lock().unwrap().replace(socket) {
            end_master(self.name.clone(), previous);
        }
        Ok(command)
    }

    /// Starts SSH and requires the endpoint's exact package version.
    pub fn connect(&self) -> io::Result<()> {
        if let Some(connection) = self.connection.lock().unwrap().take() {
            connection.close();
        }
        let mut command = std::process::Command::new("ssh");
        command.arg("-T");
        if let Some(control) = self.control.lock().unwrap().as_ref() {
            command.arg("-S").arg(control);
        }
        let mut child = command
            .args([&self.name, "pandemonium-server --stdio"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("SSH has no input"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("SSH has no output"))?;
        let stderr = child.stderr.take();
        let connection = Arc::new(Connection {
            writer: Mutex::new(Box::new(input)),
            pending: Mutex::new(HashMap::new()),
            flows: Mutex::new(HashMap::new()),
            streams: Mutex::new(HashMap::new()),
            next: AtomicU32::new(1),
            alive: AtomicBool::new(true),
            child: Mutex::new(Some(child)),
            errors: Mutex::new(String::new()),
        });
        if let Some(mut stderr) = stderr {
            let target = Arc::downgrade(&connection);
            std::thread::spawn(move || {
                let mut bytes = [0; 1024];
                while let Ok(count) = stderr.read(&mut bytes) {
                    if count == 0 {
                        break;
                    }
                    if let Some(connection) = target.upgrade() {
                        let mut errors = connection.errors.lock().unwrap();
                        if errors.len() < 8192 {
                            errors.push_str(&String::from_utf8_lossy(&bytes[..count]));
                        }
                    } else {
                        break;
                    }
                }
            });
        }
        Connection::listen(&connection, output);
        let handshake = connection.request("hello", json!({}));
        let version = handshake
            .as_ref()
            .ok()
            .and_then(|value| value["version"].as_str());
        if version != Some(env!("CARGO_PKG_VERSION")) {
            let found = version.unwrap_or("not installed");
            let detail = connection.errors.lock().unwrap().clone();
            connection.close();
            return Err(io::Error::other(format!(
                "pandemonium-server {} is needed on {}, found {found}. Build with `cargo build -p pm-server --release`, then `scp target/release/pandemonium-server {}:.local/bin/pandemonium-server` (create ~/.local/bin and put it on the remote PATH). {detail}",
                env!("CARGO_PKG_VERSION"),
                self.name,
                self.name
            )));
        }
        *self.connection.lock().unwrap() = Some(connection);
        Ok(())
    }
    /// Whether SSH is still serving the current generation.
    pub fn connected(&self) -> bool {
        self.connection
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|connection| connection.alive.load(Ordering::Acquire))
    }
    /// Borrows the current connected generation without implicitly redialing.
    fn connection(&self) -> io::Result<Arc<Connection>> {
        self.connection
            .lock()
            .unwrap()
            .as_ref()
            .filter(|connection| connection.alive.load(Ordering::Acquire))
            .cloned()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    format!("Disconnected from {}", self.name),
                )
            })
    }
    /// Performs a JSON control operation.
    pub fn request(&self, op: &str, args: Value) -> io::Result<Value> {
        self.connection()?.request(op, args)
    }
    /// Reads raw file bytes on a request channel.
    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let connection = self.connection()?;
        let channel = connection.channel();
        let mut reader = connection.reader(channel, true);
        connection.request_on(channel, "read", json!({"path":path}))?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
    /// Writes raw bytes before waiting for the write acknowledgement.
    pub fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let connection = self.connection()?;
        let channel = connection.channel();
        let bytes = bytes.to_vec();
        let path = path.to_path_buf();
        let (sender, completed) = mpsc::channel();
        let writing = connection.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let reply = writing.begin(channel, "write", json!({"path":path}))?;
                let mut writer = StreamWriter::new(writing.clone(), channel);
                writer.write_all(&bytes)?;
                drop(writer);
                writing.finish(reply).map(drop)
            })();
            let _ = sender.send(result);
        });
        match completed.recv_timeout(TIMEOUT) {
            Ok(result) => result,
            Err(_) => {
                connection.close();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "remote file save timed out",
                ))
            }
        }
    }
    /// Starts a command with independently multiplexed standard streams.
    pub fn spawn(&self, spec: &Spec) -> io::Result<Child> {
        let connection = self.connection()?;
        let process = connection.channel();
        let input = connection.channel();
        let output = connection.channel();
        let error = connection.channel();
        let stdout = matches!(spec.stdout, Stdio::Piped)
            .then(|| Box::new(connection.reader(output, false)) as crate::command::Reader);
        let stderr = matches!(spec.stderr, Stdio::Piped)
            .then(|| Box::new(connection.reader(error, false)) as crate::command::Reader);
        connection.request(
            "spawn",
            json!({"spec":spec,"channel":process,"input":input,"output":output,"error":error}),
        )?;
        Ok(Child {
            stdin: matches!(spec.stdin, Stdio::Piped)
                .then(|| Box::new(StreamWriter::new(connection.clone(), input)) as crate::Input),
            stdout,
            stderr,
            process: Process::Remote(ArcProcess {
                connection,
                channel: process,
            }),
        })
    }
    /// Opens a remote terminal and wires its byte streams.
    pub fn pty(&self, command: CommandBuilder, cols: usize, rows: usize) -> io::Result<Pty> {
        let connection = self.connection()?;
        let channel = connection.channel();
        let input = connection.channel();
        let output = connection.channel();
        let reader = Box::new(connection.reader(output, false));
        let argv = command
            .get_argv()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let env = command.iter_extra_env_as_str().collect::<Vec<_>>();
        connection.request("pty", json!({"channel":channel,"input":input,"output":output,"argv":argv,"env":env,"cwd":command.get_cwd().map(|path| path.to_string_lossy()),"cols":cols,"rows":rows}))?;
        Ok(Pty {
            control: PtyControl::Remote {
                connection: connection.clone(),
                channel,
            },
            child: PtyChild::Remote {
                connection: connection.clone(),
                channel,
            },
            reader,
            writer: Box::new(StreamWriter::new(connection, input)),
        })
    }
    /// Performs the whole walk on the host, streaming paths as events.
    pub fn walk(
        &self,
        root: &Path,
        mut found: impl FnMut(std::path::PathBuf) -> bool,
    ) -> io::Result<()> {
        let connection = self.connection()?;
        let channel = connection.channel();
        let events = connection.subscribe(channel);
        connection.request("walk", json!({"root":root,"channel":channel}))?;
        while let Ok(frame) = events.recv_timeout(TIMEOUT) {
            if frame.kind == wire::EOF {
                break;
            }
            connection.credit(channel)?;
            let path = serde_json::from_slice(&frame.payload)?;
            if !found(path) {
                break;
            }
        }
        connection.streams.lock().unwrap().remove(&channel);
        connection.request("cancel_stream", json!({"channel":channel}))?;
        Ok(())
    }
    /// Watches a remote root and publishes the same settled disk shape.
    pub fn watch(&self, root: &Path, wake: Arc<dyn Fn() + Send + Sync>) -> Watcher {
        let Ok(connection) = self.connection() else {
            return Watcher::empty();
        };
        let channel = connection.channel();
        let events = connection.subscribe(channel);
        if connection
            .request("watch", json!({"root":root,"channel":channel}))
            .is_err()
        {
            return Watcher::empty();
        }
        Watcher::remote(events, connection, channel, wake)
    }
}

/// One SSH generation; old streams never attach to a reconnected process.
pub struct Connection {
    /// Serialized writes to SSH's standard input.
    writer: Mutex<Box<dyn Write + Send>>,
    /// Outstanding control replies.
    pending: Mutex<HashMap<u32, Sender<Reply>>>,
    /// Credit windows for outgoing raw streams.
    flows: Mutex<HashMap<u32, Arc<crate::flow::Flow>>>,
    /// Bounded process and event subscribers.
    streams: Mutex<HashMap<u32, SyncSender<Frame>>>,
    /// The next request or stream id.
    next: AtomicU32,
    /// Whether reads and writes can still succeed.
    alive: AtomicBool,
    /// The SSH child to terminate on close.
    child: Mutex<Option<std::process::Child>>,
    /// SSH diagnostics for failed handshakes.
    errors: Mutex<String>,
}
impl Connection {
    /// Allocates a unique logical channel.
    fn channel(&self) -> u32 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
    /// Attaches a bounded receiver before an operation can produce bytes.
    fn subscribe(&self, channel: u32) -> Receiver<Frame> {
        let (sender, receiver) = mpsc::sync_channel(QUEUED);
        self.streams.lock().unwrap().insert(channel, sender);
        receiver
    }
    /// Returns an owned reader for a raw byte channel.
    fn reader(self: &Arc<Self>, channel: u32, bounded: bool) -> StreamReader {
        StreamReader {
            receiver: self.subscribe(channel),
            buffered: io::Cursor::new(Vec::new()),
            connection: Arc::downgrade(self),
            channel,
            ended: false,
            deadline: bounded.then(|| std::time::Instant::now() + TIMEOUT),
        }
    }
    /// Returns a consumed incoming frame's credit to the endpoint.
    pub(crate) fn credit(&self, channel: u32) -> io::Result<()> {
        self.send(Frame {
            kind: wire::CREDIT,
            channel,
            payload: Vec::new(),
        })
    }

    /// Writes one frame while holding the shared writer lock.
    pub(crate) fn send(&self, frame: Frame) -> io::Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "Disconnected"));
        }
        frame.write(&mut *self.writer.lock().unwrap())
    }
    /// Registers and sends a request before waiting for its reply.
    fn begin(&self, channel: u32, op: &str, args: Value) -> io::Result<Receiver<Reply>> {
        let (sender, receiver) = mpsc::channel();
        self.pending.lock().unwrap().insert(channel, sender);
        if let Err(error) = self.send(Frame::json(
            wire::REQUEST,
            channel,
            &Request {
                op: op.to_owned(),
                args,
            },
        )?) {
            self.pending.lock().unwrap().remove(&channel);
            return Err(error);
        }
        Ok(receiver)
    }
    /// Waits at most ten seconds for a control reply.
    fn finish(&self, reply: Receiver<Reply>) -> io::Result<Value> {
        let reply = reply.recv_timeout(TIMEOUT).map_err(|error| {
            io::Error::new(
                if matches!(error, mpsc::RecvTimeoutError::Timeout) {
                    io::ErrorKind::TimedOut
                } else {
                    io::ErrorKind::NotConnected
                },
                "host operation timed out or disconnected",
            )
        })?;
        if let Some((kind, message)) = reply.error {
            return Err(io::Error::new(error_kind(&kind), message));
        }
        Ok(reply.value)
    }
    /// Sends a request on an allocated channel.
    fn request_on(&self, channel: u32, op: &str, args: Value) -> io::Result<Value> {
        let result = self.finish(self.begin(channel, op, args)?);
        self.pending.lock().unwrap().remove(&channel);
        result
    }
    /// Performs one control operation with a bounded deadline.
    pub(crate) fn request(&self, op: &str, args: Value) -> io::Result<Value> {
        self.request_on(self.channel(), op, args)
    }
    /// Reads and dispatches replies and raw channels until SSH closes.
    fn listen(connection: &Arc<Self>, mut reader: impl Read + Send + 'static) {
        let weak = Arc::downgrade(connection);
        std::thread::spawn(move || {
            while let Ok(Some(frame)) = Frame::read(&mut reader) {
                let Some(connection) = weak.upgrade() else {
                    break;
                };
                if frame.kind == wire::CREDIT {
                    if let Some(flow) = connection.flows.lock().unwrap().get(&frame.channel) {
                        flow.give();
                    }
                } else if frame.kind == wire::REPLY {
                    if let Ok(reply) = serde_json::from_slice(&frame.payload) {
                        if let Some(sender) =
                            connection.pending.lock().unwrap().remove(&frame.channel)
                        {
                            let _ = sender.send(reply);
                        }
                    } else {
                        break;
                    }
                } else {
                    let sender = connection
                        .streams
                        .lock()
                        .unwrap()
                        .get(&frame.channel)
                        .cloned();
                    if let Some(sender) = sender {
                        let channel = frame.channel;
                        let ended = frame.kind == wire::EOF;
                        match sender.try_send(frame) {
                            Ok(()) => {
                                if ended {
                                    connection.streams.lock().unwrap().remove(&channel);
                                }
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => {
                                connection.streams.lock().unwrap().remove(&channel);
                            }
                            Err(mpsc::TrySendError::Full(_)) => {
                                connection.close();
                                break;
                            }
                        }
                    }
                }
            }
            if let Some(connection) = weak.upgrade() {
                connection.close();
            }
        });
    }
    /// Ends every outstanding operation and stream when SSH exits.
    fn close(&self) {
        self.alive.store(false, Ordering::Release);
        self.pending.lock().unwrap().clear();
        for flow in self.flows.lock().unwrap().values() {
            flow.close();
        }
        self.flows.lock().unwrap().clear();
        self.streams.lock().unwrap().clear();
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for Connection {
    /// Reaps SSH after the last machine reference is released.
    fn drop(&mut self) {
        self.close();
    }
}

/// Reads ordered raw chunks without allocating an unbounded queue.
struct StreamReader {
    /// The channel's incoming frames.
    receiver: Receiver<Frame>,
    /// The current partially read chunk.
    buffered: io::Cursor<Vec<u8>>,
    /// The transport, weak so a stream cannot keep SSH alive forever.
    connection: Weak<Connection>,
    /// The stream id to unsubscribe on drop.
    channel: u32,
    /// Whether the peer sent a clean EOF.
    ended: bool,
    /// The absolute file operation deadline; process pipes have none.
    deadline: Option<std::time::Instant>,
}
impl Read for StreamReader {
    /// Reads a chunk, distinguishing disconnect from clean stream completion.
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        if target.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.buffered.read(target)?;
            if read > 0 || self.ended {
                return Ok(read);
            }
            let frame = match self.deadline {
                Some(deadline) => self
                    .receiver
                    .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    .map_err(|error| {
                        io::Error::new(
                            if matches!(error, mpsc::RecvTimeoutError::Timeout) {
                                io::ErrorKind::TimedOut
                            } else {
                                io::ErrorKind::NotConnected
                            },
                            "remote file read timed out or disconnected",
                        )
                    })?,
                None => self
                    .receiver
                    .recv()
                    .map_err(|_| io::Error::new(io::ErrorKind::NotConnected, "Disconnected"))?,
            };
            if frame.kind == wire::EOF {
                self.ended = true;
            } else {
                if let Some(connection) = self.connection.upgrade() {
                    connection.credit(self.channel)?;
                }
                self.buffered = io::Cursor::new(frame.payload);
            }
        }
    }
}
impl Drop for StreamReader {
    /// Removes an abandoned stream from the dispatch table.
    fn drop(&mut self) {
        if let Some(connection) = self.connection.upgrade() {
            connection.streams.lock().unwrap().remove(&self.channel);
            if !self.ended {
                let _ = connection
                    .request("cancel_stream", serde_json::json!({"channel":self.channel}));
            }
        }
    }
}

/// Writes raw chunks to a process, terminal or file channel.
pub(crate) struct StreamWriter {
    /// The owning generation.
    connection: Arc<Connection>,
    /// The raw channel.
    channel: u32,
    /// The endpoint's remaining receive credits.
    flow: Arc<crate::flow::Flow>,
}
impl StreamWriter {
    /// Opens a raw writer.
    fn new(connection: Arc<Connection>, channel: u32) -> Self {
        let flow = Arc::new(crate::flow::Flow::new());
        connection
            .flows
            .lock()
            .unwrap()
            .insert(channel, flow.clone());
        Self {
            connection,
            channel,
            flow,
        }
    }
}
impl Write for StreamWriter {
    /// Writes one bounded raw chunk.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(8192);
        if count == 0 {
            return Ok(0);
        }
        self.flow.acquire()?;
        self.connection.send(Frame {
            kind: wire::BYTES,
            channel: self.channel,
            payload: bytes[..count].to_vec(),
        })?;
        Ok(count)
    }
    /// Flushes the connection writer.
    fn flush(&mut self) -> io::Result<()> {
        self.connection.writer.lock().unwrap().flush()
    }
}
impl Drop for StreamWriter {
    /// Closes the input stream independently of process lifetime.
    fn drop(&mut self) {
        self.connection.flows.lock().unwrap().remove(&self.channel);
        let _ = self.connection.send(Frame {
            kind: wire::EOF,
            channel: self.channel,
            payload: Vec::new(),
        });
    }
}

/// Restores common portable filesystem error kinds.
fn error_kind(kind: &str) -> io::ErrorKind {
    match kind {
        "NotFound" => io::ErrorKind::NotFound,
        "AlreadyExists" => io::ErrorKind::AlreadyExists,
        "PermissionDenied" => io::ErrorKind::PermissionDenied,
        "Unsupported" => io::ErrorKind::Unsupported,
        "InvalidInput" => io::ErrorKind::InvalidInput,
        _ => io::ErrorKind::Other,
    }
}

impl Drop for Remote {
    /// Ends the SSH session and its authentication master when the host is released.
    fn drop(&mut self) {
        if let Some(connection) = self.connection.get_mut().unwrap().take() {
            connection.close();
        }
        if let Some(control) = self.control.get_mut().unwrap().take() {
            end_master(self.name.clone(), control);
        }
    }
}

/// Closes an authentication master without holding the window thread.
fn end_master(name: String, control: std::path::PathBuf) {
    std::thread::spawn(move || {
        let _ = std::process::Command::new("ssh")
            .arg("-S")
            .arg(&control)
            .args(["-O", "exit", &name])
            .output();
        let _ = std::fs::remove_file(control);
    });
}
