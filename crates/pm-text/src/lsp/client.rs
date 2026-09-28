//! One language server: the process, what it has been told, and what it says.
//!
//! The client is deliberately half a protocol. It opens documents, keeps
//! them in step and collects diagnostics; it asks the server for nothing
//! else. A server is a process that can die, refuse to start or never answer
//! — none of which is an error the editor reports, because a file opens and
//! edits the same either way.

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lsp_types::{DiagnosticSeverity, PublishDiagnosticsParams};
use ropey::Rope;
use serde_json::{Value, json};

use crate::cursor::Position;
use crate::diagnostic::{Diagnostic, Severity};
use crate::frame;
use crate::language::Server;
use crate::lsp::answer::{self, Answer, Request};
use crate::lsp::encoding::{Encoding, Files};
use crate::lsp::outbox::{Outbox, Outgoing};
use crate::lsp::uri;
use crate::lsp::watch::{Watched, Watchers};
use crate::program::path_beside;
use crate::syntax::Highlight;

/// The request identifier the handshake is sent under.
const INITIALIZE: i64 = 1;

/// The identifier the first question after the handshake is asked under.
const FIRST_REQUEST: i64 = 2;

/// How long a server told to exit is given to do so before it is killed.
const EXIT_GRACE: Duration = Duration::from_millis(500);

/// How often a server told to exit is looked at, to see whether it has.
const EXIT_POLL: Duration = Duration::from_millis(20);

/// The semantic token types the editor understands, in the protocol's words.
///
/// A server hands back a legend of its own, in its own order, of the types
/// it will use out of these; a type the editor did not ask for is one it
/// will not be sent.
const TOKEN_TYPES: [&str; 23] = [
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "macro",
    "keyword",
    "modifier",
    "comment",
    "string",
    "number",
    "regexp",
    "operator",
    "decorator",
];

/// A question asked of a server, for as long as it is unanswered.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Asked(i64);

/// What a running server has told us, and what it has not been told yet.
#[derive(Default)]
struct State {
    /// Whether the server's output has ended or shutdown was requested.
    dead: bool,
    /// Whether the handshake has been answered.
    ready: bool,
    /// Messages held back until it has been, with only the latest text of
    /// each file among them.
    queued: Vec<Outgoing>,
    /// The diagnostics last published, per file.
    diagnostics: HashMap<PathBuf, Vec<Diagnostic>>,
    /// The questions asked and not yet answered, and what they were about.
    asked: HashMap<i64, (Request, PathBuf)>,
    /// The answers that have come back and not yet been collected.
    answers: HashMap<i64, Answer>,
    /// Whether anything has arrived since the editor last looked.
    fresh: bool,
    /// What the server said its semantic token types are, in its own order.
    legend: Vec<Option<Highlight>>,
    /// How the server counts a column, as the handshake settled it.
    encoding: Encoding,
    /// The text of each open file, as the server was last told it.
    texts: HashMap<PathBuf, Rope>,
    /// The files on disk the server has asked to hear about.
    watchers: Watchers,
    /// What the server said it can do, once the handshake has said it.
    capabilities: Option<Value>,
}

/// A language server the editor is talking to.
pub struct Client {
    /// The process itself, kept so that it can be ended.
    process: Mutex<Option<Child>>,
    /// When this process was started, for measuring sustained operation.
    started: Instant,
    /// Where messages for the server are handed to its writer thread.
    outbox: Outbox,
    /// What the server has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// The identifier the next question will be asked under.
    next: AtomicI64,
}

impl Client {
    /// Starts `program` as `server` over `root`, waking the window through
    /// `notify`.
    ///
    /// The handshake goes out here and is answered on the reader thread, so
    /// starting a server never blocks the frame that asked for one.
    pub fn start(
        root: &Path,
        program: &Path,
        server: Server,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let mut process = Command::new(program)
            .args(server.arguments)
            .env("PATH", path_beside(program))
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let outbox = Outbox::start(process.stdin.take().expect("stdin was piped"));
        let stdout = process.stdout.take().expect("stdout was piped");
        let state = Arc::new(Mutex::new(State::default()));

        outbox.send(Outgoing::Message(json!({
            "jsonrpc": "2.0",
            "id": INITIALIZE,
            "method": "initialize",
            "params": initialize(root, server),
        })));
        let client = Self {
            process: Mutex::new(Some(process)),
            started: Instant::now(),
            outbox: outbox.clone(),
            state: state.clone(),
            next: AtomicI64::new(FIRST_REQUEST),
        };

        let reader = Reader {
            state,
            notify,
            outbox,
            stdout: BufReader::new(stdout),
        };
        std::thread::spawn(move || reader.run());

        Ok(client)
    }

    /// Tells the server a file is open, what language it is in and what is
    /// in it.
    ///
    /// The language comes with the file rather than with the server: one
    /// clangd serves a checkout's C and its C++ alike, and each document
    /// says which of the two it is.
    pub fn did_open(&self, path: &Path, language_id: &str, version: i32, text: &str) {
        self.record(path, Rope::from_str(text));
        self.notify(json!({
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri::of(path),
                    "languageId": language_id,
                    "version": version,
                    "text": text,
                },
            },
        }));
    }

    /// Tells the server what a file now holds.
    ///
    /// The whole text goes every time. Ranged changes save bytes on a pipe
    /// that is not short of them, and cost a second representation of every
    /// edit that has to agree with the first one exactly. The rope is shared
    /// with the buffer, not copied, and becomes a message only on the writer
    /// thread, so a keystroke costs the window nothing the size of the file.
    pub fn did_change(&self, path: &Path, version: i32, text: &Rope) {
        self.record(path, text.clone());
        self.post(Outgoing::Change {
            uri: uri::of(path),
            version,
            text: text.clone(),
        });
    }

    /// Tells the server a file has been written to disk.
    pub fn did_save(&self, path: &Path, text: &str) {
        self.notify(json!({
            "method": "textDocument/didSave",
            "params": {
                "textDocument": { "uri": uri::of(path) },
                "text": text,
            },
        }));
    }

    /// Tells the server a file is about to be written to disk, if it asked.
    pub fn will_save(&self, path: &Path) {
        let asked = self.state.lock().ok().is_some_and(|state| {
            state.capabilities.as_ref().is_some_and(|capabilities| {
                capabilities["textDocumentSync"]["willSave"] == json!(true)
            })
        });
        if asked {
            self.notify(json!({
                "method": "textDocument/willSave",
                "params": { "textDocument": { "uri": uri::of(path) }, "reason": 1 },
            }));
        }
    }

    /// Whether the server offers to answer `request`.
    ///
    /// A server still starting up has not said, and is taken to offer it:
    /// what is asked before the handshake is answered waits for it, and a
    /// server that turns out not to answer refuses it then.
    pub fn offers(&self, request: &Request) -> bool {
        self.state.lock().ok().is_none_or(|state| {
            if state.dead {
                return false;
            }
            state
                .capabilities
                .as_ref()
                .is_none_or(|capabilities| request.is_offered(capabilities))
        })
    }

    /// Tells the server a file is no longer open.
    pub fn did_close(&self, path: &Path) {
        if let Ok(mut state) = self.state.lock() {
            state.texts.remove(path);
        }
        self.notify(json!({
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": uri::of(path) } },
        }));
    }

    /// Tells the server which of `changes` on disk it asked to hear about.
    ///
    /// A server that registered for none of them is sent nothing.
    pub fn watched(&self, changes: &[(PathBuf, Watched)]) {
        let notification = match self.state.lock() {
            Ok(state) => state.watchers.notification(changes),
            Err(_) => None,
        };
        if let Some(notification) = notification {
            self.notify(notification);
        }
    }

    /// Asks the server `request` about `at` in the file at `path`.
    ///
    /// The answer is not waited for: a question goes out, the reader thread
    /// takes the reply down, and the window collects it on the wake that
    /// follows. Nothing the editor asks a server may hold a frame up.
    pub fn ask(&self, request: Request, path: &Path, at: Position) -> Asked {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mut files = files(&self.state);
        let outgoing = request.encoded(path, &mut files);
        let message = json!({
            "id": id,
            "method": outgoing.method(),
            "params": outgoing.params(path, files.encode(path, at)),
        });
        if let Ok(mut state) = self.state.lock() {
            state.asked.insert(id, (request, path.to_path_buf()));
        }

        self.notify(message);
        Asked(id)
    }

    /// The answer to `asked`, once it has come back, taken off the list.
    pub fn answer(&self, asked: Asked) -> Option<Answer> {
        self.state.lock().ok()?.answers.remove(&asked.0)
    }

    /// Gives up on `asked`, for a question whose answer is no longer wanted.
    pub fn forget(&self, asked: Asked) {
        if let Ok(mut state) = self.state.lock() {
            state.asked.remove(&asked.0);
            state.answers.remove(&asked.0);
        }
    }

    /// What the server last said about `path`.
    pub fn diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.diagnostics.get(path).cloned())
            .unwrap_or_default()
    }

    /// How many errors the server has published, across every file it has
    /// said anything about.
    pub fn errors(&self) -> usize {
        self.state.lock().map_or(0, |state| {
            state
                .diagnostics
                .values()
                .flatten()
                .filter(|found| found.severity == Severity::Error)
                .count()
        })
    }

    /// Whether anything has arrived since this was last asked.
    pub fn take_fresh(&self) -> bool {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.fresh))
            .unwrap_or_default()
    }

    /// Whether this server has stopped answering.
    pub fn is_dead(&self) -> bool {
        self.state.lock().is_ok_and(|state| state.dead)
    }

    /// Whether a completed handshake ran long enough to break an exit streak.
    pub fn was_stable(&self) -> bool {
        self.started.elapsed() >= Duration::from_secs(30)
            && self.state.lock().is_ok_and(|state| state.ready)
    }

    /// Asks the server to shut down and reaps its process after a short grace period.
    pub fn shutdown(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.dead = true;
            state.queued.clear();
            state.diagnostics.clear();
            state.fresh = true;
        }
        let Some(process) = self
            .process
            .lock()
            .ok()
            .and_then(|mut process| process.take())
        else {
            return;
        };
        self.outbox.send(Outgoing::Message(json!({
            "jsonrpc": "2.0", "id": self.next.fetch_add(1, Ordering::Relaxed), "method": "shutdown"
        })));
        self.outbox.send(Outgoing::Message(
            json!({ "jsonrpc": "2.0", "method": "exit" }),
        ));
        std::thread::spawn(move || reap(process));
    }

    /// Keeps the text the server was last told, for counting columns by.
    fn record(&self, path: &Path, text: Rope) {
        if let Ok(mut state) = self.state.lock() {
            state.texts.insert(path.to_path_buf(), text);
        }
    }

    /// Sends a notification or a request, holding it back until the
    /// handshake is answered.
    fn notify(&self, mut message: Value) {
        message["jsonrpc"] = json!("2.0");
        self.post(Outgoing::Message(message));
    }

    /// Hands `outgoing` to the writer, or holds it back until the handshake
    /// is answered.
    ///
    /// The decision and the handing over happen under one lock, the same
    /// one the handshake's answer lets the held-back messages go under, so
    /// nothing sent after the handshake can overtake what was held before.
    fn post(&self, outgoing: Outgoing) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.dead {
            return;
        }
        match state.ready {
            true => self.outbox.send(outgoing),
            false => hold(&mut state.queued, outgoing),
        }
    }
}

impl Drop for Client {
    /// Tells the server to exit when the last document it served is gone,
    /// and ends it if it has not within [`EXIT_GRACE`].
    ///
    /// Nothing here waits: the exit is handed to the writer thread, and the
    /// waiting on the process happens on a thread of its own.
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Waits a moment for a server told to exit to do so, then kills it and
/// takes down its exit either way.
fn reap(mut process: Child) {
    let told = Instant::now();
    while told.elapsed() < EXIT_GRACE {
        if !matches!(process.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(EXIT_POLL);
    }
    let _ = process.kill();
    let _ = process.wait();
}

/// Holds `outgoing` back among `queued`, until the handshake is answered.
///
/// A file's whole text replaces the last one held for it, when nothing
/// else about that file has been held since: a server that takes its time
/// starting is told the text as it stands, not every keystroke on the way.
fn hold(queued: &mut Vec<Outgoing>, outgoing: Outgoing) {
    if let Outgoing::Change { uri, .. } = &outgoing {
        let last = queued.iter().rposition(|held| held.is_about(uri));
        if let Some(index) = last.filter(|&index| queued[index].is_change()) {
            queued[index] = outgoing;
            return;
        }
    }
    queued.push(outgoing);
}

/// The thread reading everything the server says.
struct Reader {
    /// What the server has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// How the window is woken once something has arrived.
    notify: Arc<dyn Fn() + Send + Sync>,
    /// Where messages for the server are handed to its writer thread.
    outbox: Outbox,
    /// The pipe the server writes on.
    stdout: BufReader<std::process::ChildStdout>,
}

impl Reader {
    /// Reads until the server stops talking, then refuses what it left unanswered.
    ///
    /// A server that exits has answered everything it will: a question still
    /// waiting on it is a refusal, and so is every question asked of it after.
    /// A `tsc` too old to know `--lsp` exits at once, and a save must not wait
    /// on it for ever.
    fn run(mut self) {
        while let Ok(Some(message)) = frame::read(&mut self.stdout) {
            self.dispatch(&message);
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.capabilities = Some(Value::Null);
        state.dead = true;
        state.queued.clear();
        state.diagnostics.clear();
        let unanswered = state.asked.drain().map(|(id, _)| id).collect::<Vec<_>>();
        for id in unanswered {
            state.answers.insert(id, Answer::Refused);
        }
        state.fresh = true;
        drop(state);
        (self.notify)();
    }

    /// Acts on one message: a handshake answer, a diagnostic or a request.
    ///
    /// A request the editor has nothing to say to is still answered: a
    /// server that is kept waiting for a reply it asked for will sooner or
    /// later stop sending diagnostics.
    fn dispatch(&self, message: &Value) {
        if message.get("id").is_some() && message.get("method").is_none() {
            if message["id"] == json!(INITIALIZE) {
                self.ready(&message["result"]);
            } else if let Some(id) = message["id"].as_i64() {
                match message.get("error") {
                    Some(_) => self.gave_up(id),
                    None => self.answered(id, &message["result"]),
                }
            }
            return;
        }
        match message["method"].as_str() {
            Some("textDocument/publishDiagnostics") => self.publish(message["params"].clone()),
            Some("client/registerCapability") => {
                self.with_watchers(|watchers| watchers.register(&message["params"]));
                self.acknowledge(message);
            }
            Some("client/unregisterCapability") => {
                self.with_watchers(|watchers| watchers.unregister(&message["params"]));
                self.acknowledge(message);
            }
            Some(_) if message.get("id").is_some() => self.acknowledge(message),
            _ => {}
        }
    }

    /// Answers the request `message` with nothing, which is all it needs.
    fn acknowledge(&self, message: &Value) {
        self.outbox.send(Outgoing::Message(json!({
            "jsonrpc": "2.0",
            "id": message["id"].clone(),
            "result": Value::Null,
        })));
    }

    /// Runs `change` over the files the server has asked to hear about.
    fn with_watchers(&self, change: impl FnOnce(&mut Watchers)) {
        if let Ok(mut state) = self.state.lock() {
            change(&mut state.watchers);
        }
    }

    /// Completes the handshake and lets the held-back messages go.
    ///
    /// They are handed over under the state's lock, so a message the window
    /// sends the moment the handshake is marked answered queues behind them.
    fn ready(&self, result: &Value) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.ready = true;
        state.legend = answer::legend(&result["capabilities"]);
        state.encoding = Encoding::of(&result["capabilities"]);
        state.capabilities = Some(result["capabilities"].clone());

        self.outbox.send(Outgoing::Message(json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {},
        })));
        for outgoing in std::mem::take(&mut state.queued) {
            self.outbox.send(outgoing);
        }
    }

    /// Takes down the answer to one question the editor asked.
    ///
    /// A reply to a question nobody is waiting for any more is dropped: the
    /// file may have been closed, or the cursor moved on, between the asking
    /// and the answering.
    fn answered(&self, id: i64, result: &Value) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some((request, path)) = state.asked.remove(&id) else {
            return;
        };
        let legend = state.legend.clone();
        drop(state);

        let mut files = files(&self.state);
        let mut answer = request.read(&path, result, &legend);
        answer.decode(&path, &mut files);

        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.answers.insert(id, answer);
        state.fresh = true;
        drop(state);
        (self.notify)();
    }

    /// Takes down that the server answered a question with an error.
    ///
    /// An error is not a short answer: a rename the server refused has not
    /// renamed nothing, it has failed, and reporting it as no edits would be
    /// reporting a refusal as a success. It is still an answer, though, and
    /// a save waiting on the server hears it and goes ahead.
    fn gave_up(&self, id: i64) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.asked.remove(&id).is_none() {
            return;
        }
        state.answers.insert(id, Answer::Refused);
        state.fresh = true;
        drop(state);
        (self.notify)();
    }

    /// Takes down what the server has said about one file.
    fn publish(&self, params: Value) {
        let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(params) else {
            return;
        };
        let Some(path) = uri::path(params.uri.as_str()) else {
            return;
        };
        let mut files = files(&self.state);
        let diagnostics = params
            .diagnostics
            .iter()
            .map(|published| {
                let mut found = diagnostic(published);
                found.range = files.decode_span(&path, found.range);
                found
            })
            .collect();

        if let Ok(mut state) = self.state.lock() {
            state.diagnostics.insert(path, diagnostics);
            state.fresh = true;
        }
        (self.notify)();
    }
}

/// The files an answer's positions are counted against, as they were sent.
fn files(state: &Mutex<State>) -> Files {
    match state.lock() {
        Ok(state) => Files::new(state.encoding, state.texts.clone()),
        Err(_) => Files::new(Encoding::default(), HashMap::new()),
    }
}

/// One published diagnostic, in the editor's own terms.
fn diagnostic(published: &lsp_types::Diagnostic) -> Diagnostic {
    Diagnostic {
        range: position(published.range.start)..position(published.range.end),
        severity: match published.severity {
            Some(DiagnosticSeverity::WARNING) => Severity::Warning,
            Some(DiagnosticSeverity::INFORMATION) => Severity::Information,
            Some(DiagnosticSeverity::HINT) => Severity::Hint,
            _ => Severity::Error,
        },
        message: published.message.clone(),
        source: published.source.clone(),
    }
}

/// One end of a published range, in the editor's own terms.
fn position(published: lsp_types::Position) -> Position {
    Position::new(published.line as usize, published.character as usize)
}

/// What the editor tells a server about itself when it starts one.
fn initialize(root: &Path, server: Server) -> Value {
    let options = serde_json::from_str::<Value>(server.options).unwrap_or(Value::Null);
    json!({
        "initializationOptions": options,
        "processId": std::process::id(),
        "clientInfo": { "name": "Pandemonium" },
        "rootUri": uri::of(root),
        "workspaceFolders": [{
            "uri": uri::of(root),
            "name": root.file_name().unwrap_or_default().to_string_lossy(),
        }],
        "general": { "positionEncodings": ["utf-8", "utf-16"] },
        "capabilities": {
            "textDocument": {
                "synchronization": {
                    "didSave": true,
                    "willSave": true,
                    "willSaveWaitUntil": true,
                    "dynamicRegistration": false,
                },
                "publishDiagnostics": { "relatedInformation": false },
                "definition": { "linkSupport": true },
                "typeDefinition": { "linkSupport": true },
                "implementation": { "linkSupport": true },
                "declaration": { "linkSupport": true },
                "references": {},
                "hover": { "contentFormat": ["markdown", "plaintext"] },
                "completion": {
                    "completionItem": { "snippetSupport": true },
                    "contextSupport": false,
                },
                "signatureHelp": {},
                "codeAction": {},
                "rename": { "prepareSupport": false },
                "formatting": {},
                "documentSymbol": { "hierarchicalDocumentSymbolSupport": true },
                "documentHighlight": {},
                "codeLens": {},
                "callHierarchy": {},
                "inlayHint": { "resolveSupport": { "properties": [] } },
                "semanticTokens": {
                    "requests": { "full": true },
                    "formats": ["relative"],
                    "tokenTypes": TOKEN_TYPES,
                    "tokenModifiers": [],
                },
            },
            "workspace": {
                "workspaceEdit": { "documentChanges": true },
                "symbol": {},
                "didChangeWatchedFiles": {
                    "dynamicRegistration": true,
                    "relativePatternSupport": true,
                },
            },
            "window": { "workDoneProgress": true },
        },
    })
}
