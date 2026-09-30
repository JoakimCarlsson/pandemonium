//! One language server: the process, what it has been told, and what it says.
//!
//! The client opens documents, keeps them in step, collects diagnostics and
//! handles server requests. A server can die, refuse to start or never answer
//! — none of which is an error the editor reports, because a file opens and
//! edits the same either way. What it says about why goes to its log.

use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lsp_types::notification::{
    DidChangeConfiguration, DidCloseTextDocument, DidCreateFiles, DidDeleteFiles,
    DidOpenTextDocument, DidRenameFiles, DidSaveTextDocument, Exit, Initialized, LogMessage,
    Notification, Progress as ProgressNotification, PublishDiagnostics, ShowMessage,
    WillSaveTextDocument,
};
use lsp_types::request::{
    ApplyWorkspaceEdit, CodeLensRefresh, DocumentDiagnosticRequest, ExecuteCommand, Initialize,
    InlayHintRefreshRequest, RegisterCapability, Request as LspRequest,
    SemanticTokensFullDeltaRequest, SemanticTokensFullRequest, SemanticTokensRefresh, ShowDocument,
    ShowMessageRequest, Shutdown, UnregisterCapability, WorkDoneProgressCreate,
    WorkspaceConfiguration, WorkspaceDiagnosticRefresh, WorkspaceFoldersRequest,
};
use lsp_types::{
    ApplyWorkspaceEditResponse, CreateFilesParams, DeleteFilesParams, DiagnosticSeverity,
    DidChangeConfigurationParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportKind, DocumentDiagnosticReportResult, ExecuteCommandParams, FileCreate,
    FileDelete, FileRename, InitializeResult, InitializedParams, MessageType, PartialResultParams,
    RenameFilesParams, SemanticToken, SemanticTokensDeltaParams, SemanticTokensEdit,
    SemanticTokensFullDeltaResult, SemanticTokensResult, ShowDocumentResult,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentSaveReason, TextDocumentSyncKind,
    WillSaveTextDocumentParams, WorkDoneProgressParams,
};
use ropey::Rope;
use serde_json::Value;

use crate::cursor::Position;
use crate::diagnostic::{Diagnostic, Severity};
use crate::frame;
use crate::language::Server;
use crate::lsp::answer::{self, Answer, Asking, Request};
use crate::lsp::capabilities::{self, Capabilities, Document};
use crate::lsp::encoding::{Encoding, Files};
use crate::lsp::log::Log;
use crate::lsp::outbox::{Outbox, Outgoing};
use crate::lsp::progress::{Progress, Works};
use crate::lsp::rpc::{self, Incoming};
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

/// A question asked of a server, for as long as it is unanswered.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Asked(i64);

/// What a server said is wrong with one file, as it said it and as the
/// editor draws it.
#[derive(Default)]
struct Faults {
    /// As the server said it, to hand back when asking it for fixes.
    wire: Vec<lsp_types::Diagnostic>,
    /// In the editor's own terms.
    shown: Vec<Diagnostic>,
}

/// What a server said is wrong with one file when asked, and the id of
/// that report to ask after it by.
#[derive(Default)]
struct Pulled {
    /// The id the server gave the report, if it gave one.
    result_id: Option<String>,
    /// What it reported.
    faults: Faults,
}

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
    /// The diagnostics the server published, per file.
    pushed: HashMap<PathBuf, Faults>,
    /// The diagnostics the server reported when asked, per file.
    pulled: HashMap<PathBuf, Pulled>,
    /// The diagnostics asked for and not yet reported, and which file each is of.
    pulling: HashMap<i64, PathBuf>,
    /// Workspace edits awaiting the window's application and reply.
    workspace_edits: Vec<WorkspaceEditRequest>,
    /// Settings given to this server at startup.
    options: Value,
    /// The name under which this server's settings are requested.
    options_section: String,
    /// The questions asked and not yet answered, and what they were about.
    asked: HashMap<i64, (Request, PathBuf)>,
    /// The answers that have come back and not yet been collected.
    answers: HashMap<i64, Answer>,
    /// Annotation kinds the server has asked the editor to refresh.
    refreshes: Vec<Request>,
    /// Whether anything has arrived since the editor last looked.
    fresh: bool,
    /// What the server said its semantic token types are, in its own order.
    legend: Vec<Option<Highlight>>,
    /// How the server counts a column, as the handshake settled it.
    encoding: Encoding,
    /// The text of each open file, as the server was last told it.
    texts: HashMap<PathBuf, Rope>,
    /// The language each open file was opened as, in the protocol's words.
    languages: HashMap<PathBuf, String>,
    /// The files on disk the server has asked to hear about.
    watchers: Watchers,
    /// What the server has said it can do.
    capabilities: Capabilities,
    /// The work the server says it is doing.
    works: Works,
    /// What the server has asked to be shown that went wrong, not yet shown.
    troubles: Vec<String>,
    /// The semantic tokens last sent for each file, under the id the server
    /// gave them, for asking only what changed since.
    tokens: HashMap<PathBuf, (String, Vec<SemanticToken>)>,
    /// The semantic token questions asked as what changed since the last.
    deltas: HashSet<i64>,
}

impl State {
    /// The open document at `path`, as a server's selectors pick documents out.
    fn document<'a>(&'a self, path: &'a Path) -> Document<'a> {
        Document {
            path,
            language: self.languages.get(path).map(String::as_str),
        }
    }
}

/// An edit requested by a server, awaiting application by the window.
pub struct WorkspaceEditRequest {
    /// The changes the server requested, in the order they are to be made.
    pub edits: Vec<crate::WorkspaceChange>,
    /// The request identifier to answer after applying them.
    id: Value,
    /// Whether the request contained only supported text edits.
    pub supported: bool,
}

/// What a client and its reader both send through: the pipe, and the
/// counter every question's id is taken from.
#[derive(Clone)]
struct Wire {
    /// Where messages for the server are handed to its writer thread.
    outbox: Outbox,
    /// The identifier the next question will be asked under.
    next: Arc<AtomicI64>,
}

impl Wire {
    /// A fresh identifier to ask a question under.
    fn id(&self) -> i64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

/// A language server the editor is talking to.
pub struct Client {
    /// The command it was started as, which is what the reader calls it.
    name: &'static str,
    /// The worktree whose paths this server watches.
    root: PathBuf,
    /// The process itself, kept so that it can be ended.
    process: Mutex<Option<Child>>,
    /// When this process was started, for measuring sustained operation.
    started: Instant,
    /// Where messages for the server go.
    wire: Wire,
    /// What the server has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// Where what the server says besides its answers is written down.
    log: Log,
}

impl Client {
    /// Starts `program` as `server` over `root`, waking the window through
    /// `notify` and writing what it says to its log in `logs`.
    ///
    /// The handshake goes out here and is answered on the reader thread, so
    /// starting a server never blocks the frame that asked for one.
    pub fn start(
        root: &Path,
        program: &Path,
        server: Server,
        notify: Arc<dyn Fn() + Send + Sync>,
        logs: Option<&Path>,
    ) -> std::io::Result<Self> {
        let log = Log::open(logs, root, server.command);
        let spawned = Command::new(program)
            .args(server.arguments)
            .env("PATH", path_beside(program))
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut process = match spawned {
            Ok(process) => process,
            Err(error) => {
                log.write(&format!("could not start {}: {error}", program.display()));
                return Err(error);
            }
        };
        if let Some(stderr) = process.stderr.take() {
            log.follow(stderr);
        }

        let outbox = Outbox::start(process.stdin.take().expect("stdin was piped"), log.clone());
        let stdout = process.stdout.take().expect("stdout was piped");
        let state = Arc::new(Mutex::new(State {
            options: serde_json::from_str(server.options).unwrap_or(Value::Null),
            options_section: server.command.to_owned(),
            ..State::default()
        }));
        let wire = Wire {
            outbox,
            next: Arc::new(AtomicI64::new(FIRST_REQUEST)),
        };

        wire.outbox
            .send(Outgoing::Message(rpc::request::<Initialize>(
                INITIALIZE,
                capabilities::initialize(root, server),
            )));
        let reader = Reader {
            root: root.to_path_buf(),
            state: state.clone(),
            notify,
            wire: wire.clone(),
            log: log.clone(),
            stdout: BufReader::new(stdout),
        };
        std::thread::spawn(move || reader.run());

        Ok(Self {
            name: server.command,
            root: root.to_path_buf(),
            process: Mutex::new(Some(process)),
            started: Instant::now(),
            wire,
            state,
            log,
        })
    }

    /// The command the server was started as.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Where the server's log is kept, when it is.
    pub fn log_path(&self) -> Option<&Path> {
        self.log.path()
    }

    /// Whether the server's log has grown since this was last asked.
    pub fn take_log_grown(&self) -> bool {
        self.log.take_grown()
    }

    /// Tells the server a file is open, what language it is in and what is
    /// in it.
    ///
    /// The language comes with the file rather than with the server: one
    /// clangd serves a checkout's C and its C++ alike, and each document
    /// says which of the two it is.
    pub fn did_open(&self, path: &Path, language_id: &str, version: i32, text: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.texts.insert(path.to_path_buf(), Rope::from_str(text));
        state
            .languages
            .insert(path.to_path_buf(), language_id.to_owned());
        let message = rpc::notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri::typed(path),
                language_id.to_owned(),
                version,
                text.to_owned(),
            ),
        });
        post(&mut state, &self.wire, Outgoing::Message(message));
        pull(&mut state, &self.wire, path);
    }

    /// Tells the server what a file now holds.
    ///
    /// A server that asked for ranged changes is told the span that changed
    /// since it was last told, one that asked for whole texts the whole text,
    /// and one that asked for neither nothing at all. The rope is shared with
    /// the buffer, not copied, and what changed in it is worked out only on
    /// the writer thread, so a keystroke costs the window nothing the size of
    /// the file.
    pub fn did_change(&self, path: &Path, version: i32, text: &Rope) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let before = state.texts.insert(path.to_path_buf(), text.clone());
        let sync = match state.ready {
            true => state.capabilities.sync(state.document(path)),
            false => TextDocumentSyncKind::FULL,
        };
        let before = match sync {
            TextDocumentSyncKind::INCREMENTAL => before,
            TextDocumentSyncKind::FULL => None,
            _ => return,
        };
        let change = Outgoing::Change {
            uri: uri::typed(path),
            version,
            before,
            after: text.clone(),
            encoding: state.encoding,
        };
        post(&mut state, &self.wire, change);
        pull(&mut state, &self.wire, path);
    }

    /// Tells the server a file has been written to disk, if it asked, with
    /// what was written if it asked for that too.
    ///
    /// A server whose dependencies run between files is asked again about
    /// every other file it has open: what they say may have changed with it.
    pub fn did_save(&self, path: &Path, text: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let wanted = match state.capabilities.is_known() {
            true => state.capabilities.wants_saved(state.document(path)),
            false => Some(true),
        };
        let Some(with_text) = wanted else {
            return;
        };
        let message = rpc::notification::<DidSaveTextDocument>(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri::typed(path)),
            text: with_text.then(|| text.to_owned()),
        });
        post(&mut state, &self.wire, Outgoing::Message(message));
        let others = state
            .texts
            .keys()
            .filter(|open| open.as_path() != path)
            .filter(|open| {
                state
                    .capabilities
                    .pulls(state.document(open))
                    .is_some_and(|options| options.inter_file_dependencies)
            })
            .cloned()
            .collect::<Vec<_>>();
        for other in others {
            pull(&mut state, &self.wire, &other);
        }
    }

    /// Tells the server a file is about to be written to disk, if it asked.
    pub fn will_save(&self, path: &Path) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if !state.capabilities.wants_will_save(state.document(path)) {
            return;
        }
        let message = rpc::notification::<WillSaveTextDocument>(WillSaveTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri::typed(path)),
            reason: TextDocumentSaveReason::MANUAL,
        });
        post(&mut state, &self.wire, Outgoing::Message(message));
    }

    /// Whether the server offers to answer `request` about the file at `path`.
    ///
    /// A server still starting up has not said, and is taken to offer it:
    /// what is asked before the handshake is answered waits for it, and a
    /// server that turns out not to answer refuses it then.
    pub fn offers(&self, request: &Request, path: &Path) -> bool {
        self.state.lock().ok().is_none_or(|state| {
            !state.dead && request.is_offered(&state.capabilities, Some(state.document(path)))
        })
    }

    /// Hands the server the settings `options`, a JSON object, when they are
    /// not the ones it already has.
    ///
    /// The settings go in the notification itself, and are what the server
    /// is answered with when it asks for them after.
    pub fn configure(&self, options: &str) {
        let options = serde_json::from_str::<Value>(options).unwrap_or(Value::Null);
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.options == options {
            return;
        }
        state.options = options.clone();
        let message = rpc::notification::<DidChangeConfiguration>(DidChangeConfigurationParams {
            settings: options,
        });
        post(&mut state, &self.wire, Outgoing::Message(message));
    }

    /// Tells the server that `paths` were made, those of them it asked to
    /// hear about.
    pub fn did_create(&self, paths: &[PathBuf]) {
        let files = self.wanted("workspace/didCreateFiles", paths.iter());
        if files.is_empty() {
            return;
        }
        self.post_message(rpc::notification::<DidCreateFiles>(CreateFilesParams {
            files: files
                .into_iter()
                .map(|path| FileCreate { uri: uri::of(path) })
                .collect(),
        }));
    }

    /// Tells the server that files and folders were moved, those of them it
    /// asked to hear about.
    pub fn did_rename(&self, moves: &[(PathBuf, PathBuf)]) {
        let wanted = self.wanted("workspace/didRenameFiles", moves.iter().map(|(_, to)| to));
        let files = moves
            .iter()
            .filter(|(_, to)| wanted.contains(&to))
            .map(|(from, to)| FileRename {
                old_uri: uri::of(from),
                new_uri: uri::of(to),
            })
            .collect::<Vec<_>>();
        if files.is_empty() {
            return;
        }
        self.post_message(rpc::notification::<DidRenameFiles>(RenameFilesParams {
            files,
        }));
    }

    /// Tells the server that `paths` were taken away, those of them it asked
    /// to hear about.
    pub fn did_delete(&self, paths: &[PathBuf]) {
        let files = self.wanted("workspace/didDeleteFiles", paths.iter());
        if files.is_empty() {
            return;
        }
        self.post_message(rpc::notification::<DidDeleteFiles>(DeleteFilesParams {
            files: files
                .into_iter()
                .map(|path| FileDelete { uri: uri::of(path) })
                .collect(),
        }));
    }

    /// The paths among `paths` the server asked to hear `method` about.
    fn wanted<'a>(
        &self,
        method: &str,
        paths: impl Iterator<Item = &'a PathBuf>,
    ) -> Vec<&'a PathBuf> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        paths
            .filter(|path| state.capabilities.file_operation(method, path))
            .collect()
    }

    /// Hands one message to the writer, or holds it until the handshake.
    fn post_message(&self, message: Value) {
        if let Ok(mut state) = self.state.lock() {
            post(&mut state, &self.wire, Outgoing::Message(message));
        }
    }

    /// The characters the server formats after in the file at `path`.
    pub fn on_type_triggers(&self, path: &Path) -> Vec<char> {
        self.state
            .lock()
            .map(|state| {
                state
                    .capabilities
                    .on_type_triggers(Some(state.document(path)))
            })
            .unwrap_or_default()
    }

    /// The characters the server completes after in the file at `path`.
    pub fn completion_triggers(&self, path: &Path) -> Vec<char> {
        self.state
            .lock()
            .map(|state| {
                state
                    .capabilities
                    .completion_triggers(Some(state.document(path)))
            })
            .unwrap_or_default()
    }

    /// The characters that start or refresh signature help in the file at `path`.
    pub fn signature_triggers(&self, path: &Path, showing: bool) -> Vec<char> {
        self.state
            .lock()
            .map(|state| {
                state
                    .capabilities
                    .signature_triggers(Some(state.document(path)), showing)
            })
            .unwrap_or_default()
    }

    /// Tells the server a file is no longer open.
    pub fn did_close(&self, path: &Path) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.texts.remove(path);
        state.languages.remove(path);
        state.tokens.remove(path);
        state.pushed.remove(path);
        state.pulled.remove(path);
        let pulling = state
            .pulling
            .iter()
            .filter(|(_, pulled)| pulled.as_path() == path)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in pulling {
            state.pulling.remove(&id);
            post(&mut state, &self.wire, Outgoing::Message(cancel(id)));
        }
        state.fresh = true;
        let message = rpc::notification::<DidCloseTextDocument>(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri::typed(path)),
        });
        post(&mut state, &self.wire, Outgoing::Message(message));
    }

    /// Tells the server which of `changes` on disk it asked to hear about.
    ///
    /// A server that registered for none of them is sent nothing.
    pub fn watched(&self, changes: &[(PathBuf, Watched)]) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if let Some(notification) = state.watchers.notification(&self.root, changes) {
            post(&mut state, &self.wire, Outgoing::Message(notification));
        }
    }

    /// Asks the server `request` about `at` in the file at `path`.
    ///
    /// The answer is not waited for: a question goes out, the reader thread
    /// takes the reply down, and the window collects it on the wake that
    /// follows. `indent` supplies the file's formatting options. Nothing
    /// the editor asks a server may hold a frame up.
    pub fn ask(
        &self,
        request: Request,
        path: &Path,
        at: Position,
        indent: crate::Indent,
        selection: Range<Position>,
    ) -> Asked {
        let id = self.wire.id();
        let mut files = files(&self.state);
        let encoded = request.encoded(path, &mut files);
        let selection = files.encode(path, selection.start)..files.encode(path, selection.end);
        let at = files.encode(path, at);
        let Ok(mut state) = self.state.lock() else {
            return Asked(id);
        };
        let diagnostics = match request {
            Request::CodeActions => state
                .pushed
                .get(path)
                .into_iter()
                .map(|faults| &faults.wire)
                .chain(state.pulled.get(path).map(|pulled| &pulled.faults.wire))
                .flatten()
                .filter(|found| {
                    let first = found.range.start.line as usize;
                    let last = found.range.end.line as usize;
                    first <= selection.end.line && last >= selection.start.line
                })
                .cloned()
                .collect(),
            _ => Vec::new(),
        };
        let asking = Asking {
            path,
            at,
            selection,
            indent,
            diagnostics,
        };
        let previous = match request {
            Request::Semantics if state.capabilities.sends_semantic_deltas() => {
                state.tokens.get(path).map(|(result, _)| result.clone())
            }
            _ => None,
        };
        let message = match previous {
            Some(previous_result_id) => {
                state.deltas.insert(id);
                Some(rpc::request::<SemanticTokensFullDeltaRequest>(
                    id,
                    SemanticTokensDeltaParams {
                        work_done_progress_params: WorkDoneProgressParams::default(),
                        partial_result_params: PartialResultParams::default(),
                        text_document: TextDocumentIdentifier::new(uri::typed(path)),
                        previous_result_id,
                    },
                ))
            }
            None => encoded.message(id, asking),
        };
        match message {
            Some(message) => {
                state.asked.insert(id, (request, path.to_path_buf()));
                post(&mut state, &self.wire, Outgoing::Message(message));
            }
            None => {
                state.answers.insert(id, Answer::Refused);
                state.fresh = true;
            }
        }
        Asked(id)
    }

    /// The answer to `asked`, once it has come back, taken off the list.
    pub fn answer(&self, asked: Asked) -> Option<Answer> {
        self.state.lock().ok()?.answers.remove(&asked.0)
    }

    /// Gives up on `asked`, for a question whose answer is no longer wanted.
    pub fn forget(&self, asked: Asked) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let outstanding = state.asked.remove(&asked.0).is_some();
        state.answers.remove(&asked.0);
        if outstanding {
            post(&mut state, &self.wire, Outgoing::Message(cancel(asked.0)));
        }
    }

    /// Cancels an outstanding question at the server and discards its answer.
    pub fn cancel(&self, asked: Asked) {
        self.forget(asked);
    }

    /// Takes annotation refresh requests the server has sent.
    pub fn take_refreshes(&self) -> Vec<Request> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.refreshes))
            .unwrap_or_default()
    }

    /// What the server last said about `path`, whether it published it or
    /// was asked for it.
    pub fn diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        self.state
            .lock()
            .ok()
            .map(|state| {
                state
                    .pushed
                    .get(path)
                    .into_iter()
                    .chain(state.pulled.get(path).map(|pulled| &pulled.faults))
                    .flat_map(|faults| faults.shown.iter().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Takes workspace edits the server asked the window to apply.
    pub fn take_workspace_edits(&self) -> Vec<WorkspaceEditRequest> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.workspace_edits))
            .unwrap_or_default()
    }

    /// Reports whether the window applied a server-requested workspace edit.
    pub fn answer_workspace_edit(&self, request: WorkspaceEditRequest, applied: bool) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let message = rpc::response::<ApplyWorkspaceEdit>(
            request.id,
            ApplyWorkspaceEditResponse {
                applied,
                failure_reason: None,
                failed_change: None,
            },
        );
        post(&mut state, &self.wire, Outgoing::Message(message));
    }

    /// Runs the server command attached to a chosen code action.
    pub fn execute_command(&self, command: crate::lsp::answer::Command) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let message = rpc::request::<ExecuteCommand>(
            self.wire.id(),
            ExecuteCommandParams {
                command: command.name,
                arguments: command.arguments,
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        );
        post(&mut state, &self.wire, Outgoing::Message(message));
    }

    /// How many errors the server has reported, across every file it has
    /// said anything about.
    pub fn errors(&self) -> usize {
        self.state.lock().map_or(0, |state| {
            state
                .pushed
                .values()
                .chain(state.pulled.values().map(|pulled| &pulled.faults))
                .flat_map(|faults| &faults.shown)
                .filter(|found| found.severity == Severity::Error)
                .count()
        })
    }

    /// Error diagnostics with their paths, including files not open in a pane.
    pub fn error_diagnostics(&self) -> Vec<(PathBuf, Diagnostic)> {
        self.state.lock().map_or_else(
            |_| Vec::new(),
            |state| {
                state
                    .pushed
                    .iter()
                    .chain(
                        state
                            .pulled
                            .iter()
                            .map(|(path, pulled)| (path, &pulled.faults)),
                    )
                    .flat_map(|(path, faults)| {
                        faults
                            .shown
                            .iter()
                            .filter(|diagnostic| diagnostic.severity == Severity::Error)
                            .map(|diagnostic| (path.clone(), diagnostic.clone()))
                    })
                    .collect()
            },
        )
    }

    /// The work the server says it is doing, oldest first.
    pub fn progress(&self) -> Vec<Progress> {
        self.state
            .lock()
            .map(|state| state.works.running())
            .unwrap_or_default()
    }

    /// Takes what the server asked to be shown that went wrong.
    pub fn take_troubles(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.troubles))
            .unwrap_or_default()
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
            state.pushed.clear();
            state.pulled.clear();
            state.workspace_edits.clear();
            state.works.clear();
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
        self.wire
            .outbox
            .send(Outgoing::Message(rpc::request::<Shutdown>(
                self.wire.id(),
                (),
            )));
        self.wire
            .outbox
            .send(Outgoing::Message(rpc::notification::<Exit>(())));
        std::thread::spawn(move || reap(process));
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

/// The notification calling off the question asked under `id`.
fn cancel(id: i64) -> Value {
    rpc::notification::<lsp_types::notification::Cancel>(lsp_types::CancelParams {
        id: lsp_types::NumberOrString::Number(id as i32),
    })
}

/// Hands `outgoing` to the writer, or holds it back until the handshake is
/// answered.
///
/// The decision and the handing over happen under one lock, the same one
/// the handshake's answer lets the held-back messages go under, so nothing
/// sent after the handshake can overtake what was held before.
fn post(state: &mut State, wire: &Wire, outgoing: Outgoing) {
    if state.dead {
        return;
    }
    match state.ready {
        true => wire.outbox.send(outgoing),
        false => hold(&mut state.queued, outgoing),
    }
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

/// Asks the server what is wrong with the file at `path`, if it is a server
/// that is asked rather than one that says.
///
/// A question still out about the same file is called off first: the text
/// it was about is not the text there now.
fn pull(state: &mut State, wire: &Wire, path: &Path) {
    if !state.ready || state.dead {
        return;
    }
    let Some(options) = state.capabilities.pulls(state.document(path)) else {
        return;
    };
    let stale = state
        .pulling
        .iter()
        .filter(|(_, pulled)| pulled.as_path() == path)
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    for id in stale {
        state.pulling.remove(&id);
        wire.outbox.send(Outgoing::Message(cancel(id)));
    }
    let id = wire.id();
    let message = rpc::request::<DocumentDiagnosticRequest>(
        id,
        DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier::new(uri::typed(path)),
            identifier: options.identifier,
            previous_result_id: state
                .pulled
                .get(path)
                .and_then(|pulled| pulled.result_id.clone()),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        },
    );
    state.pulling.insert(id, path.to_path_buf());
    wire.outbox.send(Outgoing::Message(message));
}

/// Asks the server again what is wrong with every file it has open.
fn pull_all(state: &mut State, wire: &Wire) {
    let open = state.texts.keys().cloned().collect::<Vec<_>>();
    for path in open {
        pull(state, wire, &path);
    }
}

/// The thread reading everything the server says.
struct Reader {
    /// The worktree the server was started over.
    root: PathBuf,
    /// What the server has said and what it is owed.
    state: Arc<Mutex<State>>,
    /// How the window is woken once something has arrived.
    notify: Arc<dyn Fn() + Send + Sync>,
    /// Where messages for the server go.
    wire: Wire,
    /// Where what the server says besides its answers is written down.
    log: Log,
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
            self.log.trace("<--", &message);
            if let Some(message) = Incoming::read(message) {
                self.dispatch(message);
            }
        }
        self.log.write("── the server stopped talking ──");
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if !state.capabilities.is_known() {
            state
                .capabilities
                .state(lsp_types::ServerCapabilities::default());
        }
        state.dead = true;
        state.queued.clear();
        state.pushed.clear();
        state.pulled.clear();
        state.pulling.clear();
        state.workspace_edits.clear();
        state.works.clear();
        let unanswered = state.asked.drain().map(|(id, _)| id).collect::<Vec<_>>();
        for id in unanswered {
            state.answers.insert(id, Answer::Refused);
        }
        state.fresh = true;
        drop(state);
        (self.notify)();
    }

    /// Acts on one message: an answer, a request or a notification.
    fn dispatch(&self, message: Incoming) {
        match message {
            Incoming::Response { id, outcome } if id == INITIALIZE => self.ready(outcome),
            Incoming::Response { id, outcome } => self.answered(id, outcome),
            Incoming::Request { id, method, params } => self.asked(id, &method, params),
            Incoming::Notification { method, params } => self.told(&method, params),
        }
    }

    /// Answers one request the server sent.
    ///
    /// A request the editor has nothing to say to is still answered: a
    /// server that is kept waiting for a reply it asked for will sooner or
    /// later stop sending diagnostics. One the editor does not know is
    /// answered as unknown, which is what the protocol asks for.
    fn asked(&self, id: Value, method: &str, params: Value) {
        match method {
            RegisterCapability::METHOD => {
                if let Some(params) = rpc::params::<RegisterCapability>(params) {
                    self.register(params.registrations);
                }
                self.send(rpc::response::<RegisterCapability>(id, ()));
            }
            UnregisterCapability::METHOD => {
                if let Some(params) = rpc::params::<UnregisterCapability>(params) {
                    self.with_state(|state| {
                        let ids = params
                            .unregisterations
                            .into_iter()
                            .map(|taken| taken.id)
                            .collect::<Vec<_>>();
                        state.watchers.unregister(&ids);
                        state.capabilities.unregister(ids);
                    });
                }
                self.send(rpc::response::<UnregisterCapability>(id, ()));
            }
            WorkDoneProgressCreate::METHOD => {
                self.send(rpc::response::<WorkDoneProgressCreate>(id, ()));
            }
            SemanticTokensRefresh::METHOD => {
                self.refresh(Request::Semantics);
                self.send(rpc::response::<SemanticTokensRefresh>(id, ()));
            }
            InlayHintRefreshRequest::METHOD => {
                self.refresh(Request::Hints(Position::default()..Position::default()));
                self.send(rpc::response::<InlayHintRefreshRequest>(id, ()));
            }
            CodeLensRefresh::METHOD => {
                self.refresh(Request::Lenses);
                self.send(rpc::response::<CodeLensRefresh>(id, ()));
            }
            WorkspaceDiagnosticRefresh::METHOD => {
                self.with_state(|state| pull_all(state, &self.wire));
                self.send(rpc::response::<WorkspaceDiagnosticRefresh>(id, ()));
            }
            WorkspaceConfiguration::METHOD => {
                let values = rpc::params::<WorkspaceConfiguration>(params)
                    .map(|params| self.configuration(params))
                    .unwrap_or_default();
                self.send(rpc::response::<WorkspaceConfiguration>(id, values));
            }
            WorkspaceFoldersRequest::METHOD => {
                self.send(rpc::response::<WorkspaceFoldersRequest>(
                    id,
                    Some(vec![capabilities::folder(&self.root)]),
                ));
            }
            ApplyWorkspaceEdit::METHOD => match rpc::params::<ApplyWorkspaceEdit>(params) {
                Some(params) => self.apply_edit(id, params),
                None => self.send(rpc::error(id, rpc::METHOD_NOT_FOUND, "malformed edit")),
            },
            ShowMessageRequest::METHOD => {
                if let Some(params) = rpc::params::<ShowMessageRequest>(params) {
                    self.show(params.typ, &params.message);
                }
                self.send(rpc::response::<ShowMessageRequest>(id, None));
            }
            ShowDocument::METHOD => {
                self.send(rpc::response::<ShowDocument>(
                    id,
                    ShowDocumentResult { success: false },
                ));
            }
            _ => self.send(rpc::error(
                id,
                rpc::METHOD_NOT_FOUND,
                "not handled by Pandemonium",
            )),
        }
    }

    /// Takes in one notification the server sent.
    fn told(&self, method: &str, params: Value) {
        match method {
            PublishDiagnostics::METHOD => {
                if let Some(params) = rpc::said::<PublishDiagnostics>(params) {
                    self.publish(params);
                }
            }
            ProgressNotification::METHOD => {
                if let Some(params) = rpc::said::<ProgressNotification>(params) {
                    let changed = self
                        .with_state(|state| {
                            let changed = state.works.report(params);
                            state.fresh |= changed;
                            changed
                        })
                        .unwrap_or(false);
                    if changed {
                        (self.notify)();
                    }
                }
            }
            LogMessage::METHOD => {
                if let Some(params) = rpc::said::<LogMessage>(params) {
                    self.log.message(params.typ, &params.message);
                }
            }
            ShowMessage::METHOD => {
                if let Some(params) = rpc::said::<ShowMessage>(params) {
                    self.show(params.typ, &params.message);
                }
            }
            _ => {}
        }
    }

    /// Writes down a message the server asked to be shown, and holds it for
    /// the window when it says something went wrong.
    fn show(&self, kind: MessageType, message: &str) {
        self.log.message(kind, message);
        if !matches!(kind, MessageType::ERROR | MessageType::WARNING) {
            return;
        }
        self.with_state(|state| {
            state.troubles.push(message.to_owned());
            state.fresh = true;
        });
        (self.notify)();
    }

    /// Hands one message to the writer.
    fn send(&self, message: Value) {
        self.wire.outbox.send(Outgoing::Message(message));
    }

    /// Runs `change` over the state, if it can be had.
    fn with_state<T>(&self, change: impl FnOnce(&mut State) -> T) -> Option<T> {
        self.state.lock().ok().map(|mut state| change(&mut state))
    }

    /// Takes in what the server registered, and asks again for whatever the
    /// registrations newly offer.
    ///
    /// A server that registers semantic tokens, hints, lenses or pulled
    /// diagnostics after the files are open has only just said it answers
    /// them: the editor asked nobody before, and asks now.
    fn register(&self, registrations: Vec<lsp_types::Registration>) {
        let methods = registrations
            .iter()
            .map(|registration| registration.method.clone())
            .collect::<Vec<_>>();
        self.with_state(|state| {
            state.watchers.register(&registrations);
            state.capabilities.register(registrations);
            state.legend = answer::legend(&state.capabilities);
            for method in &methods {
                let request = match method.as_str() {
                    "textDocument/semanticTokens" => Request::Semantics,
                    "textDocument/inlayHint" => {
                        Request::Hints(Position::default()..Position::default())
                    }
                    "textDocument/codeLens" => Request::Lenses,
                    "textDocument/diagnostic" => {
                        pull_all(state, &self.wire);
                        continue;
                    }
                    _ => continue,
                };
                state.refreshes.push(request);
                state.fresh = true;
            }
        });
        (self.notify)();
    }

    /// Answers each requested settings section in its original order.
    fn configuration(&self, params: lsp_types::ConfigurationParams) -> Vec<Value> {
        let (options, section) = self
            .with_state(|state| (state.options.clone(), state.options_section.clone()))
            .unwrap_or((Value::Null, String::new()));
        params
            .items
            .iter()
            .map(|item| {
                let asked = item.section.as_deref().unwrap_or_default();
                let asked = asked
                    .strip_prefix(&section)
                    .filter(|rest| rest.is_empty() || rest.starts_with('.'))
                    .unwrap_or(asked);
                let value = asked
                    .split('.')
                    .filter(|part| !part.is_empty())
                    .fold(&options, |value, part| &value[part]);
                if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                    Value::Null
                } else {
                    value.clone()
                }
            })
            .collect()
    }

    /// Sends a requested workspace edit to the window for application.
    fn apply_edit(&self, id: Value, params: lsp_types::ApplyWorkspaceEditParams) {
        let request = WorkspaceEditRequest {
            edits: answer::workspace_edit(&params.edit),
            supported: answer::is_supported(&params.edit),
            id,
        };
        self.with_state(|state| {
            state.workspace_edits.push(request);
            state.fresh = true;
        });
        (self.notify)();
    }

    /// Records a server request to ask again for one annotation kind.
    fn refresh(&self, request: Request) {
        self.with_state(|state| {
            state.refreshes.push(request);
            state.fresh = true;
        });
        (self.notify)();
    }

    /// Completes the handshake and lets the held-back messages go.
    ///
    /// They are handed over under the state's lock, so a message the window
    /// sends the moment the handshake is marked answered queues behind them.
    /// A server that turned the handshake down is taken to offer nothing,
    /// and says why in its log.
    fn ready(&self, outcome: Result<Value, rpc::Failure>) {
        let result = match outcome {
            Ok(result) => rpc::result::<Initialize>(result),
            Err(failure) => {
                self.log.write(&format!(
                    "the server turned the handshake down: {} ({})",
                    failure.message, failure.code
                ));
                None
            }
        };
        let InitializeResult { capabilities, .. } = result.unwrap_or_default();
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.ready = true;
        state.encoding = Encoding::of(capabilities.position_encoding.as_ref());
        state.capabilities.state(capabilities);
        state.legend = answer::legend(&state.capabilities);

        self.wire
            .outbox
            .send(Outgoing::Message(rpc::notification::<Initialized>(
                InitializedParams {},
            )));
        let queued = std::mem::take(&mut state.queued);
        for outgoing in queued {
            let silent = match &outgoing {
                Outgoing::Change { uri, .. } => uri::path_of(uri).is_some_and(|path| {
                    state.capabilities.sync(state.document(&path)) == TextDocumentSyncKind::NONE
                }),
                Outgoing::Message(_) => false,
            };
            if !silent {
                self.wire.outbox.send(outgoing);
            }
        }
        pull_all(&mut state, &self.wire);
    }

    /// Takes down one answer: to a question the window asked, or to the
    /// editor's own asking after a file's diagnostics.
    fn answered(&self, id: i64, outcome: Result<Value, rpc::Failure>) {
        let pulled = self.with_state(|state| state.pulling.remove(&id)).flatten();
        if let Some(path) = pulled {
            return self.reported(&path, outcome);
        }
        match outcome {
            Ok(result) => self.took(id, result),
            Err(failure) => self.gave_up(id, &failure),
        }
    }

    /// Takes down the answer to one question the editor asked.
    ///
    /// A reply to a question nobody is waiting for any more is dropped: the
    /// file may have been closed, or the cursor moved on, between the asking
    /// and the answering.
    fn took(&self, id: i64, result: Value) {
        let Some(Some((request, path, legend))) = self.with_state(|state| {
            let (request, path) = state.asked.remove(&id)?;
            Some((request, path, state.legend.clone()))
        }) else {
            return;
        };

        let mut files = files(&self.state);
        let read = match request {
            Request::Semantics => self.semantics(id, &path, result, &legend),
            _ => request.read(&path, result, &legend),
        };
        let mut answer = read.unwrap_or_else(|| {
            self.log.write(&format!(
                "{} was answered in a shape the editor could not read",
                request.method()
            ));
            Answer::Refused
        });
        answer.decode(&path, &mut files);

        self.with_state(|state| {
            state.answers.insert(id, answer);
            state.fresh = true;
        });
        (self.notify)();
    }

    /// What a file's semantic tokens come to, from a reply that holds all of
    /// them or one that holds what changed since the ones last sent.
    ///
    /// Tokens the server gave an id are kept under it, to ask after them by
    /// next time; a reply to what changed is made against the ones kept.
    fn semantics(
        &self,
        id: i64,
        path: &Path,
        result: Value,
        legend: &[Option<Highlight>],
    ) -> Option<Answer> {
        let (delta, kept) = self
            .with_state(|state| {
                (
                    state.deltas.remove(&id),
                    state.tokens.get(path).map(|(_, tokens)| tokens.clone()),
                )
            })
            .unwrap_or((false, None));
        let (result_id, tokens) = match delta {
            true => match rpc::result::<SemanticTokensFullDeltaRequest>(result)? {
                None => (None, Vec::new()),
                Some(SemanticTokensFullDeltaResult::Tokens(tokens)) => {
                    (tokens.result_id, tokens.data)
                }
                Some(SemanticTokensFullDeltaResult::TokensDelta(delta)) => {
                    (delta.result_id, changed(kept?, delta.edits))
                }
                Some(SemanticTokensFullDeltaResult::PartialTokensDelta { edits }) => {
                    (None, changed(kept?, edits))
                }
            },
            false => match rpc::result::<SemanticTokensFullRequest>(result)? {
                None => (None, Vec::new()),
                Some(SemanticTokensResult::Tokens(tokens)) => (tokens.result_id, tokens.data),
                Some(SemanticTokensResult::Partial(partial)) => (None, partial.data),
            },
        };
        let spans = answer::semantics(&tokens, legend);
        self.with_state(|state| match result_id {
            Some(result_id) => {
                state.tokens.insert(path.to_path_buf(), (result_id, tokens));
            }
            None => {
                state.tokens.remove(path);
            }
        });
        Some(Answer::Semantics(spans))
    }

    /// Takes down that the server answered a question with an error.
    ///
    /// An error is not a short answer: a rename the server refused has not
    /// renamed nothing, it has failed, and reporting it as no edits would be
    /// reporting a refusal as a success. It is still an answer, though, and
    /// a save waiting on the server hears it and goes ahead.
    ///
    /// Why is written to the log, unless it is only that the editor called
    /// the question off itself.
    fn gave_up(&self, id: i64, failure: &rpc::Failure) {
        let asked = self
            .with_state(|state| {
                let (request, path) = state.asked.remove(&id)?;
                if state.deltas.remove(&id) {
                    state.tokens.remove(&path);
                }
                state.answers.insert(id, Answer::Refused);
                state.fresh = true;
                Some(request)
            })
            .flatten();
        let Some(request) = asked else {
            return;
        };
        if failure.code != rpc::REQUEST_CANCELLED {
            self.log.write(&format!(
                "{} failed: {} ({})",
                request.method(),
                failure.message,
                failure.code
            ));
        }
        (self.notify)();
    }

    /// Takes down what the server reported was wrong with the file at `path`
    /// when asked, and the files it said depend on it.
    ///
    /// A report that nothing has changed keeps the last one. A server that
    /// called the question off itself and asks for it again is asked again.
    fn reported(&self, path: &Path, outcome: Result<Value, rpc::Failure>) {
        let report = match outcome {
            Ok(result) => rpc::result::<DocumentDiagnosticRequest>(result),
            Err(failure) => {
                if failure.retrigger || failure.code == rpc::SERVER_CANCELLED {
                    self.with_state(|state| {
                        if state.texts.contains_key(path) {
                            pull(state, &self.wire, path);
                        }
                    });
                }
                return;
            }
        };
        let Some(DocumentDiagnosticReportResult::Report(report)) = report else {
            return;
        };
        let (own, related) = match report {
            DocumentDiagnosticReport::Full(full) => (
                Some((
                    full.full_document_diagnostic_report.result_id,
                    full.full_document_diagnostic_report.items,
                )),
                full.related_documents,
            ),
            DocumentDiagnosticReport::Unchanged(unchanged) => (None, unchanged.related_documents),
        };
        let mut reports = own
            .map(|own| (path.to_path_buf(), own))
            .into_iter()
            .collect::<Vec<_>>();
        for (uri, report) in related.into_iter().flatten() {
            if let (Some(path), DocumentDiagnosticReportKind::Full(full)) =
                (uri::path_of(&uri), report)
            {
                reports.push((path, (full.result_id, full.items)));
            }
        }
        let mut files = files(&self.state);
        let reports = reports
            .into_iter()
            .map(|(path, (result_id, items))| {
                let faults = faults(&path, items, &mut files);
                (path, Pulled { result_id, faults })
            })
            .collect::<Vec<_>>();
        self.with_state(|state| {
            for (path, pulled) in reports {
                state.pulled.insert(path, pulled);
            }
            state.fresh = true;
        });
        (self.notify)();
    }

    /// Takes down what the server has said about one file.
    fn publish(&self, params: lsp_types::PublishDiagnosticsParams) {
        let Some(path) = uri::path_of(&params.uri) else {
            return;
        };
        let mut files = files(&self.state);
        let faults = faults(&path, params.diagnostics, &mut files);
        self.with_state(|state| {
            state.pushed.insert(path, faults);
            state.fresh = true;
        });
        (self.notify)();
    }
}

/// `tokens` with the server's `edits` made to them, latest first.
///
/// An edit counts in the protocol's integers, five to a token.
fn changed(
    mut tokens: Vec<SemanticToken>,
    mut edits: Vec<SemanticTokensEdit>,
) -> Vec<SemanticToken> {
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        let start = (edit.start / 5) as usize;
        let end = (start + (edit.delete_count / 5) as usize).min(tokens.len());
        let start = start.min(end);
        tokens.splice(start..end, edit.data.unwrap_or_default());
    }
    tokens
}

/// What a server said is wrong with the file at `path`, kept both ways.
fn faults(path: &Path, wire: Vec<lsp_types::Diagnostic>, files: &mut Files) -> Faults {
    let shown = wire
        .iter()
        .map(|published| {
            let mut found = diagnostic(published);
            found.range = files.decode_span(path, found.range);
            found
        })
        .collect();
    Faults { wire, shown }
}

/// The files an answer's positions are counted against, as they were sent.
fn files(state: &Mutex<State>) -> Files {
    match state.lock() {
        Ok(state) => Files::new(state.encoding, state.texts.clone()),
        Err(_) => Files::new(Encoding::default(), HashMap::new()),
    }
}

/// One diagnostic, in the editor's own terms.
fn diagnostic(published: &lsp_types::Diagnostic) -> Diagnostic {
    Diagnostic {
        range: answer::range(published.range),
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
