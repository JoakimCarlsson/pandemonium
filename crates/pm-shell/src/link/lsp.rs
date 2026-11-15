//! The link's part in keeping the open files in step with their language
//! servers: opening a file for its server once its text arrives, telling it
//! each change, asking what the editor asks for, taking in what the server
//! says, making the edits it answers with and closing a file when its tab
//! closes. Every request goes to the server holding the file, so remote
//! projects work the same.

use pm_editor::{Document, FIX_ALL, ORGANIZE_IMPORTS, ServerAsk};
use pm_lsp::{FileEdit, Seek};
use pm_protocol::{Request, Response};

use super::{Ask, Link};
use crate::open_files::{EditorPopup, FileBody, OpenFile};

/// What a request to a file's language server was for, naming the file by
/// its folder and path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ServerFor {
    /// Opening the file for its server, at this version.
    Opened(String, String, i32),
    /// The completions asked at this version.
    Completions(String, String, i32),
    /// The rest of the completion picked.
    Resolved(String, String),
    /// What the server says about a place, asked at this version.
    Hovered(String, String, i32),
    /// The signature of the call the caret is in, asked at this version.
    Signature(String, String, i32),
    /// The places the name at the caret is used, asked at this version.
    Highlights(String, String, i32),
    /// Where the name at a place leads.
    Seek(String, String),
    /// What can be renamed at the caret.
    RenameTarget(String, String),
    /// The changes offered at the caret, asked at this version on this line.
    Actions(String, String, i32, usize),
    /// The changes of one kind for the whole file, taken as soon as they are offered.
    SourceAction(String, String),
    /// The edits a change, a rename or formatting makes.
    Edited(String, String),
    /// The ways a colour can be written.
    Colour(String, String),
    /// What each name of the file is, asked at this version.
    Tokens(String, String, i32),
    /// What each name within a stretch of the file is, asked at this version.
    TokenRange(String, String, i32, pm_editor::Span),
    /// The hints inside the lines, asked at this version.
    Hints(String, String, i32),
    /// The lenses, asked at this version.
    Lenses(String, String, i32),
    /// The colours spelled in the file, asked at this version.
    Colours(String, String, i32),
    /// Formatting the file before it is written, as the settings ask.
    FormatThenSave(String, String),
    /// The blocks of lines that can be folded, asked at this version.
    Folds(String, String, i32),
    /// The places related to the thing at a place as the seek looks for them, such as the functions calling it, listed to choose from.
    Related(String, String, Seek),
    /// The code actions of one kind offered for the whole file before it is saved, with the kinds still to ask for after it.
    SaveActions(String, String, Vec<String>),
    /// Taking a code action offered before the file is saved, with the kinds still to ask for after it.
    SaveActionTaken(String, String, Vec<String>),
    /// The stretches edited together with the one at the caret, asked at this version.
    Linked(String, String, i32),
}

impl Link {
    /// Opens the open file at `index` for its language server, when it is
    /// text written in a language the server knows and not open for it yet.
    pub(super) fn open_for_server(&mut self, index: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        if file.body != FileBody::Text {
            return;
        }
        let document = file.document.0.borrow();
        let Some(language) = document.language.clone() else {
            return;
        };
        if document.synced {
            return;
        }
        let version = document.version.max(1);
        let text = document.field.text().to_owned();
        drop(document);
        let (directory, path) = (file.directory.clone(), file.path.clone());
        self.ask(
            Request::OpenDocument {
                directory: directory.clone(),
                path: path.clone(),
                language,
                version,
                text,
            },
            Ask::Server(ServerFor::Opened(directory, path, version)),
        );
    }

    /// Opens every open file of this machine for its language server again, as the server forgot them when the connection dropped.
    pub(super) fn reopen_for_server(&mut self) {
        for index in 0..self.workspace.open_files.files.len() {
            let file = &self.workspace.open_files.files[index];
            if pm_protocol::host_of(&file.directory).is_some() {
                continue;
            }
            file.document.0.borrow_mut().synced = false;
            self.open_for_server(index);
        }
    }

    /// Takes in `diagnostics`, what a language server found wrong with the
    /// file at `path` in `directory`: listed in the problems, kept for when
    /// the file opens, and handed to its editor when it is open.
    pub(super) fn take_diagnostics(
        &mut self,
        directory: String,
        path: String,
        diagnostics: Vec<pm_protocol::Diagnostic>,
    ) {
        if let Some(index) = self.workspace.open_files.position(&directory, &path) {
            self.workspace.open_files.files[index]
                .document
                .0
                .borrow_mut()
                .diagnostics = diagnostics.clone();
        }
        let key = (directory.clone(), path.clone());
        if diagnostics.is_empty() {
            self.diagnosed.remove(&key);
        } else {
            self.diagnosed.insert(key, diagnostics.clone());
        }
        self.change_dock(|dock| dock.diagnostics(directory, path, diagnostics));
    }

    /// Hands the open file at `index` what its language server last found
    /// wrong with it, once its text has arrived.
    pub(super) fn hand_diagnostics(&mut self, index: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let key = (file.directory.clone(), file.path.clone());
        if let Some(found) = self.diagnosed.get(&key) {
            file.document.0.borrow_mut().diagnostics = found.clone();
        }
    }

    /// Tells the language servers that the files `closed`, by folder and
    /// path, are no longer open.
    pub(super) fn close_for_server(&mut self, closed: Vec<(String, String)>) {
        for (directory, path) in closed {
            self.ask(Request::CloseDocument { directory, path }, Ask::Nothing);
        }
    }

    /// Carries out `ask`, which the editor of the open file at `index` made
    /// of the file's language server.
    pub fn code(&mut self, index: usize, ask: ServerAsk) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let (directory, path) = (file.directory.clone(), file.path.clone());
        let document = file.document.clone();
        let version = document.0.borrow().version;
        let (request, purpose) = match ask {
            ServerAsk::CaretLine(line) => {
                self.ask_blame(index, line);
                return;
            }
            ServerAsk::Changed(version, change) => {
                self.ask(
                    Request::ChangeDocument {
                        directory: directory.clone(),
                        path: path.clone(),
                        version,
                        changes: vec![change],
                    },
                    Ask::Nothing,
                );
                self.ask_annotations(index);
                return;
            }
            ServerAsk::Complete(version, position, trigger) => (
                Request::Completion {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    position,
                    trigger,
                    scope: None,
                },
                ServerFor::Completions(directory, path, version),
            ),
            ServerAsk::Resolve(item) => (
                Request::ResolveCompletion {
                    directory: directory.clone(),
                    path: path.clone(),
                    item,
                },
                ServerFor::Resolved(directory, path),
            ),
            ServerAsk::Hover(version, position) => (
                Request::Hover {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    position,
                },
                ServerFor::Hovered(directory, path, version),
            ),
            ServerAsk::Signature(version, position) => (
                Request::Signature {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    position,
                },
                ServerFor::Signature(directory, path, version),
            ),
            ServerAsk::Highlights(version, position) => {
                if self.settings.editor.linked_editing {
                    self.ask(
                        Request::LinkedRanges {
                            directory: directory.clone(),
                            path: path.clone(),
                            version,
                            position,
                        },
                        Ask::Server(ServerFor::Linked(directory.clone(), path.clone(), version)),
                    );
                }
                (
                    Request::Highlights {
                        directory: directory.clone(),
                        path: path.clone(),
                        version,
                        position,
                    },
                    ServerFor::Highlights(directory, path, version),
                )
            }
            ServerAsk::Seek(_, Seek::References) => {
                self.workspace.open_files.tab = crate::open_files::SessionTab::File(index);
                self.open_choices(crate::quick::Choosing::References);
                return;
            }
            ServerAsk::Seek(position, seek) => {
                let purpose = if is_hierarchy(seek) {
                    ServerFor::Related(directory.clone(), path.clone(), seek)
                } else {
                    ServerFor::Seek(directory.clone(), path.clone())
                };
                (
                    Request::Definition {
                        directory,
                        path,
                        version,
                        position,
                        seek,
                    },
                    purpose,
                )
            }
            ServerAsk::PrepareRename(position) => (
                Request::PrepareRename {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    position,
                },
                ServerFor::RenameTarget(directory, path),
            ),
            ServerAsk::Rename(position, name) => (
                Request::Rename {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    position,
                    name,
                },
                ServerFor::Edited(directory, path),
            ),
            ServerAsk::CodeActions(span, only) => {
                let line = span.start.line as usize;
                let purpose = if only.as_ref().is_some_and(|kinds| {
                    kinds
                        .iter()
                        .any(|kind| kind == ORGANIZE_IMPORTS || kind == FIX_ALL)
                }) {
                    ServerFor::SourceAction(directory.clone(), path.clone())
                } else {
                    ServerFor::Actions(directory.clone(), path.clone(), version, line)
                };
                (
                    Request::CodeActions {
                        directory,
                        path,
                        version,
                        span,
                        only,
                    },
                    purpose,
                )
            }
            ServerAsk::TakeAction(place) => (
                Request::TakeCodeAction {
                    directory: directory.clone(),
                    path: path.clone(),
                    index: place,
                },
                ServerFor::Edited(directory, path),
            ),
            ServerAsk::PressLens(place) => (
                Request::PressCodeLens {
                    directory: directory.clone(),
                    path: path.clone(),
                    index: place,
                },
                ServerFor::Edited(directory, path),
            ),
            ServerAsk::Colour(span, color) => (
                Request::ColorPresentations {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    span,
                    color,
                },
                ServerFor::Colour(directory, path),
            ),
            ServerAsk::Format(range) => (
                Request::Format {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    range,
                },
                ServerFor::Edited(directory, path),
            ),
            ServerAsk::FormatOnType(version, position, character) => (
                Request::FormatOnType {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    position,
                    character,
                },
                ServerFor::Edited(directory, path),
            ),
        };
        self.ask(request, Ask::Server(purpose));
    }

    /// Asks the language server of the open file at `index` for what it
    /// shows in and beside the text, each when it has not said it for the
    /// text as it is now.
    fn ask_annotations(&mut self, index: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let (directory, path) = (file.directory.clone(), file.path.clone());
        let shared = file.document.clone();
        let mut document = shared.0.borrow_mut();
        let version = document.version;
        let mut asks = Vec::new();
        let editor = &self.settings.editor;
        if editor.semantic_highlighting && document.lines() > editor.semantic_range_lines {
            let caret = document.line_of(document.field.caret());
            let margin = editor.semantic_range_margin.max(1);
            let lines = caret.saturating_sub(margin)..caret + margin + 1;
            if let Some(span) = document.semantic_range_wanted(lines.clone()) {
                document.semantic_range_asked(version, lines);
                asks.push((
                    Request::SemanticTokens {
                        directory: directory.clone(),
                        path: path.clone(),
                        version,
                        rules: editor.semantic_token_rules.clone(),
                        range: Some(span),
                    },
                    ServerFor::TokenRange(directory.clone(), path.clone(), version, span),
                ));
            }
        }
        if editor.semantic_highlighting && document.wants_semantic_tokens() {
            document.semantic_tokens_asked(version);
            asks.push((
                Request::SemanticTokens {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    rules: editor.semantic_token_rules.clone(),
                    range: None,
                },
                ServerFor::Tokens(directory.clone(), path.clone(), version),
            ));
        }
        if document.wants_inlay_hints() {
            document.inlay_hints_asked(version);
            asks.push((
                Request::InlayHints {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    span: document.whole_span(),
                },
                ServerFor::Hints(directory.clone(), path.clone(), version),
            ));
        }
        if document.wants_code_lenses() {
            document.code_lenses_asked(version);
            asks.push((
                Request::CodeLenses {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    dependencies: editor.dependency_hints,
                },
                ServerFor::Lenses(directory.clone(), path.clone(), version),
            ));
        }
        if document.wants_colors() {
            document.colors_asked(version);
            asks.push((
                Request::Colors {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                },
                ServerFor::Colours(directory.clone(), path.clone(), version),
            ));
        }
        if document.folds_supported && document.server_folds_version() != Some(version) {
            document.take_folds(version, Vec::new());
            asks.push((
                Request::FoldingRanges {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                },
                ServerFor::Folds(directory, path, version),
            ));
        }
        drop(document);
        for (request, purpose) in asks {
            self.ask(request, Ask::Server(purpose));
        }
    }

    /// The position among the open files of the file at `path` in `directory`.
    fn served_file(&mut self, directory: &str, path: &str) -> Option<usize> {
        self.workspace.open_files.position(directory, path)
    }

    /// Handles the `response` of a language server to a request made for `purpose`.
    pub(super) fn answer_server(&mut self, purpose: ServerFor, response: Response) {
        if let Response::Failed { message } = &response {
            match &purpose {
                ServerFor::SaveActions(directory, path, kinds)
                | ServerFor::SaveActionTaken(directory, path, kinds) => {
                    if let Some(index) = self.served_file(directory, path) {
                        self.save_with_actions(index, kinds.clone());
                    }
                    return;
                }
                ServerFor::FormatThenSave(directory, path) => {
                    if let Some(index) = self.served_file(directory, path) {
                        self.write_file(index);
                    }
                    return;
                }
                _ => {}
            }
            if matches!(
                purpose,
                ServerFor::Seek(..)
                    | ServerFor::Related(..)
                    | ServerFor::RenameTarget(..)
                    | ServerFor::Edited(..)
                    | ServerFor::SourceAction(..)
            ) {
                self.workspace.status = Some(message.clone());
            }
            return;
        }
        match (purpose, response) {
            (
                ServerFor::Opened(directory, path, version),
                Response::Opened {
                    triggers,
                    signature,
                    format,
                    formats_ranges,
                    folds,
                    links,
                    semantic_tokens,
                    inlay_hints,
                    code_lens,
                    inline_values,
                    document_links,
                    colors,
                    semantic_ranges,
                },
            ) => {
                let Some(index) = self.served_file(&directory, &path) else {
                    return;
                };
                {
                    let file = &self.workspace.open_files.files[index];
                    let mut document = file.document.0.borrow_mut();
                    document.synced = true;
                    document.version = version;
                    document.triggers = triggers;
                    document.signature_triggers = signature;
                    document.format_triggers = format;
                    document.formats_ranges = formats_ranges;
                    document.folds_supported = folds;
                    document.links_supported = links;
                    document.semantic_tokens_supported = semantic_tokens;
                    document.semantic_ranges_supported = semantic_ranges;
                    document.inlay_hints_supported = inlay_hints;
                    document.code_lens_supported = code_lens;
                    document.inline_values_supported = inline_values;
                    document.document_links_supported = document_links;
                    document.colors_supported = colors;
                }
                self.ask_annotations(index);
            }
            (ServerFor::Completions(directory, path, version), Response::Completions { items }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_completions(version, items)
                });
            }
            (ServerFor::Resolved(directory, path), Response::CompletionResolved { item }) => {
                self.with_document(&directory, &path, |document| document.take_resolved(item));
            }
            (ServerFor::Hovered(directory, path, _), Response::Hovered { hover }) => {
                self.with_document(&directory, &path, |document| match hover {
                    Some(hover) => {
                        if let Some(card) = &mut document.hover {
                            card.text = Some(hover.text);
                        }
                    }
                    None => {
                        let keep = document
                            .hover
                            .as_ref()
                            .is_some_and(|card| !card.diagnostics.is_empty());
                        if !keep {
                            document.hover = None;
                        }
                    }
                });
            }
            (ServerFor::Signature(directory, path, version), Response::Signature { signature }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_signature(version, signature)
                });
            }
            (ServerFor::Highlights(directory, path, version), Response::Highlights { spans }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_highlights(version, spans)
                });
            }
            (ServerFor::Linked(directory, path, version), Response::LinkedRanges { spans }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_linked(version, spans)
                });
            }
            (ServerFor::Seek(..), Response::Definitions { locations }) => match locations.first() {
                Some(location) if locations.len() == 1 => {
                    let line = location.span.start.line as usize + 1;
                    self.open_location(location.path.clone(), line);
                }
                Some(_) => self.list_locations(crate::quick::Choosing::References, locations),
                None => self.workspace.status = Some("Nothing found there.".to_owned()),
            },
            (ServerFor::Related(_, _, seek), Response::Definitions { locations }) => {
                if locations.is_empty() {
                    self.workspace.status =
                        Some(format!("No {} found there.", seek.title().to_lowercase()));
                } else {
                    self.list_locations(crate::quick::Choosing::Related(seek), locations);
                }
            }
            (ServerFor::SaveActions(directory, path, kinds), Response::CodeActions { actions }) => {
                let Some(index) = self.served_file(&directory, &path) else {
                    return;
                };
                match actions.iter().position(|action| action.disabled.is_none()) {
                    Some(place) => self.ask(
                        Request::TakeCodeAction {
                            directory: directory.clone(),
                            path: path.clone(),
                            index: place,
                        },
                        Ask::Server(ServerFor::SaveActionTaken(directory, path, kinds)),
                    ),
                    None => self.save_with_actions(index, kinds),
                }
            }
            (ServerFor::SaveActionTaken(directory, path, kinds), response) => {
                if let Response::Edited { files } | Response::ActionTaken { files, .. } = response {
                    self.make_edits(files);
                }
                if let Some(index) = self.served_file(&directory, &path) {
                    self.save_with_actions(index, kinds);
                }
            }
            (ServerFor::SaveActions(directory, path, kinds), _) => {
                if let Some(index) = self.served_file(&directory, &path) {
                    self.save_with_actions(index, kinds);
                }
            }
            (ServerFor::RenameTarget(directory, path), Response::RenameTarget { target }) => {
                let Some(index) = self.served_file(&directory, &path) else {
                    return;
                };
                let file = &mut self.workspace.open_files.files[index];
                let document = file.document.0.borrow();
                let caret = document.field.caret();
                let range = match target.as_ref().and_then(|target| target.span) {
                    Some(span) => document.range_of(span),
                    None => document.field.word_range_at(caret),
                };
                let name = target
                    .and_then(|target| target.placeholder)
                    .unwrap_or_else(|| document.field.text()[range.clone()].to_owned());
                drop(document);
                if name.is_empty() {
                    self.workspace.status = Some("Nothing to rename there.".to_owned());
                    return;
                }
                file.rename = Some((range, name));
            }
            (
                ServerFor::Actions(directory, path, version, line),
                Response::CodeActions { actions },
            ) => {
                let Some(index) = self.served_file(&directory, &path) else {
                    return;
                };
                let file = &mut self.workspace.open_files.files[index];
                let offered = actions
                    .iter()
                    .filter(|action| action.disabled.is_none())
                    .count();
                file.document
                    .0
                    .borrow_mut()
                    .take_bulb(version, line, offered);
                file.actions = actions;
            }
            (ServerFor::SourceAction(directory, path), Response::CodeActions { actions }) => {
                let Some(index) = self.served_file(&directory, &path) else {
                    return;
                };
                match actions.iter().position(|action| action.disabled.is_none()) {
                    Some(place) => self.code(index, ServerAsk::TakeAction(place)),
                    None => self.workspace.status = Some("Nothing to change.".to_owned()),
                }
            }
            (
                ServerFor::Edited(..),
                Response::Edited { files }
                | Response::ActionTaken { files, .. }
                | Response::LensPressed { files, .. },
            ) => self.make_edits(files),
            (ServerFor::FormatThenSave(directory, path), response) => {
                let Some(index) = self.workspace.open_files.position(&directory, &path) else {
                    return;
                };
                if let Response::Formatted { edits } = response {
                    self.edit_open_file(index, edits);
                }
                self.write_file(index);
            }
            (ServerFor::Edited(_, path), Response::Formatted { edits }) => {
                self.make_edits(vec![FileEdit {
                    path,
                    changes: edits,
                }]);
            }
            (ServerFor::Colour(_, path), Response::ColorPresentations { presentations }) => {
                if let Some(presentation) = presentations.into_iter().next() {
                    let mut changes = vec![pm_lsp::TextChange {
                        span: Some(presentation.span),
                        text: presentation.text,
                    }];
                    changes.extend(presentation.additional_edits);
                    self.make_edits(vec![FileEdit { path, changes }]);
                }
            }
            (
                ServerFor::Tokens(directory, path, version),
                Response::SemanticTokens { tokens, .. },
            ) => {
                self.with_document(&directory, &path, |document| {
                    document.take_semantic_tokens(version, tokens)
                });
            }
            (
                ServerFor::TokenRange(directory, path, version, span),
                Response::SemanticTokens { tokens, .. },
            ) => {
                self.with_document(&directory, &path, |document| {
                    document.take_semantic_range(version, span, tokens)
                });
            }
            (ServerFor::Hints(directory, path, version), Response::InlayHints { hints }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_inlay_hints(version, hints)
                });
            }
            (ServerFor::Lenses(directory, path, version), Response::CodeLenses { lenses }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_code_lenses(version, lenses)
                });
            }
            (ServerFor::Colours(directory, path, version), Response::Colors { colors }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_colors(version, colors)
                });
            }
            (ServerFor::Folds(directory, path, version), Response::FoldingRanges { ranges }) => {
                self.with_document(&directory, &path, |document| {
                    document.take_folds(version, ranges)
                });
            }
            _ => {}
        }
    }

    /// Changes the document of the open file at `path` in `directory` with
    /// `change`, when the file is open.
    fn with_document(&mut self, directory: &str, path: &str, change: impl FnOnce(&mut Document)) {
        if let Some(index) = self.served_file(directory, path) {
            let file = &self.workspace.open_files.files[index];
            if let Ok(mut document) = file.document.0.try_borrow_mut() {
                change(&mut document);
            }
        }
    }

    /// Makes the edits a language server answered with: in the editor of
    /// each file that is open, left unsaved, and for the others in a tab
    /// opened for them, once their text arrives.
    pub(super) fn make_edits(&mut self, files: Vec<FileEdit>) {
        let shown = self.workspace.open_files.tab;
        for edit in files {
            let directory = self
                .workspace
                .open_files
                .shown()
                .map(|file| file.directory.clone())
                .or_else(|| {
                    self.workspace
                        .open_files
                        .files
                        .first()
                        .map(|file| file.directory.clone())
                });
            let Some(directory) = directory else {
                continue;
            };
            let index = match self.workspace.open_files.position(&directory, &edit.path) {
                Some(index) => index,
                None => {
                    let opened = self.workspace.open_files.open(&directory, &edit.path, true);
                    if let Some(index) = opened {
                        self.workspace.open_files.files[index].pending = edit.changes;
                        self.read_file(directory.clone(), edit.path.clone());
                    }
                    continue;
                }
            };
            self.edit_open_file(index, edit.changes);
            self.workspace.open_files.files[index].preview = false;
        }
        self.workspace.open_files.tab = shown;
    }

    /// Makes `changes`, which a language server asked for, in the open file
    /// at `index`, telling its server of each; changes to a file whose text
    /// has not arrived yet wait until it does.
    pub(super) fn edit_open_file(&mut self, index: usize, changes: Vec<pm_lsp::TextChange>) {
        let edits = {
            let file: &OpenFile = &self.workspace.open_files.files[index];
            if file.body != FileBody::Text {
                self.workspace.open_files.files[index]
                    .pending
                    .extend(changes);
                return;
            }
            let document = file.document.0.borrow();
            document.change_edits(changes)
        };
        let told = {
            let file = &self.workspace.open_files.files[index];
            let mut document = file.document.0.borrow_mut();
            let mut told = Vec::new();
            for change in edits {
                if let Some(edited) = document.edit(change)
                    && let Some(change) = edited.change
                {
                    told.push(change);
                }
            }
            told
        };
        for (version, change) in told {
            self.code(index, ServerAsk::Changed(version, change));
        }
    }

    /// Lists `locations`, places a language server found, in the quick input as the list of choices `choosing`, to open one.
    fn list_locations(
        &mut self,
        choosing: crate::quick::Choosing,
        locations: Vec<pm_lsp::Location>,
    ) {
        self.workspace.quick = Some(crate::quick::QuickInput {
            choosing: Some(choosing),
            found: super::listing::location_items(locations),
            ..crate::quick::QuickInput::default()
        });
    }

    /// Closes the popups the window opened over the editor of the open file at `index`.
    pub fn dismiss_popups(&mut self, index: usize) {
        if let Some(file) = self.workspace.open_files.files.get_mut(index) {
            file.rename = None;
            file.popup = None::<EditorPopup>;
        }
    }
}

/// Whether `seek` looks for the functions or types related to a function or a type, which are listed to choose from however many are found.
fn is_hierarchy(seek: Seek) -> bool {
    matches!(
        seek,
        Seek::IncomingCalls | Seek::OutgoingCalls | Seek::Supertypes | Seek::Subtypes
    )
}
