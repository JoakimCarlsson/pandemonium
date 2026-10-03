//! What the window asks a language server, and what it does with the answer.
//!
//! Nothing here waits. A command sends a question and is over; the reply
//! arrives on the reader thread, wakes the window, and is acted on then —
//! which is why every question is written down with what it was about, and
//! why an answer to a question about a file that has since been closed is
//! dropped rather than applied to whatever is open now.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use pm_core::ops;
use pm_text::{
    Answer, Asked, Calls, Client, FileEdit, Handle, Lens, Location, NamedLocation, Position,
    Request, Trigger, WorkspaceChange,
};

use crate::app::App;
use crate::app::places::Place;
use crate::editor::{Completions, FileId, Shown};
use crate::keymap::Action;
use crate::picker::{Choice, Kind, Row};

/// Why a question was asked, for an answer that is not always acted on.
///
/// The same question serves two purposes: a reader asking to be taken
/// somewhere, and the editor asking whether there is anywhere to be taken —
/// which is what a name has to have before it is drawn as a link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Purpose {
    /// The reader asked; act on the answer.
    Act,
    /// The pointer is resting on a name; only say whether it leads anywhere.
    Link,
    /// Refresh the followed file's persistent outline.
    Outline,
}

/// The name the pointer is over, and what the server made of it.
#[derive(Clone, Debug)]
pub struct Link {
    /// The file the pointer is over.
    pub file: FileId,
    /// The word under it, which the question was asked about.
    pub about: std::ops::Range<Position>,
    /// What to underline, once a server has said the name leads somewhere.
    ///
    /// The server's own span is used when it gave one: it knows where the
    /// name it resolved begins and ends better than a walk over the
    /// characters around the pointer does.
    pub span: Option<std::ops::Range<Position>>,
}

/// One question asked of a server, for as long as it is unanswered.
pub struct Pending {
    /// The server it was asked of.
    client: Arc<Client>,
    /// What it was asked under.
    asked: Asked,
    /// The file it was asked about.
    file: FileId,
    /// Where in that file it was asked about.
    at: Position,
    /// What was asked.
    request: Request,
    /// Why it was asked.
    purpose: Purpose,
    /// The buffer version this question was about.
    version: i32,
}

/// A completion put in before the server that offered it had filled it in.
///
/// What it brings along — the import a name needs — arrives when the server
/// has filled it in, and is made then, if nothing has been typed since.
pub struct TakenCompletion {
    /// The server filling it in.
    client: Arc<Client>,
    /// Its record, as the server handed it out.
    handle: Handle,
    /// The file it was put into.
    file: FileId,
    /// Where it was put in, which what it brings along must come before:
    /// whatever came after has moved since the server measured it.
    start: Position,
    /// The version the file was at once it was put in.
    version: i32,
}

/// A code action kept with the server and document version that offered it.
pub struct OfferedCodeAction {
    /// The action displayed in the menu.
    pub action: pm_text::CodeAction,
    /// The server that runs its command, when it has one.
    pub client: Arc<Client>,
    /// The document it was offered for.
    pub file: FileId,
    /// The document version it was offered for.
    pub version: i32,
}

impl App {
    /// Offers one missing installable server per launch according to the preference.
    pub(super) fn offer_missing_servers(&mut self) {
        for server in self.editor.take_missing_servers() {
            if !self.offered_servers.insert(server.command) {
                continue;
            }
            match self.preferences.install_language_servers {
                crate::config::InstallLanguageServers::Ask => {
                    let language = self.server_install_language(server.command);
                    let name = language.unwrap_or("configured");
                    self.notices.installation(
                        server.command,
                        language,
                        crate::notice::InstallationStage::Offer,
                        format!("The {name} language server {} is not installed. Would you like to install it?", server.command),
                    );
                }
                crate::config::InstallLanguageServers::Always => {
                    self.start_server_install(server.command, false);
                }
                crate::config::InstallLanguageServers::Never => {}
            }
            self.request_redraw();
        }
    }

    /// Finds an open language requesting this server before consulting the catalogue.
    fn server_install_language(&self, command: &str) -> Option<&'static str> {
        let uses = |language: &pm_text::Language| {
            crate::settings::languages::servers(*language, &self.language_servers)
                .iter()
                .any(|server| server.command == command)
        };
        self.active_file()
            .and_then(|file| file.borrow().buffer().language())
            .filter(uses)
            .or_else(|| pm_text::Language::all().into_iter().find(uses))
            .map(pm_text::Language::name)
    }

    /// Starts one pinned install away from the window thread.
    pub(super) fn start_server_install(&mut self, command: &'static str, manual: bool) {
        if self.installing_servers.contains_key(command) {
            return;
        }
        let Some(recipe) = self.configured_server_recipe(command) else {
            return;
        };
        let Some(directory) = crate::config::servers() else {
            self.notices.installation(
                command,
                self.server_install_language(command),
                crate::notice::InstallationStage::Failed,
                format!("Installing {command} needs an editor home directory."),
            );
            return;
        };
        if manual {
            self.offered_servers.insert(command);
        }
        let notice = self.notices.installation(
            command,
            self.server_install_language(command),
            crate::notice::InstallationStage::Progress,
            format!("Installing {command}…"),
        );
        self.installing_servers.insert(command, notice);
        let results = self.installed_servers.clone();
        let wake = self.waker(crate::app::Wake::Install);
        std::thread::spawn(move || {
            let result = pm_text::install::install(&directory, command, recipe).map(|_| ());
            results.lock().unwrap().push((command, result));
            wake();
        });
        self.request_redraw();
    }

    /// Reports finished installs and attaches their servers to open documents.
    pub(super) fn finish_server_installs(&mut self) {
        let finished = std::mem::take(&mut *self.installed_servers.lock().unwrap());
        for (command, result) in finished {
            self.installing_servers.remove(command);
            match result {
                Ok(()) => {
                    let started = self.editor.reopen_command(command);
                    if started
                        && let Some(directory) = crate::config::servers()
                        && let Some(recipe) = self.configured_server_recipe(command)
                    {
                        pm_text::install::prune_older(&directory, command, recipe.version());
                    }
                    if let Some(recipe) = self.configured_server_recipe(command) {
                        self.notices.installation(
                            command,
                            self.server_install_language(command),
                            crate::notice::InstallationStage::Done,
                            format!("Installed {command} {}", recipe.version()),
                        );
                    }
                }
                Err(error) => {
                    self.notices.installation(
                        command,
                        self.server_install_language(command),
                        crate::notice::InstallationStage::Failed,
                        format!("Could not install {command}: {error}"),
                    );
                }
            }
        }
        self.request_redraw();
    }

    /// Opens the log of a language server behind the focused file, or of one
    /// running over the worktree in front when no file is focused.
    ///
    /// A file served by one server opens that server's log; a file served by
    /// several offers them to choose between. The log opens at its end, which
    /// is where a server that has just misbehaved says why.
    pub(super) fn open_server_log(&mut self) {
        let logs = match self.active_file_id() {
            Some(file) => self
                .editor
                .server_states(file)
                .into_iter()
                .filter_map(|status| Some((status.command, status.log?)))
                .collect::<Vec<_>>(),
            None => self
                .scope()
                .and_then(|scope| self.root_of(scope))
                .map(|root| self.editor.server_logs_over(&root))
                .unwrap_or_default(),
        };
        match logs.as_slice() {
            [] => self
                .notices
                .trouble("No language server log is available for this file.", None),
            [(_, path)] => {
                self.open_server_log_at(path);
            }
            _ => {
                let rows = logs
                    .iter()
                    .filter_map(|(name, path)| {
                        let place = self.place_of(path, log_end(path))?;
                        Some(Row {
                            section: None,
                            label: (*name).to_owned(),
                            detail: path.display().to_string(),
                            choice: Choice::OpenAt(place.scope, place.path, place.position),
                            enabled: true,
                        })
                    })
                    .collect();
                self.open_picker_with(Kind::ServerLogs, rows, String::new());
            }
        }
    }

    /// Opens a retained server log at its last line.
    pub(super) fn open_server_log_at(&mut self, path: &std::path::Path) {
        if let Some(place) = self.place_of(path, log_end(path)) {
            self.jump_to(&place);
        }
    }

    /// Puts up what the servers asked to be shown that went wrong, each
    /// leading to the log of the server that said it.
    pub(super) fn hear_server_troubles(&mut self) {
        self.hear_server_failures();
        for client in self.editor.clients() {
            for trouble in client.take_troubles() {
                self.notices.trouble(
                    format!("{}: {trouble}", client.name()),
                    Some(crate::message::Message::OpenServerLog),
                );
            }
        }
    }

    /// Reports each abandoned server with an action targeting its retained log.
    pub(super) fn hear_server_failures(&mut self) {
        for failed in self.editor.take_server_failures() {
            if let Some(notice) = failed.failure_notice() {
                let action = failed.log.map(|path| {
                    let index = self
                        .server_failure_logs
                        .iter()
                        .position(|stored| stored == &path)
                        .unwrap_or_else(|| {
                            self.server_failure_logs.push(path);
                            self.server_failure_logs.len() - 1
                        });
                    crate::message::Message::OpenServerLogAt(index)
                });
                self.notices.trouble(notice, action);
                self.request_redraw();
            }
        }
    }

    /// What the servers behind `file` say they are working on, as one line
    /// for the status bar: the oldest piece of work still running.
    pub(super) fn server_activity(&self, file: FileId) -> Option<String> {
        let document = self.editor.get(file)?;
        let servers = document.borrow().servers();
        servers.iter().find_map(|client| {
            let work = client.progress().into_iter().next()?;
            let message = work
                .message
                .map(|message| format!(" {message}"))
                .unwrap_or_default();
            let percentage = work
                .percentage
                .map(|percentage| format!(" {percentage}%"))
                .unwrap_or_default();
            Some(format!(
                "{}: {}{message}{percentage}",
                client.name(),
                work.title
            ))
        })
    }

    /// The next delayed annotation request in a visible document.
    pub(super) fn next_annotation(&self) -> Option<Instant> {
        let scope = self.scope()?;
        self.panes
            .panes()
            .into_iter()
            .filter_map(|pane| self.file_in(self.panes.pane(pane)?.active(scope)?))
            .filter_map(|file| self.editor.next_annotation(file))
            .min()
    }
    /// Carries out a command the language server behind the file answers.
    pub(super) fn act_on_language(&mut self, action: Action) {
        if action == Action::Format && self.format_locally(false) {
            return;
        }
        let request = match action {
            Action::GoToDefinition => Request::Definition,
            Action::GoToTypeDefinition => Request::TypeDefinition,
            Action::GoToImplementation => Request::Implementation,
            Action::GoToDeclaration => Request::Declaration,
            Action::FindReferences => Request::References,
            Action::ShowHover => Request::Hover,
            Action::ShowCompletions => Request::Completions(Trigger::Invoked),
            Action::ShowSignature => Request::Signature,
            Action::ShowCodeActions => Request::CodeActions,
            Action::Format => self.format_request(),
            Action::ShowSymbols => Request::Symbols,
            Action::ShowIncomingCalls => Request::PrepareCalls(Calls::Incoming),
            Action::ShowOutgoingCalls => Request::PrepareCalls(Calls::Outgoing),
            Action::ShowWorkspaceSymbols => return self.open_picker(Kind::WorkspaceSymbols),
            Action::Rename => return self.prepare_rename(),
            _ => return,
        };
        if self.say_unserved() {
            return;
        }
        self.ask(request);
    }

    /// The formatting question the focused file is asked: its selected lines
    /// when something is selected and a server formats a range, else all of it.
    fn format_request(&self) -> Request {
        let selected = self
            .active_file()
            .is_some_and(|document| !document.borrow().buffer().selection().is_empty());
        let ranged = selected
            && self.active_file_id().is_some_and(|file| {
                let (Some(document), Some(path)) = (self.editor.get(file), self.editor.path(file))
                else {
                    return false;
                };
                let servers = document.borrow().servers();
                servers
                    .iter()
                    .any(|client| client.offers(&Request::FormatSelection, &path))
            });
        match ranged {
            true => Request::FormatSelection,
            false => Request::Format,
        }
    }

    /// Asks the first server behind the focused file that formats as the
    /// reader types what it would change now that `typed` was typed.
    pub(super) fn format_on_type(&mut self, typed: char) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let (Some(document), Some(path)) = (self.editor.get(file), self.editor.path(file)) else {
            return;
        };
        let (servers, at) = {
            let document = document.borrow();
            (document.servers(), document.buffer().selection().head)
        };
        let request = Request::FormatOnType(typed);
        let Some(client) = servers.into_iter().find(|client| {
            client.offers(&request, &path) && client.on_type_triggers(&path).contains(&typed)
        }) else {
            return;
        };
        self.ask_of(client, file, at, request, Purpose::Act);
    }

    /// Opens the prompt for a new name, having asked the server first
    /// whether the symbol under the cursor can be renamed and what it is
    /// called, when the server says.
    fn prepare_rename(&mut self) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let (Some(document), Some(path)) = (self.editor.get(file), self.editor.path(file)) else {
            return;
        };
        let (servers, at) = {
            let document = document.borrow();
            (document.servers(), document.buffer().selection().head)
        };
        let request = Request::PrepareRename;
        match servers
            .into_iter()
            .find(|client| client.offers(&request, &path))
        {
            Some(client) => {
                self.forget(file, &request, Purpose::Act);
                self.ask_of(client, file, at, request, Purpose::Act);
            }
            None => self.open_prompt(Action::Rename),
        }
    }

    /// Grows the selection to the next span out the servers said it can
    /// grow through, asking them when they have not said for this text, and
    /// by the syntax tree when no server answers it.
    pub(super) fn expand_selection(&mut self) {
        if self.grow_selection() {
            return;
        }
        let Some(file) = self.active_file_id() else {
            return;
        };
        let (Some(document), Some(path)) = (self.editor.get(file), self.editor.path(file)) else {
            return;
        };
        let (servers, at) = {
            let document = document.borrow();
            (document.servers(), document.buffer().selection().head)
        };
        let request = Request::SelectionRanges;
        match servers
            .into_iter()
            .find(|client| client.offers(&request, &path))
        {
            Some(client) => self.ask_of(client, file, at, request, Purpose::Act),
            None => self.edit_active(pm_text::Buffer::expand_selection),
        }
    }

    /// Grows the selection to the first span the servers said it can grow
    /// through that holds more than it does, answering whether there was one.
    fn grow_selection(&mut self) -> bool {
        let Some(file) = self.active_file_id() else {
            return false;
        };
        let Some(document) = self.editor.get(file) else {
            return false;
        };
        let Some((known, version, spans)) = self.selection_ranges.as_ref() else {
            return false;
        };
        if *known != file || *version != document.borrow().buffer().version() {
            return false;
        }
        let selection = document.borrow().buffer().selection();
        let (start, end) = match selection.anchor <= selection.head {
            true => (selection.anchor, selection.head),
            false => (selection.head, selection.anchor),
        };
        let Some(span) = spans
            .iter()
            .find(|span| {
                span.start <= start && span.end >= end && (span.start, span.end) != (start, end)
            })
            .cloned()
        else {
            return false;
        };
        self.edit_active(|buffer| {
            buffer.set_selection(pm_text::Selection {
                anchor: span.start,
                head: span.end,
            });
        });
        true
    }

    /// Writes every message to and from the language servers to their logs,
    /// or stops, and says which.
    pub(super) fn toggle_server_trace(&mut self) {
        let on = !pm_text::is_tracing();
        pm_text::set_trace(on);
        self.notices.done(
            match on {
                true => "Tracing language servers into their logs",
                false => "Stopped tracing language servers",
            },
            Some(crate::message::Message::OpenServerLog),
        );
    }

    /// Says `said` in the panel at the cursor.
    fn say_at_cursor(&mut self, said: &str) {
        let mut hint = Shown::at(self.cursor_point());
        hint.said = Some(said.to_owned());
        self.hint = Some(hint);
    }

    /// Says that the file has no server, when it has none.
    ///
    /// A command that quietly does nothing is a command the reader retries.
    /// Nothing is running behind a file whose server is not installed, and
    /// saying so is the whole of what the editor can do about it.
    fn say_unserved(&mut self) -> bool {
        let Some(document) = self.active_file() else {
            return false;
        };
        if document.borrow().is_served() {
            return false;
        }
        let (language, advice) = document.borrow().buffer().language().map_or_else(
            || (String::from("this file"), None),
            |language| {
                let advice = match self.editor.installable_server(language) {
                    Some(_) => Some("Install it from the palette."),
                    None => self.editor.server_needs(language),
                };
                (language.name().to_owned(), advice)
            },
        );

        let mut hint = Shown::at(self.cursor_point());
        hint.said = Some(match advice {
            Some(advice) => format!("No language server is running for {language}. {advice}"),
            None => format!("No language server is running for {language}."),
        });
        self.hint = Some(hint);
        self.request_redraw();
        true
    }

    /// Asks the server behind the focused file `request`, about the cursor.
    pub(super) fn ask(&mut self, request: Request) {
        if matches!(request, Request::Hover | Request::Signature) {
            let mut hint = Shown::at(self.cursor_point());
            hint.language = self
                .active_file()
                .and_then(|document| document.borrow().buffer().language());
            self.hint = Some(hint);
        }
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(at) = self
            .editor
            .get(file)
            .map(|document| document.borrow().buffer().selection().head)
        else {
            return;
        };
        self.ask_about(file, at, request);
    }

    /// Asks every server behind `file` `request`, about `at` in it.
    ///
    /// A question of the same kind that is still unanswered is given up on
    /// first: what the reader is asking about now is where the cursor is
    /// now, and an answer about where it was a keystroke ago is an answer to
    /// nothing.
    ///
    /// All of them are asked rather than one of them, because which has the
    /// answer is not knowable in advance: a linter has the fix and a type
    /// checker has the type, and the one with nothing to say says nothing.
    pub(super) fn ask_about(&mut self, file: FileId, at: Position, request: Request) {
        self.ask_about_for(file, at, request, Purpose::Act);
    }

    /// Asks the first server for edits, or every server for other answers.
    pub(super) fn ask_about_for(
        &mut self,
        file: FileId,
        at: Position,
        request: Request,
        purpose: Purpose,
    ) {
        self.forget(file, &request, purpose);
        if request == Request::CodeActions {
            self.code_actions.clear();
        }
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let clients = document.borrow().servers();
        let path = document.borrow().buffer().path().to_path_buf();
        for client in clients {
            let offered = client.offers(&request, &path);
            self.ask_of(client, file, at, request.clone(), purpose);
            if offered
                && matches!(
                    request,
                    Request::Format
                        | Request::FormatSelection
                        | Request::WillSave
                        | Request::SourceActions(_)
                )
            {
                break;
            }
        }
    }

    /// Asks `client` alone `request`, about `at` in `file`, if it answers it.
    ///
    /// A question that carries what one server handed out goes back to that
    /// server and no other: a symbol rust-analyzer named means nothing to a
    /// linter running beside it.
    pub(super) fn ask_of(
        &mut self,
        client: Arc<Client>,
        file: FileId,
        at: Position,
        request: Request,
        purpose: Purpose,
    ) {
        let Some(path) = self.editor.path(file) else {
            return;
        };
        if !client.offers(&request, &path) {
            return;
        }
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let (version, indent, selection) = {
            let document = document.borrow();
            let buffer = document.buffer();
            let selected = buffer.selection();
            let whole = Position::default()
                ..Position::new(
                    buffer.line_count().saturating_sub(1),
                    buffer.line_len(buffer.line_count().saturating_sub(1)),
                );
            let selection = if matches!(request, Request::SourceActions(_)) {
                whole
            } else if selected.anchor == selected.head {
                Position::new(at.line, 0)..Position::new(at.line, buffer.line_len(at.line))
            } else if selected.anchor < selected.head {
                selected.anchor..selected.head
            } else {
                selected.head..selected.anchor
            };
            (buffer.version(), buffer.indent(), selection)
        };
        let asked = client.ask(request.clone(), &path, at, indent, selection);
        self.asked.push(Pending {
            client,
            asked,
            file,
            at,
            request,
            purpose,
            version,
        });
    }

    /// Whether a question of `request`'s kind about `file` is unanswered.
    fn awaits(&self, file: FileId, request: &Request) -> bool {
        let kind = std::mem::discriminant(request);
        self.asked
            .iter()
            .any(|pending| pending.file == file && std::mem::discriminant(&pending.request) == kind)
    }

    /// Starts the symbol picker that was just opened over the worktree's
    /// files asking the servers for what its query names.
    ///
    /// The symbols are the servers' to fill, not the picker's: a workspace
    /// has too many to gather up front, so each query is asked anew and the
    /// list is filled as the answers come in. The files it was opened over
    /// stay below them, so a name finds something even where no server runs.
    pub(super) fn ask_typed_symbols(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let query = Kind::WorkspaceSymbols
            .query(picker.field().value())
            .to_owned();
        self.workspace_files = picker.rows().cloned().collect();
        self.workspace_symbols = (None, Vec::new());
        self.ask_workspace_symbols(query);
    }

    /// Asks every server over the focused file's worktree for the symbols
    /// `query` names.
    ///
    /// Every server of the worktree is asked rather than the file's own: a
    /// symbol of the workspace may be declared in any of its languages. A
    /// query already asked is not asked again, which is what moving the caret
    /// along the field would otherwise do.
    pub(super) fn ask_workspace_symbols(&mut self, query: String) {
        if self.workspace_symbols.0.as_ref() == Some(&query) {
            return;
        }
        let Some(file) = self.active_file_id() else {
            return;
        };
        self.workspace_symbols = (Some(query.clone()), Vec::new());
        let Some(root) = self.worktree_of(file) else {
            return;
        };
        let request = Request::WorkspaceSymbols(query);
        self.forget(file, &request, Purpose::Act);
        for client in self.editor.servers_over(&root) {
            self.ask_of(
                client,
                file,
                Position::default(),
                request.clone(),
                Purpose::Act,
            );
        }
    }

    /// Asks the servers what they know about the files on screen: what to
    /// write into their lines, and what their names are.
    ///
    /// The whole of each file is asked about rather than the part on screen:
    /// scrolling is not a question, and a file short enough to be open is
    /// short enough to be answered about in one go.
    pub(super) fn refresh_annotations(&mut self) {
        self.follow_focused_file();
        self.collect_annotation_refreshes();
        self.cancel_stale_annotations();
        let Some(scope) = self.scope() else {
            return;
        };
        self.refresh_outline(scope);
        let showing = self
            .panes
            .panes()
            .into_iter()
            .filter_map(|pane| self.file_in(self.panes.pane(pane)?.active(scope)?))
            .collect::<Vec<_>>();

        if let Some(file) = self.active_file_id() {
            self.refresh_uses(file);
        }
        for file in showing {
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            let clients = document.borrow().servers();
            let path = document.borrow().buffer().path().to_path_buf();
            for client in clients {
                if client.offers(&Request::Semantics, &path)
                    && document.borrow_mut().wants_semantics(&client)
                {
                    self.ask_of(
                        client.clone(),
                        file,
                        Position::default(),
                        Request::Semantics,
                        Purpose::Act,
                    );
                }
                if self.preferences.code_lens
                    && client.offers(&Request::Lenses, &path)
                    && document.borrow_mut().wants_lenses(&client)
                {
                    self.forget_lens_resolves(file, &client);
                    self.ask_of(
                        client.clone(),
                        file,
                        Position::default(),
                        Request::Lenses,
                        Purpose::Act,
                    );
                }
                if client.offers(&Request::Folds, &path)
                    && document.borrow_mut().wants_folds(&client)
                {
                    self.ask_of(
                        client.clone(),
                        file,
                        Position::default(),
                        Request::Folds,
                        Purpose::Act,
                    );
                }
                let last = document.borrow().buffer().line_count().saturating_sub(1);
                let span = Position::default()..Position::new(last, 0);
                let request = Request::Hints(span);
                if self.preferences.inlay_hints
                    && client.offers(&request, &path)
                    && document.borrow_mut().wants_hints(&client)
                {
                    self.ask_of(client, file, Position::default(), request, Purpose::Act);
                }
            }
        }
    }

    /// Refreshes the open outline from syntax, then asks the first offering server.
    fn refresh_outline(&mut self, scope: pm_core::Scope) {
        if !self
            .panes
            .held()
            .contains(&crate::panes::Item::Outline(scope))
        {
            return;
        }
        let Some(file) = self.outlines.followed(scope) else {
            return;
        };
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let outline_pane = self.panes.panes().into_iter().find(|pane| {
            self.panes.pane(*pane).is_some_and(|pane| {
                pane.active(Some(scope)) == Some(crate::panes::Item::Outline(scope))
            })
        });
        let height = self
            .geometry
            .pane_size(outline_pane.unwrap_or_else(|| self.panes.focus()))
            .map_or(400.0, |size| size.height);
        let visible_rows = ((height - 70.0) / crate::outline::ROW_HEIGHT).max(1.0) as usize;
        let (version, symbols, cursor, clients, path) = {
            let mut document = document.borrow_mut();
            let buffer = document.buffer();
            let cursor = buffer.selection().head;
            if !document.wants_outline() {
                self.outlines
                    .get_mut(file)
                    .follow_cursor(cursor, visible_rows);
                return;
            }
            (
                document.buffer().version(),
                document.buffer().declarations(),
                cursor,
                document.servers(),
                document.buffer().path().to_path_buf(),
            )
        };
        self.outlines
            .get_mut(file)
            .replace(version, symbols, "syntax".to_owned());
        self.outlines
            .get_mut(file)
            .follow_cursor(cursor, visible_rows);
        if let Some(client) = clients
            .into_iter()
            .find(|client| client.offers(&Request::Symbols, &path))
        {
            self.ask_of(
                client,
                file,
                Position::default(),
                Request::Symbols,
                Purpose::Outline,
            );
        }
    }

    /// Applies server refresh requests to every document they serve.
    fn collect_annotation_refreshes(&mut self) {
        for client in self.editor.clients() {
            for request in client.take_refreshes() {
                self.editor.refresh_annotation(&client, &request);
            }
        }
    }

    /// Cancels annotation work for text that has since changed.
    fn cancel_stale_annotations(&mut self) {
        self.asked.retain(|pending| {
            let annotation = matches!(
                pending.request,
                Request::Hints(_)
                    | Request::Semantics
                    | Request::Lenses
                    | Request::ResolveLens(_)
                    | Request::Folds
            );
            let stale = (annotation || pending.purpose == Purpose::Outline)
                && self
                    .editor
                    .get(pending.file)
                    .is_none_or(|document| document.borrow().buffer().version() != pending.version);
            if stale {
                pending.client.forget(pending.asked);
            }
            !stale
        });
    }

    /// Cancels unresolved lenses superseded by a fresh list from one server.
    fn forget_lens_resolves(&mut self, file: FileId, client: &Arc<Client>) {
        self.asked.retain(|pending| {
            let obsolete = pending.file == file
                && Arc::ptr_eq(&pending.client, client)
                && matches!(pending.request, Request::ResolveLens(_));
            if obsolete {
                pending.client.forget(pending.asked);
            }
            !obsolete
        });
    }

    /// Asks where the symbol at the cursor of `file` is used, once it has
    /// moved or the text has changed.
    fn refresh_uses(&mut self, file: FileId) {
        if !self.preferences.display.occurrences {
            return;
        }
        let Some(document) = self.editor.get(file) else {
            return;
        };
        if !document.borrow_mut().wants_uses() {
            return;
        }
        let at = document.borrow().buffer().selection().head;
        self.ask_about(file, at, Request::Occurrences);
    }

    /// Says what the editor knows about the place the pointer has stopped on.
    ///
    /// A fault the editor already knows about goes up at once, because it
    /// has nothing to ask anybody, and the server is asked about the same
    /// place all the same: what a squiggle is on still has a type, and the
    /// two are read together.
    ///
    /// A word already being talked about is not asked about again, which is
    /// what lets the panel stay up while the pointer crosses the name it is
    /// about, and nothing is asked while the pointer rests on the panel: the
    /// text under it is not what the reader is looking at.
    ///
    /// The panel hangs from the bottom of the line the word is on, so the
    /// pointer can go straight down onto it without crossing anything else.
    pub(super) fn hover_at(&mut self, point: pm_gfx::Point) {
        if self.hint.as_ref().is_some_and(|hint| hint.covers(point)) {
            return;
        }
        let Some((file, at)) = self.place_under(point) else {
            return self.forget_hint(point);
        };
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let about = (file, document.borrow().buffer().word_at(at));
        if self.hint.as_ref().and_then(|hint| hint.about.clone()) == Some(about.clone()) {
            return;
        }

        let fault = document
            .borrow()
            .buffer()
            .diagnostic_at(at)
            .map(|found| found.message.clone());

        let (line_top, under) = {
            let document = document.borrow();
            (
                document.point_of(about.1.start).y,
                document.layout().cell.height,
            )
        };
        self.hint = Some(Shown {
            at: pm_gfx::Point::new(point.x, line_top + under),
            fault,
            about: Some(about),
            language: document.borrow().buffer().language(),
            ..Shown::default()
        });
        self.ask_about(file, at, Request::Hover);
        self.request_redraw();
    }

    /// The character in a file the pointer is over, when it is over one.
    ///
    /// The place is the character under the pointer rather than the gap a
    /// caret would fall into: what is being asked about is a name, not a
    /// place between two of them.
    pub(super) fn place_under(&self, point: pm_gfx::Point) -> Option<(FileId, Position)> {
        let (file, document) = self.document_at(point)?;
        let at = document.borrow().position_under(point)?;
        Some((file, at))
    }

    /// The word under `point`, and the file it is in.
    pub(super) fn word_under(
        &self,
        point: pm_gfx::Point,
    ) -> Option<(FileId, std::ops::Range<Position>)> {
        let (file, at) = self.place_under(point)?;
        let document = self.editor.get(file)?;
        let word = document.borrow().buffer().word_at(at);
        (word.start != word.end).then_some((file, word))
    }

    /// The name the panel is answering about, while it is up.
    ///
    /// It is lit on the text as well as answered for beside it, so a panel
    /// that opened over a crowded line still says which name it is about.
    pub(super) fn hovered_name(&self) -> Option<(FileId, std::ops::Range<Position>)> {
        let hint = self.hint.as_ref()?;
        (!hint.is_empty()).then_some(hint.about.clone()?)
    }

    /// The name to draw as a link, while the key that makes one is held.
    ///
    /// Nothing is underlined until a server has said the name leads
    /// somewhere: a link that goes nowhere is a promise the editor cannot
    /// keep.
    pub(super) fn link_target(&self) -> Option<(FileId, std::ops::Range<Position>)> {
        if !self.modifiers.control_key() {
            return None;
        }
        let link = self.link.as_ref()?;
        Some((link.file, link.span.clone()?))
    }

    /// Asks whether the name under the pointer leads anywhere, once.
    ///
    /// The question is asked again only when the pointer reaches another
    /// name, so sliding along one costs nothing after the first answer.
    pub(super) fn follow_pointer(&mut self, point: pm_gfx::Point) {
        if self.settings_open || !self.modifiers.control_key() {
            return self.drop_link();
        }
        let Some((file, word)) = self.word_under(point) else {
            return self.drop_link();
        };
        if self
            .link
            .as_ref()
            .is_some_and(|link| link.file == file && link.about == word)
        {
            return;
        }
        if !self
            .editor
            .get(file)
            .is_some_and(|document| document.borrow().is_served())
        {
            return self.drop_link();
        }

        let at = word.start;
        self.link = Some(Link {
            file,
            about: word,
            span: None,
        });
        self.ask_about_for(file, at, Request::Definition, Purpose::Link);
    }

    /// Forgets the link the pointer was over, redrawing if one was drawn.
    pub(super) fn drop_link(&mut self) {
        if self.link.take().is_some() {
            self.request_redraw();
        }
    }

    /// Takes down what a server said about the name under the pointer.
    fn link_found(&mut self, pending: &Pending, found: &[Location]) {
        let Some(link) = self.link.as_mut() else {
            return;
        };
        if link.file != pending.file || !link.about.contains(&pending.at) {
            return;
        }
        link.span = Some(
            found
                .iter()
                .find_map(|location| location.origin.clone())
                .unwrap_or_else(|| link.about.clone()),
        );
        self.request_redraw();
    }

    /// Drops what was being said about a place the pointer has left.
    ///
    /// A panel about the word the pointer is still on stays up: reading a
    /// hover means moving across the name it is about, and one that went
    /// away at the first pixel of that would never be read at all. So does
    /// a panel the pointer has moved onto, so that it can be scrolled.
    pub(super) fn forget_hint(&mut self, point: pm_gfx::Point) {
        if self.ui.as_ref().is_some_and(pm_ui::Ui::selecting_text) {
            return;
        }
        let Some(hint) = self.hint.as_ref() else {
            return;
        };
        if hint.covers(point) {
            return;
        }
        let Some(about) = hint.about.clone() else {
            return;
        };
        let over = self
            .place_under(point)
            .filter(|(file, at)| *file == about.0 && about.1.contains(at));
        if over.is_none() {
            self.hint = None;
            self.request_redraw();
        }
    }

    /// The file drawn under `point`, whichever pane is showing it.
    pub(super) fn document_at(
        &self,
        point: pm_gfx::Point,
    ) -> Option<(FileId, crate::editor::OpenFile)> {
        let scope = self.scope()?;
        self.panes.panes().into_iter().find_map(|pane| {
            let file = self.panes.pane(pane)?.active(scope)?.file()?;
            let document = self.editor.get(file)?;
            let over = document.borrow().layout().text_area().contains(point);
            over.then_some((file, document))
        })
    }

    /// Gives up on every unanswered question of this kind and reason.
    ///
    /// The reason is part of it: the editor asking whether a name leads
    /// anywhere must not call off the reader asking to be taken there.
    fn forget(&mut self, file: FileId, request: &Request, purpose: Purpose) {
        let kind = std::mem::discriminant(request);
        self.asked.retain(|pending| {
            if pending.file != file
                || pending.purpose != purpose
                || (std::mem::discriminant(&pending.request) != kind
                    && !(matches!(request, Request::Lenses)
                        && matches!(pending.request, Request::ResolveLens(_))))
            {
                return true;
            }
            pending.client.forget(pending.asked);
            false
        });
    }

    /// Collects every answer that has come back, saying whether any had.
    pub(super) fn collect_answers(&mut self) -> bool {
        let mut applied = false;
        for client in self.editor.clients() {
            for request in client.take_workspace_edits() {
                let supported = request.supported;
                let edits = request.edits.clone();
                let success = supported && self.apply_changes(edits);
                client.answer_workspace_edit(request, success);
                applied = true;
            }
        }
        let ready = self
            .asked
            .iter()
            .enumerate()
            .filter_map(|(index, pending)| {
                let answer = pending.client.answer(pending.asked)?;
                Some((index, answer))
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return applied;
        }

        for (index, answer) in ready.iter().rev() {
            let pending = self.asked.remove(*index);
            self.answered(&pending, answer.clone());
        }
        true
    }

    /// Acts on one answer, if the file it was about is still open.
    ///
    /// Empty annotation replies clear only the responding server's results;
    /// empty replies to interactive questions have nothing to display.
    fn answered(&mut self, pending: &Pending, answer: Answer) {
        if self.editor.get(pending.file).is_none() {
            return;
        }
        let stale = self
            .editor
            .get(pending.file)
            .is_some_and(|document| document.borrow().buffer().version() != pending.version);
        if pending.purpose == Purpose::Outline {
            if !stale {
                self.outline_answered(pending, answer);
            }
            return;
        }
        if stale
            && (matches!(answer, Answer::Edits(_) | Answer::Changes(_))
                || matches!(pending.request, Request::CodeActions | Request::Signature))
        {
            if self.saving && Self::is_save_step(&pending.request) {
                self.finish_save();
            }
            return;
        }
        if matches!(answer, Answer::Refused) {
            if let Some(document) = self.editor.get(pending.file) {
                let mut document = document.borrow_mut();
                match pending.request {
                    Request::Hints(_) => {
                        document.answered_hints(&pending.client, pending.version, None)
                    }
                    Request::Semantics => {
                        document.answered_semantics(&pending.client, pending.version, None);
                    }
                    Request::Lenses => {
                        document.answered_lenses(&pending.client, pending.version, None);
                    }
                    Request::Folds => {
                        document.answered_folds(&pending.client, pending.version, None);
                    }
                    _ => {}
                }
            }
            match pending.request {
                Request::PrepareRename => self.say_at_cursor("This can't be renamed."),
                Request::SelectionRanges => self.edit_active(pm_text::Buffer::expand_selection),
                _ => {}
            }
            return self.save_once_formatted(pending);
        }
        if answer.is_empty()
            && !matches!(
                pending.request,
                Request::Hints(_) | Request::Semantics | Request::Lenses | Request::Folds
            )
        {
            return self.save_once_formatted(pending);
        }
        match answer {
            Answer::Inline(_) => {}
            Answer::Locations(found) if pending.purpose == Purpose::Link => {
                self.link_found(pending, &found);
            }
            Answer::Locations(found) if pending.request == Request::References => {
                self.show_references(found);
            }
            Answer::Locations(found) => self.follow_definition(pending, found),
            Answer::Hover(text) => {
                if let Some(hint) = self.hint.as_mut() {
                    hint.said = Some(text);
                }
            }
            Answer::Signature(signature) => {
                let current = self.active_file_id() == Some(pending.file)
                    && self.editor.get(pending.file).is_some_and(|document| {
                        document.borrow().buffer().selection().head == pending.at
                    });
                if let Some(hint) = self.hint.as_mut().filter(|_| current) {
                    hint.signature = Some(signature);
                }
            }
            Answer::Renamable { span, placeholder } => {
                let seeded = placeholder.or_else(|| {
                    let document = self.editor.get(pending.file)?;
                    let document = document.borrow();
                    let buffer = document.buffer();
                    Some(buffer.text_in(span.unwrap_or_else(|| buffer.word_at(pending.at))))
                });
                self.open_picker_with(Kind::Rename, Vec::new(), seeded.unwrap_or_default());
            }
            Answer::Folds(folds) => {
                if let Some(document) = self.editor.get(pending.file) {
                    document.borrow_mut().answered_folds(
                        &pending.client,
                        pending.version,
                        Some(folds),
                    );
                }
            }
            Answer::Selections(spans) => {
                self.selection_ranges = Some((pending.file, pending.version, spans));
                self.grow_selection();
            }
            Answer::Completions { items, incomplete } => {
                self.show_completions(pending, items, incomplete);
            }
            Answer::Resolved(item) => self.take_resolved(pending, *item),
            Answer::CodeActions(actions) => self.show_code_actions(pending, actions),
            Answer::Edits(files) => {
                self.apply_edits(files);
                self.save_once_formatted(pending);
            }
            Answer::Changes(changes) => {
                self.apply_changes(changes);
            }
            Answer::Hints(hints) => {
                if let Some(document) = self.editor.get(pending.file) {
                    document.borrow_mut().answered_hints(
                        &pending.client,
                        pending.version,
                        Some(hints),
                    );
                }
            }
            Answer::Semantics(spans) => {
                if let Some(document) = self.editor.get(pending.file) {
                    document.borrow_mut().answered_semantics(
                        &pending.client,
                        pending.version,
                        Some(spans),
                    );
                }
                self.repaint_review(pending.file);
            }
            Answer::Occurrences(spans) => {
                if let Some(document) = self.editor.get(pending.file) {
                    document.borrow_mut().buffer_mut().set_uses(spans);
                }
            }
            Answer::Lenses(lenses) => self.take_lenses(pending, lenses),
            Answer::CallItems(items) => self.follow_calls(pending, items),
            Answer::Named(found) => match &pending.request {
                Request::WorkspaceSymbols(query) => self.show_workspace_symbols(query, found),
                _ => {
                    let rows = self.named_rows(found);
                    self.open_picker_with(Kind::Calls, rows, String::new());
                }
            },
            Answer::Refused => {}
            Answer::Symbols(symbols) => {
                let rows = symbols
                    .into_iter()
                    .filter_map(|symbol| {
                        let scope = self.editor.scope_of(pending.file)?;
                        let path = self.editor.path(pending.file)?;
                        Some(Row {
                            section: None,
                            label: format!("{}{}", "  ".repeat(symbol.depth), symbol.name),
                            detail: if symbol.detail.is_empty() {
                                symbol.kind.to_owned()
                            } else {
                                symbol.detail
                            },
                            choice: Choice::OpenAt(scope, path, symbol.position),
                            enabled: true,
                        })
                    })
                    .collect();
                self.open_picker_with(Kind::Symbols, rows, String::new());
            }
        }
        self.request_redraw();
    }

    /// Takes the first nonempty server outline or asks the next offering server.
    fn outline_answered(&mut self, pending: &Pending, answer: Answer) {
        if let Answer::Symbols(symbols) = answer
            && !symbols.is_empty()
        {
            self.outlines.get_mut(pending.file).replace(
                pending.version,
                symbols,
                pending.client.name().to_owned(),
            );
            self.request_redraw();
            return;
        }
        let Some(document) = self.editor.get(pending.file) else {
            return;
        };
        let clients = document.borrow().servers();
        let path = document.borrow().buffer().path().to_path_buf();
        let next = clients
            .iter()
            .position(|client| Arc::ptr_eq(client, &pending.client))
            .and_then(|index| {
                clients
                    .into_iter()
                    .skip(index + 1)
                    .find(|client| client.offers(&Request::Symbols, &path))
            });
        if let Some(client) = next {
            self.ask_of(
                client,
                pending.file,
                Position::default(),
                Request::Symbols,
                Purpose::Outline,
            );
        }
    }

    /// Takes the next step after one formatting or pre-save reply.
    ///
    /// A save the servers take part in is three steps: the formatter's
    /// changes, then whatever each server wants changed before the file is
    /// written, then the writing. Each server is asked after the previous
    /// answer has been applied to the document.
    fn save_once_formatted(&mut self, pending: &Pending) {
        if !Self::is_save_step(&pending.request) {
            return;
        }
        if self.ask_next_edit_server(pending) {
            return;
        }
        if !self.saving {
            return;
        }
        match pending.request {
            Request::WillSave => self.finish_save(),
            _ => self.next_save_step(pending.file),
        }
    }

    /// Whether `request` is one of the questions a save asks the servers.
    fn is_save_step(request: &Request) -> bool {
        matches!(
            request,
            Request::Format | Request::WillSave | Request::SourceActions(_)
        )
    }

    /// Asks the next capable server about the document left by the last one.
    fn ask_next_edit_server(&mut self, pending: &Pending) -> bool {
        let Some(document) = self.editor.get(pending.file) else {
            return false;
        };
        let (clients, at, path) = {
            let document = document.borrow();
            (
                document.servers(),
                document.buffer().selection().head,
                document.buffer().path().to_path_buf(),
            )
        };
        let next = clients
            .into_iter()
            .skip_while(|client| !Arc::ptr_eq(client, &pending.client))
            .skip(1)
            .find(|client| client.offers(&pending.request, &path));
        if let Some(client) = next {
            self.ask_of(
                client,
                pending.file,
                at,
                pending.request.clone(),
                pending.purpose,
            );
            return true;
        }
        false
    }

    /// Starts a save the servers behind the focused file take part in.
    ///
    /// A save with no server to wait on is written at once. Otherwise the
    /// servers are asked, in order, to organize the imports and make their
    /// own fixes when those are wanted, then to lay the file out when that
    /// is, and last what they would change before it is written.
    pub(super) fn begin_save(&mut self, format: bool) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        self.saving = true;
        self.formatting = format;
        let settings = self.active_language_settings();
        let format =
            format && settings.formatter == crate::config::languages::Formatter::LanguageServer;
        self.save_steps = [
            (
                settings.organize_imports_on_save,
                Request::SourceActions("source.organizeImports".to_owned()),
            ),
            (
                settings.fix_on_save,
                Request::SourceActions("source.fixAll".to_owned()),
            ),
            (format, Request::Format),
        ]
        .into_iter()
        .filter_map(|(wanted, request)| wanted.then_some(request))
        .collect();
        self.next_save_step(file);
    }

    /// Asks the next question a save has for the servers, skipping the ones
    /// no server answers, and goes on to what they would change before the
    /// file is written when there are none left.
    fn next_save_step(&mut self, file: FileId) {
        let at = self
            .editor
            .get(file)
            .map(|document| document.borrow().buffer().selection().head)
            .unwrap_or_default();
        while let Some(request) = self.save_steps.pop_front() {
            self.ask_about(file, at, request.clone());
            if self.awaits(file, &request) {
                return;
            }
        }
        self.ask_before_save(file);
    }

    /// Asks each server what it would change in `file` before it is saved.
    fn ask_before_save(&mut self, file: FileId) {
        let at = self
            .editor
            .get(file)
            .map(|document| document.borrow().buffer().selection().head)
            .unwrap_or_default();
        self.ask_about(file, at, Request::WillSave);
        if !self.awaits(file, &Request::WillSave) {
            self.finish_save();
        }
    }

    /// Writes the file a save was waiting on the servers for.
    fn finish_save(&mut self) {
        self.saving = false;
        if self.formatting {
            self.format_locally(true);
        }
        self.save_active();
    }

    /// Takes down the notes a server put above the file's declarations.
    ///
    /// A note the server sent without saying what it says is resolved by
    /// the same server, one question each: which of them need it is the
    /// server's to decide, and a count of uses is only worked out on asking.
    fn take_lenses(&mut self, pending: &Pending, lenses: Vec<Lens>) {
        let Some(document) = self.editor.get(pending.file) else {
            return;
        };
        if matches!(pending.request, Request::ResolveLens(_)) {
            for lens in lenses {
                document.borrow_mut().resolve_lens(&pending.client, lens);
            }
            return;
        }
        if !document.borrow_mut().answered_lenses(
            &pending.client,
            pending.version,
            Some(lenses.clone()),
        ) {
            return;
        }
        for lens in lenses.into_iter().filter(|lens| lens.title.is_none()) {
            self.ask_of(
                pending.client.clone(),
                pending.file,
                lens.position,
                Request::ResolveLens(lens.handle),
                pending.purpose,
            );
        }
    }

    /// Asks the server that named the symbol under the cursor for its calls.
    fn follow_calls(&mut self, pending: &Pending, items: Vec<pm_text::Handle>) {
        let Request::PrepareCalls(direction) = pending.request else {
            return;
        };
        let Some(item) = items.into_iter().next() else {
            return;
        };
        let request = Request::Calls(direction, item);
        self.forget(pending.file, &request, pending.purpose);
        self.ask_of(
            pending.client.clone(),
            pending.file,
            pending.at,
            request,
            pending.purpose,
        );
    }

    /// Fills the workspace symbol picker with what a server found for `query`.
    ///
    /// What each server finds is added to what the others found for the
    /// same query, and a query the reader has typed past is dropped.
    fn show_workspace_symbols(&mut self, query: &str, found: Vec<NamedLocation>) {
        let open = self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.kind() == Kind::WorkspaceSymbols);
        if !open || self.workspace_symbols.0.as_deref() != Some(query) {
            return;
        }
        let rows = self.named_rows(found);
        self.workspace_symbols.1.extend(rows);
        let rows = self
            .workspace_symbols
            .1
            .iter()
            .chain(&self.workspace_files)
            .cloned()
            .collect();
        if let Some(picker) = self.picker.as_mut() {
            picker.refill(rows);
        }
    }

    /// The picker rows of named places: a symbol and where it is.
    fn named_rows(&self, found: Vec<NamedLocation>) -> Vec<Row> {
        found
            .into_iter()
            .filter_map(|named| {
                let location = named.location;
                let place = self.place_of(&location.path, location.range.start)?;
                let file = format!(
                    "{}:{}",
                    self.relative_to(place.scope, &location.path),
                    place.position.line + 1
                );
                let about = if named.detail.is_empty() {
                    named.kind
                } else {
                    named.detail.as_str()
                };
                Some(Row {
                    section: None,
                    label: named.name,
                    detail: match about.is_empty() {
                        true => file,
                        false => format!("{about} · {file}"),
                    },
                    choice: Choice::OpenAt(place.scope, place.path, place.position),
                    enabled: true,
                })
            })
            .collect()
    }

    /// Goes to the first place a server named, taking down where the cursor was.
    fn go_to_first(&mut self, found: &[Location]) {
        let Some(location) = found.first() else {
            return;
        };
        let Some(place) = self.place_of(&location.path, location.range.start) else {
            return;
        };
        self.jump_to(&place);
    }

    /// Goes where a definition, declaration or implementation lies.
    ///
    /// A name asked about at its own definition has nowhere to go, so what
    /// is wanted of it is where it is used, as in VS Code. Several places
    /// are listed to choose from rather than the first being taken.
    fn follow_definition(&mut self, pending: &Pending, found: Vec<Location>) {
        let path = self.editor.path(pending.file);
        let at_definition = found.iter().any(|location| {
            Some(&location.path) == path.as_ref()
                && location.range.start <= pending.at
                && pending.at <= location.range.end
        });
        if at_definition {
            self.ask_about(pending.file, pending.at, Request::References);
            return;
        }
        match found.len() {
            0 | 1 => self.go_to_first(&found),
            _ => self.show_references(found),
        }
    }

    /// Opens the picker over everywhere a symbol is used.
    fn show_references(&mut self, found: Vec<Location>) {
        let rows = found
            .into_iter()
            .filter_map(|location| {
                let place = self.place_of(&location.path, location.range.start)?;
                let name = location
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some(Row {
                    section: None,
                    label: format!("{name}:{}", place.position.line + 1),
                    detail: self.relative_to(place.scope, &location.path),
                    choice: Choice::OpenAt(place.scope, place.path, place.position),
                    enabled: true,
                })
            })
            .collect::<Vec<_>>();
        self.open_picker_with(Kind::References, rows, String::new());
    }

    /// Offers what could be written where the cursor is.
    ///
    /// What a server offers for the word the list is already up for joins
    /// the list, in place of what that server offered before.
    fn show_completions(
        &mut self,
        pending: &Pending,
        items: Vec<pm_text::Completion>,
        incomplete: bool,
    ) {
        if self.active_file_id() != Some(pending.file) {
            return;
        }
        if items.is_empty() {
            if self.completions.is_none() {
                self.offer_words();
            }
            return;
        }
        let Some(document) = self.editor.get(pending.file) else {
            return;
        };
        let document = document.borrow();
        let head = document.buffer().selection().head;
        let word = document.buffer().word_at(head);
        if pending.at.line != head.line || pending.at < word.start || pending.at > head {
            return;
        }
        let typed = document.buffer().text_in(word.start..head);
        let under = document.layout().cell.height;
        let at = document.point_of(word.start);
        let point = pm_gfx::Point::new(at.x, at.y + under);
        drop(document);

        if self
            .completions
            .as_ref()
            .is_none_or(|list| list.start() != word.start)
        {
            let mut list = Completions::new(word.start, point, self.recent_completions.clone());
            list.narrow(&typed);
            self.completions = Some(list);
        }
        if let Some(list) = self.completions.as_mut() {
            list.offer(&pending.client, items, incomplete);
        }
        if self.completions.as_ref().is_some_and(Completions::is_empty) {
            self.completions = None;
        }
        self.resolve_completion();
    }

    /// Whether any language server is behind the focused file.
    pub(super) fn active_file_has_servers(&self) -> bool {
        self.active_file()
            .is_some_and(|document| !document.borrow().servers().is_empty())
    }

    /// Offers the words of the focused file as what could be written where
    /// the cursor is, for a word of at least two characters begun there.
    ///
    /// A server that has nothing to say leaves the reader with what the file
    /// itself already says, the way an editor without a server would.
    pub(super) fn offer_words(&mut self) {
        let Some(document) = self.active_file() else {
            return;
        };
        let document = document.borrow();
        let buffer = document.buffer();
        let head = buffer.selection().head;
        let word = buffer.word_at(head);
        if head.line != word.start.line || head.column - word.start.column.min(head.column) < 2 {
            return;
        }
        let typed = buffer.text_in(word.start..head);
        let mut seen = std::collections::HashSet::new();
        let words = (0..buffer.line_count())
            .flat_map(|line| {
                buffer
                    .line_text(line)
                    .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                    .filter(|word| word.chars().count() > 1)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|word| *word != typed && seen.insert(word.clone()))
            .map(|word| pm_text::Completion::word(&word))
            .collect::<Vec<_>>();
        let under = document.layout().cell.height;
        let at = document.point_of(word.start);
        drop(document);
        if words.is_empty() {
            return;
        }
        let mut list = Completions::new(
            word.start,
            pm_gfx::Point::new(at.x, at.y + under),
            self.recent_completions.clone(),
        );
        list.narrow(&typed);
        list.offer_words(words);
        if !list.is_empty() {
            self.completions = Some(list);
        }
    }

    /// Whether a server behind the focused file completes after `typed`.
    pub(super) fn completes_after(&self, typed: char) -> bool {
        let Some(file) = self.active_file_id() else {
            return false;
        };
        let (Some(document), Some(path)) = (self.editor.get(file), self.editor.path(file)) else {
            return false;
        };
        let servers = document.borrow().servers();
        servers
            .iter()
            .any(|client| client.completion_triggers(&path).contains(&typed))
    }

    /// Refreshes a visible call signature or opens one after a server trigger.
    pub(super) fn signature_after_typing(&mut self, typed: Option<char>) {
        let showing = self
            .hint
            .as_ref()
            .is_some_and(|hint| hint.signature.is_some());
        let file = self.active_file_id();
        let awaiting = file.is_some_and(|file| self.awaits(file, &Request::Signature));
        let Some(typed) = typed else {
            if showing || awaiting {
                self.hint = None;
                if let Some(file) = file {
                    self.forget(file, &Request::Signature, Purpose::Act);
                }
            }
            return;
        };
        if (showing || awaiting) && typed == ')' {
            self.hint = None;
            if let Some(file) = file {
                self.forget(file, &Request::Signature, Purpose::Act);
            }
            return;
        }
        if showing {
            let at = self.cursor_point();
            if let Some(hint) = self.hint.as_mut() {
                hint.at = at;
            }
        }
        let Some(file) = file else {
            return;
        };
        let (Some(document), Some(path)) = (self.editor.get(file), self.editor.path(file)) else {
            return;
        };
        let servers = document.borrow().servers();
        if awaiting
            || servers
                .iter()
                .any(|client| client.signature_triggers(&path, showing).contains(&typed))
        {
            self.ask(Request::Signature);
        }
    }

    /// Asks `clients` again what could be written where the cursor is, for
    /// lists they said were not all there was.
    pub(super) fn ask_incomplete(&mut self, clients: Vec<Arc<Client>>) {
        if clients.is_empty() {
            return;
        }
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(at) = self
            .editor
            .get(file)
            .map(|document| document.borrow().buffer().selection().head)
        else {
            return;
        };
        let request = Request::Completions(Trigger::Incomplete);
        self.forget(file, &request, Purpose::Act);
        for client in clients {
            self.ask_of(client, file, at, request.clone(), Purpose::Act);
        }
    }

    /// Asks the server that offered the completions to fill in the selected
    /// one, the first time it is selected.
    ///
    /// A server may leave what an item says at length, and the edits it
    /// brings along, out of the list, and send them only for the item the
    /// reader is looking at; it is asked as the selection reaches each one.
    pub(super) fn resolve_completion(&mut self) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(completions) = self.completions.as_mut() else {
            return;
        };
        let Some(path) = self.editor.path(file) else {
            return;
        };
        let start = completions.start();
        for (client, handle) in completions.unasked() {
            let request = Request::ResolveCompletion(handle);
            if client.offers(&request, &path) {
                self.ask_of(client, file, start, request, Purpose::Act);
            }
        }
    }

    /// Takes in what a server filled a completion in with: into the list if
    /// it is still up, or into the file if it was already put in there.
    fn take_resolved(&mut self, pending: &Pending, item: pm_text::Completion) {
        let Request::ResolveCompletion(handle) = &pending.request else {
            return;
        };
        if let Some(completions) = self.completions.as_mut() {
            completions.fill(item.clone());
            return;
        }
        let Some(taken) = self.taken_completion.take_if(|taken| {
            taken.handle == *handle && Arc::ptr_eq(&taken.client, &pending.client)
        }) else {
            return;
        };
        let Some(document) = self.editor.get(taken.file) else {
            return;
        };
        if document.borrow().buffer().version() != taken.version || item.extra.is_empty() {
            return;
        }
        let before = item
            .extra
            .into_iter()
            .filter(|(span, _)| span.end <= taken.start)
            .collect::<Vec<_>>();
        document.borrow_mut().edit(|buffer| {
            let head = buffer.selection().head;
            buffer.complete(head..head, "", before);
        });
    }

    /// Waits for what a completion just put in brings along, when its server
    /// was asked to fill it in and has not answered yet.
    pub(super) fn await_taken_completion(
        &mut self,
        client: Arc<Client>,
        item: pm_text::Completion,
        start: Position,
        asked: bool,
    ) {
        self.taken_completion = None;
        let waiting = asked
            && self.asked.iter().any(|pending| {
                Arc::ptr_eq(&pending.client, &client)
                    && pending.request == Request::ResolveCompletion(item.handle.clone())
            });
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(document) = self.editor.get(file) else {
            return;
        };
        if waiting {
            self.taken_completion = Some(TakenCompletion {
                client,
                handle: item.handle,
                file,
                start,
                version: document.borrow().buffer().version(),
            });
        }
    }

    /// Opens the menu of fixes a server offers where the cursor is.
    fn show_code_actions(&mut self, pending: &Pending, actions: Vec<pm_text::CodeAction>) {
        self.code_actions
            .extend(actions.into_iter().map(|action| OfferedCodeAction {
                action,
                client: pending.client.clone(),
                file: pending.file,
                version: pending.version,
            }));
        if !self.code_actions.is_empty() {
            self.open_menu(crate::workspace::MenuTarget::CodeActions);
        }
    }

    /// Takes the `index`-th code action the server offered.
    pub(super) fn take_code_action(&mut self, index: usize) {
        let Some(offered) = self.code_actions.get(index) else {
            return;
        };
        if self
            .editor
            .get(offered.file)
            .is_none_or(|document| document.borrow().buffer().version() != offered.version)
        {
            return;
        }
        let client = offered.client.clone();
        let action = offered.action.clone();
        if self.apply_changes(action.edits)
            && let Some(command) = action.command
        {
            client.execute_command(command);
        }
    }

    /// Makes the changes a formatter asked for.
    pub(super) fn apply_edits(&mut self, files: Vec<FileEdit>) -> bool {
        self.apply_changes(files.into_iter().map(WorkspaceChange::Edit).collect())
    }

    /// Makes the changes a rename or a fix asked for, in the order asked.
    ///
    /// A file that is open takes its changes through the document it is open
    /// as, so the cursor, the undo history and the server all move with it; a
    /// file that is not open is rewritten on disk. Files are made, moved and
    /// taken away the way the file tree does it, and the tabs showing them
    /// follow. Nothing is done at all when a file to be moved or taken away
    /// has changes the reader has not saved: those are the reader's, and a
    /// server's rename is not something to lose them to.
    pub(super) fn apply_changes(&mut self, changes: Vec<WorkspaceChange>) -> bool {
        if let Some(unsaved) = changes.iter().find_map(|change| self.unsaved_under(change)) {
            self.notices.trouble(
                format!(
                    "Save {} before a server moves or removes it.",
                    unsaved.display()
                ),
                None,
            );
            return false;
        }
        let mut applied = true;
        let mut shifted = false;
        for change in changes {
            match change {
                WorkspaceChange::Edit(FileEdit { path, edits }) => {
                    applied &= self.edit_file(&path, edits);
                }
                WorkspaceChange::Create {
                    path,
                    overwrite,
                    ignore_if_exists,
                } => {
                    shifted = true;
                    let made = match std::fs::symlink_metadata(&path).is_ok() {
                        true if overwrite => std::fs::write(&path, "").is_ok(),
                        true => ignore_if_exists,
                        false => ops::create_file(&path).is_ok(),
                    };
                    if made {
                        self.tell_servers_made(std::slice::from_ref(&path));
                    }
                    applied &= made;
                }
                WorkspaceChange::Rename {
                    from,
                    to,
                    overwrite,
                    ignore_if_exists,
                } => {
                    shifted = true;
                    self.save_under(&from);
                    let taken = std::fs::symlink_metadata(&to).is_ok() && from != to;
                    let moved = match taken {
                        true if ignore_if_exists && !overwrite => continue,
                        true if overwrite => {
                            ops::remove(&to).and_then(|()| ops::rename(&from, &to))
                        }
                        _ => ops::rename(&from, &to),
                    };
                    applied &= moved.is_ok();
                    if moved.is_ok() {
                        self.retarget_tabs(&from, &to);
                        self.tell_servers_moved(&[(from, to)]);
                    }
                }
                WorkspaceChange::Delete {
                    path,
                    ignore_if_not_exists,
                } => {
                    shifted = true;
                    applied &= match std::fs::symlink_metadata(&path).is_ok() {
                        true => ops::trash(&path).is_ok(),
                        false => ignore_if_not_exists,
                    };
                    self.close_tabs_of(&path);
                    self.tell_servers_removed(std::slice::from_ref(&path));
                }
            }
        }
        if shifted {
            self.reread_worktree();
        }
        self.store();
        applied
    }

    /// Makes `edits` to the file at `path`: through its document when it is
    /// open, on disk when it is not.
    fn edit_file(
        &mut self,
        path: &std::path::Path,
        edits: Vec<(std::ops::Range<Position>, String)>,
    ) -> bool {
        if edits.is_empty() {
            return true;
        }
        match self
            .opened_file(path)
            .and_then(|file| self.editor.get(file))
        {
            Some(document) => {
                document
                    .borrow_mut()
                    .edit(|buffer| buffer.apply_edits(edits));
                true
            }
            None => write_through(path, edits),
        }
    }

    /// The open file at `path`, in whichever worktree has it open.
    fn opened_file(&self, path: &std::path::Path) -> Option<FileId> {
        self.scopes()
            .into_iter()
            .find_map(|scope| self.editor.opened(scope, path))
    }

    /// The open files at or under `path`.
    fn opened_under(&self, path: &std::path::Path) -> Vec<FileId> {
        self.panes
            .held()
            .into_iter()
            .filter_map(|item| item.file())
            .filter(|file| {
                self.editor
                    .path(*file)
                    .is_some_and(|open| open.starts_with(path))
            })
            .collect()
    }

    /// A file with unsaved changes that `change` would move or take away.
    fn unsaved_under(&self, change: &WorkspaceChange) -> Option<PathBuf> {
        let path = match change {
            WorkspaceChange::Rename { from, .. } => from,
            WorkspaceChange::Delete { path, .. } => path,
            WorkspaceChange::Edit(_) | WorkspaceChange::Create { .. } => return None,
        };
        self.opened_under(path)
            .into_iter()
            .find(|file| self.editor.is_dirty(*file))
            .and_then(|file| self.editor.path(file))
    }

    /// Writes the open files at or under `path` to disk, so that what a
    /// server changed in them moves with them.
    fn save_under(&mut self, path: &std::path::Path) {
        for file in self.opened_under(path) {
            if self.editor.is_dirty(file)
                && let Some(root) = self.worktree_of(file)
            {
                self.editor.save(file, &root);
            }
        }
    }

    /// The place `at` in the file at `path` comes to, in whichever worktree holds it.
    pub(super) fn place_of(&self, path: &std::path::Path, at: Position) -> Option<Place> {
        let scope = self
            .scopes()
            .into_iter()
            .filter_map(|scope| Some((scope, self.root_of(scope)?)))
            .filter(|(_, root)| path.starts_with(root))
            .max_by_key(|(_, root)| root.as_os_str().len())
            .map(|(scope, _)| scope)
            .or_else(|| self.scope())?;
        Some(Place {
            scope,
            path: path.to_path_buf(),
            position: at,
        })
    }

    /// `path` written from the worktree `scope` names down.
    fn relative_to(&self, scope: pm_core::Scope, path: &std::path::Path) -> String {
        let Some(root) = self.root_of(scope) else {
            return path.display().to_string();
        };
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

/// Makes `edits` to the file at `path`, which nothing has open.
fn write_through(path: &std::path::Path, edits: Vec<(std::ops::Range<Position>, String)>) -> bool {
    let Ok(mut buffer) = pm_text::Buffer::open(path) else {
        return false;
    };
    buffer.apply_edits(edits);
    buffer.save().is_ok()
}

/// The start of the last line of the log at `path`, where the newest of what
/// a server said is.
fn log_end(path: &std::path::Path) -> Position {
    let lines = std::fs::read_to_string(path).map_or(0, |text| text.lines().count());
    Position::new(lines.saturating_sub(1), 0)
}
