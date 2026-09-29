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

use pm_text::{
    Answer, Asked, Calls, Client, FileEdit, Lens, Location, NamedLocation, Position, Request,
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
                crate::config::InstallLanguageServers::Ask => self.notices.trouble(
                    format!("{} is not installed. Install it?", server.command),
                    Some(crate::message::Message::InstallLanguageServer(
                        server.command,
                    )),
                ),
                crate::config::InstallLanguageServers::Always => {
                    self.start_server_install(server.command, false);
                }
                crate::config::InstallLanguageServers::Never => {}
            }
            self.request_redraw();
        }
    }

    /// Starts one pinned install away from the window thread.
    pub(super) fn start_server_install(&mut self, command: &'static str, manual: bool) {
        if self.installing_servers.contains_key(command) {
            return;
        }
        let Some(recipe) = pm_text::install::recipe(command) else {
            return;
        };
        let Some(directory) = crate::config::servers() else {
            self.notices.trouble(
                format!("Installing {command} needs an editor home directory."),
                None,
            );
            return;
        };
        if manual {
            self.offered_servers.insert(command);
        }
        let notice = self.notices.progress(format!("Installing {command}…"));
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
            if let Some(notice) = self.installing_servers.remove(command) {
                self.notices.dismiss(notice);
            }
            match result {
                Ok(()) => {
                    let started = self.editor.reopen_command(command);
                    if started
                        && let Some(directory) = crate::config::servers()
                        && let Some(recipe) = pm_text::install::recipe(command)
                    {
                        pm_text::install::prune_older(&directory, command, recipe.version());
                    }
                    if let Some(recipe) = pm_text::install::recipe(command) {
                        self.notices
                            .done(format!("Installed {command} {}", recipe.version()), None);
                    }
                }
                Err(error) => self
                    .notices
                    .trouble(format!("Could not install {command}: {error}"), None),
            }
        }
        self.request_redraw();
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
        let request = match action {
            Action::GoToDefinition => Request::Definition,
            Action::GoToTypeDefinition => Request::TypeDefinition,
            Action::GoToImplementation => Request::Implementation,
            Action::GoToDeclaration => Request::Declaration,
            Action::FindReferences => Request::References,
            Action::ShowHover => Request::Hover,
            Action::ShowCompletions => Request::Completions,
            Action::ShowSignature => Request::Signature,
            Action::ShowCodeActions => Request::CodeActions,
            Action::Format => Request::Format,
            Action::ShowSymbols => Request::Symbols,
            Action::ShowIncomingCalls => Request::PrepareCalls(Calls::Incoming),
            Action::ShowOutgoingCalls => Request::PrepareCalls(Calls::Outgoing),
            Action::ShowWorkspaceSymbols => return self.open_picker(Kind::WorkspaceSymbols),
            Action::Rename => return self.open_prompt(Action::Rename),
            _ => return,
        };
        if self.say_unserved() {
            return;
        }
        self.ask(request);
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
        let (language, installable) = document.borrow().buffer().language().map_or_else(
            || (String::from("this file"), false),
            |language| {
                (
                    language.name().to_owned(),
                    self.editor.installable_server(language).is_some(),
                )
            },
        );

        let mut hint = Shown::at(self.cursor_point());
        hint.said = Some(format!(
            "No language server is running for {language}.{}",
            if installable {
                " Install it from the palette."
            } else {
                ""
            }
        ));
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
        for client in clients {
            let offered = client.offers(&request);
            self.ask_of(client, file, at, request.clone(), purpose);
            if offered && matches!(request, Request::Format | Request::WillSave) {
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
        if !client.offers(&request) {
            return;
        }
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let (version, indent, selection) = {
            let document = document.borrow();
            let buffer = document.buffer();
            let selected = buffer.selection();
            let selection = if selected.anchor == selected.head {
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
        self.collect_annotation_refreshes();
        self.cancel_stale_annotations();
        let Some(scope) = self.scope() else {
            return;
        };
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
            for client in clients {
                if client.offers(&Request::Semantics)
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
                    && client.offers(&Request::Lenses)
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
                let last = document.borrow().buffer().line_count().saturating_sub(1);
                let span = Position::default()..Position::new(last, 0);
                let request = Request::Hints(span);
                if self.preferences.inlay_hints
                    && client.offers(&request)
                    && document.borrow_mut().wants_hints(&client)
                {
                    self.ask_of(client, file, Position::default(), request, Purpose::Act);
                }
            }
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
                Request::Hints(_) | Request::Semantics | Request::Lenses | Request::ResolveLens(_)
            );
            let stale = annotation
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
        if !self.modifiers.control_key() {
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
                let success = supported && self.apply_edits(edits);
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
        if stale && (matches!(answer, Answer::Edits(_)) || pending.request == Request::CodeActions)
        {
            if self.saving && matches!(pending.request, Request::Format | Request::WillSave) {
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
                    _ => {}
                }
            }
            return self.save_once_formatted(pending);
        }
        if answer.is_empty()
            && !matches!(
                pending.request,
                Request::Hints(_) | Request::Semantics | Request::Lenses
            )
        {
            return self.save_once_formatted(pending);
        }
        match answer {
            Answer::Locations(found) if pending.purpose == Purpose::Link => {
                self.link_found(pending, &found);
            }
            Answer::Locations(found) if pending.request == Request::References => {
                self.show_references(found);
            }
            Answer::Locations(found) => self.go_to_first(&found),
            Answer::Hover(text) | Answer::Signature(text) => {
                if let Some(hint) = self.hint.as_mut() {
                    hint.said = Some(text);
                }
            }
            Answer::Completions(items) => self.show_completions(pending, items),
            Answer::CodeActions(actions) => self.show_code_actions(pending, actions),
            Answer::Edits(files) => {
                self.apply_edits(files);
                self.save_once_formatted(pending);
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

    /// Takes the next step after one formatting or pre-save reply.
    ///
    /// A save the servers take part in is three steps: the formatter's
    /// changes, then whatever each server wants changed before the file is
    /// written, then the writing. Each server is asked after the previous
    /// answer has been applied to the document.
    fn save_once_formatted(&mut self, pending: &Pending) {
        if !matches!(pending.request, Request::Format | Request::WillSave) {
            return;
        }
        if self.ask_next_edit_server(pending) {
            return;
        }
        if !self.saving {
            return;
        }
        match pending.request {
            Request::Format => self.ask_before_save(pending.file),
            _ => self.finish_save(),
        }
    }

    /// Asks the next capable server about the document left by the last one.
    fn ask_next_edit_server(&mut self, pending: &Pending) -> bool {
        let Some(document) = self.editor.get(pending.file) else {
            return false;
        };
        let (clients, at) = {
            let document = document.borrow();
            (document.servers(), document.buffer().selection().head)
        };
        let next = clients
            .into_iter()
            .skip_while(|client| !Arc::ptr_eq(client, &pending.client))
            .skip(1)
            .find(|client| client.offers(&pending.request));
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
    /// A save with no server to wait on is written at once.
    pub(super) fn begin_save(&mut self, format: bool) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        self.saving = true;
        if format {
            self.ask(Request::Format);
            if self.awaits(file, &Request::Format) {
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
    fn show_completions(&mut self, pending: &Pending, items: Vec<pm_text::Completion>) {
        if items.is_empty() {
            return;
        }
        let Some(document) = self.editor.get(pending.file) else {
            return;
        };
        let document = document.borrow();
        let word = document.buffer().word_at(pending.at);
        let typed = document
            .buffer()
            .text_in(word.start..document.buffer().selection().head);
        let under = document.layout().cell.height;
        let at = document.point_of(word.start);
        let point = pm_gfx::Point::new(at.x, at.y + under);
        drop(document);

        let mut completions = Completions::new(items, word.start, point);
        completions.narrow(&typed);
        self.completions = (!completions.is_empty()).then_some(completions);
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
        if self.apply_edits(action.edits)
            && let Some(command) = action.command
        {
            client.execute_command(command);
        }
    }

    /// Makes the changes a rename, a formatter or a fix asked for.
    ///
    /// A file that is open takes its changes through the document it is open
    /// as, so the cursor, the undo history and the server all move with it; a
    /// file that is not open is rewritten on disk.
    pub(super) fn apply_edits(&mut self, files: Vec<FileEdit>) -> bool {
        let mut applied = true;
        for FileEdit { path, edits } in files {
            if edits.is_empty() {
                continue;
            }
            let opened = self
                .scopes()
                .into_iter()
                .find_map(|scope| self.editor.opened(scope, &path));

            match opened.and_then(|file| self.editor.get(file)) {
                Some(document) => document
                    .borrow_mut()
                    .edit(|buffer| buffer.apply_edits(edits)),
                None => applied &= write_through(&path, edits),
            }
        }
        self.store();
        applied
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
fn write_through(path: &PathBuf, edits: Vec<(std::ops::Range<Position>, String)>) -> bool {
    let Ok(mut buffer) = pm_text::Buffer::open(path) else {
        return false;
    };
    buffer.apply_edits(edits);
    buffer.save().is_ok()
}
