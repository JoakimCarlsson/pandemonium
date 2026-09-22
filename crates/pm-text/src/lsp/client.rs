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
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use lsp_types::{DiagnosticSeverity, PublishDiagnosticsParams};
use ropey::Rope;
use serde_json::{Value, json};

use crate::cursor::Position;
use crate::diagnostic::{Diagnostic, Severity};
use crate::language::Server;
use crate::lsp::answer::{self, Answer, Request};
use crate::lsp::encoding::{Encoding, Files};
use crate::lsp::{transport, uri};
use crate::syntax::Highlight;

/// The request identifier the handshake is sent under.
const INITIALIZE: i64 = 1;

/// The identifier the first question after the handshake is asked under.
const FIRST_REQUEST: i64 = 2;

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
    /// Whether the handshake has been answered.
    ready: bool,
    /// Notifications held back until it has been.
    queued: Vec<Value>,
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
}

/// A language server the editor is talking to.
pub struct Client {
    /// The process itself, kept so that it can be ended.
    process: Mutex<Child>,
    /// The pipe messages are written to, shared with the reader thread.
    stdin: Arc<Mutex<ChildStdin>>,
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
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = process.stdin.take().expect("stdin was piped");
        let stdout = process.stdout.take().expect("stdout was piped");
        let state = Arc::new(Mutex::new(State::default()));

        let client = Self {
            process: Mutex::new(process),
            stdin: Arc::new(Mutex::new(stdin)),
            state: state.clone(),
            next: AtomicI64::new(FIRST_REQUEST),
        };
        client.send(&json!({
            "jsonrpc": "2.0",
            "id": INITIALIZE,
            "method": "initialize",
            "params": initialize(root, server),
        }));

        let reader = Reader {
            state,
            notify,
            answers: Answers {
                stdin: client.stdin.clone(),
            },
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
        self.record(path, text);
        self.notify(&json!({
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
    /// edit that has to agree with the first one exactly.
    pub fn did_change(&self, path: &Path, version: i32, text: &str) {
        self.record(path, text);
        self.notify(&json!({
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri::of(path), "version": version },
                "contentChanges": [{ "text": text }],
            },
        }));
    }

    /// Tells the server a file has been written to disk.
    pub fn did_save(&self, path: &Path, text: &str) {
        self.notify(&json!({
            "method": "textDocument/didSave",
            "params": {
                "textDocument": { "uri": uri::of(path) },
                "text": text,
            },
        }));
    }

    /// Tells the server a file is no longer open.
    pub fn did_close(&self, path: &Path) {
        if let Ok(mut state) = self.state.lock() {
            state.texts.remove(path);
        }
        self.notify(&json!({
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": uri::of(path) } },
        }));
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

        self.notify(&message);
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

    /// Whether anything has arrived since this was last asked.
    pub fn take_fresh(&self) -> bool {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.fresh))
            .unwrap_or_default()
    }

    /// Keeps the text the server was last told, for counting columns by.
    fn record(&self, path: &Path, text: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.texts.insert(path.to_path_buf(), Rope::from_str(text));
        }
    }

    /// Sends a notification, holding it back until the handshake is answered.
    fn notify(&self, message: &Value) {
        let mut message = message.clone();
        message["jsonrpc"] = json!("2.0");

        let queued = match self.state.lock() {
            Ok(mut state) if !state.ready => {
                state.queued.push(message.clone());
                true
            }
            _ => false,
        };
        if !queued {
            self.send(&message);
        }
    }

    /// Writes one message to the server, dropping it if the pipe has gone.
    fn send(&self, message: &Value) {
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = transport::write(&mut *stdin, message);
        }
    }
}

impl Drop for Client {
    /// Ends the server process when the last document it served is gone.
    fn drop(&mut self) {
        self.send(&json!({ "jsonrpc": "2.0", "method": "exit" }));
        if let Ok(mut process) = self.process.lock() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

/// The pipe the reader thread writes its answers on.
struct Answers {
    /// The same handle on the server's standard input the client writes to.
    stdin: Arc<Mutex<ChildStdin>>,
}

impl Answers {
    /// Writes one message, dropping it if the pipe has gone.
    fn send(&self, message: &Value) {
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = transport::write(&mut *stdin, message);
        }
    }
}

/// The thread reading everything the server says.
struct Reader {
    /// What the server has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// How the window is woken once something has arrived.
    notify: Arc<dyn Fn() + Send + Sync>,
    /// The pipe the server is answered on.
    answers: Answers,
    /// The pipe the server writes on.
    stdout: BufReader<std::process::ChildStdout>,
}

impl Reader {
    /// Reads until the server stops talking.
    fn run(mut self) {
        while let Ok(Some(message)) = transport::read(&mut self.stdout) {
            self.dispatch(&message);
        }
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
            Some(_) if message.get("id").is_some() => self.answers.send(&json!({
                "jsonrpc": "2.0",
                "id": message["id"].clone(),
                "result": Value::Null,
            })),
            _ => {}
        }
    }

    /// Completes the handshake and lets the held-back notifications go.
    fn ready(&self, result: &Value) {
        let queued = match self.state.lock() {
            Ok(mut state) => {
                state.ready = true;
                state.legend = answer::legend(&result["capabilities"]);
                state.encoding = Encoding::of(&result["capabilities"]);
                std::mem::take(&mut state.queued)
            }
            Err(_) => return,
        };

        self.answers.send(&json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {},
        }));
        for message in queued {
            self.answers.send(&message);
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

    /// Forgets a question the server answered with an error.
    ///
    /// An error is not a short answer: a rename the server refused has not
    /// renamed nothing, it has failed, and reporting it as no edits would be
    /// reporting a refusal as a success.
    fn gave_up(&self, id: i64) {
        if let Ok(mut state) = self.state.lock() {
            state.asked.remove(&id);
        }
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
                "synchronization": { "didSave": true, "dynamicRegistration": false },
                "publishDiagnostics": { "relatedInformation": false },
                "definition": { "linkSupport": true },
                "typeDefinition": { "linkSupport": true },
                "implementation": { "linkSupport": true },
                "declaration": { "linkSupport": true },
                "references": {},
                "hover": { "contentFormat": ["plaintext", "markdown"] },
                "completion": {
                    "completionItem": { "snippetSupport": true },
                    "contextSupport": false,
                },
                "signatureHelp": {},
                "codeAction": {},
                "rename": { "prepareSupport": false },
                "formatting": {},
                "documentSymbol": { "hierarchicalDocumentSymbolSupport": true },
                "inlayHint": { "resolveSupport": { "properties": [] } },
                "semanticTokens": {
                    "requests": { "full": true },
                    "formats": ["relative"],
                    "tokenTypes": TOKEN_TYPES,
                    "tokenModifiers": [],
                },
            },
            "workspace": { "workspaceEdit": { "documentChanges": true } },
            "window": { "workDoneProgress": true },
        },
    })
}
