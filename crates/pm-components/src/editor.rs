//! Writing code in the file shown: edits kept in step with the syntax tree and the language server, new lines indented as the language says, completions at the caret and what the server says about a place.

use pm_protocol::{Completion, Diagnostic, Hover, Position};
use pm_text::Edit;
use pm_treesitter::Syntax;

use crate::open_files::{Contents, Viewer};
use crate::state::{Outgoing, WorkspaceState};
use pm_editor::{Document, HoverCard};

/// How many completions the list shows at once.
pub const COMPLETIONS_SHOWN: usize = 10;

/// Whether `viewer` shows the file at `path` as it is in the folder.
fn is_file(viewer: &Viewer, path: &str) -> bool {
    viewer.path == path && !viewer.head
}

/// The text of `viewer`, when it holds text.
fn document_of(viewer: &mut Viewer) -> Option<&mut Document> {
    match &mut viewer.contents {
        Contents::Text(document) => Some(document),
        _ => None,
    }
}

impl WorkspaceState {
    /// The session the file shown belongs to, by position, and the file shown.
    fn shown_mut(&mut self) -> Option<(usize, &mut Viewer)> {
        let session = self.review_index()?;
        let index = self.git.shown?;
        Some((session, self.git.viewers.get_mut(index)?))
    }

    /// Every open file of every session, whether its tab is showing or kept aside, with the session it belongs to.
    pub(crate) fn every_viewer(&mut self) -> Vec<(usize, &mut Viewer)> {
        let owner = self.files_owner;
        let mut found: Vec<(usize, &mut Viewer)> = Vec::new();
        if let Some(owner) = owner {
            found.extend(self.git.viewers.iter_mut().map(|viewer| (owner, viewer)));
        }
        for (session, (viewers, _)) in self.file_stash.iter_mut() {
            found.extend(viewers.iter_mut().map(|viewer| (*session, viewer)));
        }
        found
    }

    /// The open file at `path` of the session at `session`, as it is in the folder.
    pub(crate) fn file_mut(&mut self, session: usize, path: &str) -> Option<&mut Document> {
        self.every_viewer()
            .into_iter()
            .find(|(owner, viewer)| *owner == session && is_file(viewer, path))
            .and_then(|(_, viewer)| document_of(viewer))
    }

    /// Takes in what the server said the file at `path` of the session at `session` is written in: parses it when the grammar is loaded and asks for the grammar when it is not, and opens it for the language server when it can be edited.
    pub(crate) fn took_language(
        &mut self,
        session: usize,
        path: &str,
        head: bool,
        language: Option<String>,
    ) {
        let Some(language) = language else {
            return;
        };
        let mut wants_grammar = false;
        let mut opening = None;
        if let Some((_, viewer)) = self
            .every_viewer()
            .into_iter()
            .find(|(owner, viewer)| *owner == session && viewer.path == path && viewer.head == head)
            && let Some(document) = document_of(viewer)
        {
            document.language = Some(language.clone());
            match pm_treesitter::registry::loaded(&language) {
                Some(grammar) => {
                    if let Some(syntax) = Syntax::parse(grammar, document.field.text()) {
                        document.with_syntax(syntax);
                    }
                }
                None => wants_grammar = true,
            }
            if !head && !document.field.is_read_only() {
                document.synced = true;
                document.version = 1;
                opening = Some(document.field.text().to_owned());
            }
        }
        if wants_grammar {
            self.outbox.push(Outgoing::Grammar {
                session,
                language: language.clone(),
            });
        }
        if let Some(text) = opening {
            self.outbox.push(Outgoing::OpenDocument {
                session,
                path: path.to_owned(),
                language,
                version: 1,
                text,
            });
        }
        self.apply_indentation();
    }

    /// The state after the server of `host` said how its languages stand: an open file of a project on that machine that was read before the language it is written in was installed there takes that language now.
    pub fn with_host_extensions(
        mut self,
        host: &Option<String>,
        languages: &[pm_protocol::ExtensionInfo],
    ) -> Self {
        crate::file_icon::learn_languages(host, languages);
        let owners: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                self.projects
                    .get(session.project)
                    .is_some_and(|project| project.host() == *host)
            })
            .map(|(place, _)| place)
            .collect();
        let waiting: Vec<(usize, String, bool, String)> = self
            .every_viewer()
            .into_iter()
            .filter(|(session, _)| owners.contains(session))
            .filter_map(|(session, viewer)| {
                let Contents::Text(document) = &viewer.contents else {
                    return None;
                };
                if document.language.is_some() {
                    return None;
                }
                let suffix = viewer.path.rsplit_once('.')?.1.to_ascii_lowercase();
                let language = languages.iter().find(|language| {
                    language.installed
                        && language
                            .suffixes
                            .iter()
                            .any(|held| held.eq_ignore_ascii_case(&suffix))
                })?;
                Some((
                    session,
                    viewer.path.clone(),
                    viewer.head,
                    language.id.clone(),
                ))
            })
            .collect();
        for (session, path, head, language) in waiting {
            self.took_language(session, &path, head, Some(language));
        }
        self
    }

    /// The state after the grammar of `language` was loaded: every open file written in it that is not parsed yet is parsed.
    pub fn with_grammar(mut self, language: &str) -> Self {
        let Some(grammar) = pm_treesitter::registry::loaded(language) else {
            return self;
        };
        for (_, viewer) in self.every_viewer() {
            let Some(document) = document_of(viewer) else {
                continue;
            };
            document.repaint();
            if document.syntax.is_none()
                && document.language.as_deref() == Some(language)
                && let Some(syntax) = Syntax::parse(grammar.clone(), document.field.text())
            {
                document.with_syntax(syntax);
            }
        }
        self.apply_indentation();
        self
    }

    /// Opens again for its language server every open file of a project on `host` that the server was told of, as the connection to that server was made again and the server may have started afresh.
    pub fn reopen_documents(mut self, host: &Option<String>) -> Self {
        let owners: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                self.projects
                    .get(session.project)
                    .is_some_and(|project| project.host() == *host)
            })
            .map(|(place, _)| place)
            .collect();
        let reopened: Vec<Outgoing> = self
            .every_viewer()
            .into_iter()
            .filter(|(session, viewer)| owners.contains(session) && !viewer.head)
            .filter_map(|(session, viewer)| {
                let path = viewer.path.clone();
                let document = document_of(viewer).filter(|document| document.synced)?;
                Some(Outgoing::OpenDocument {
                    session,
                    path,
                    language: document.language.clone()?,
                    version: document.version,
                    text: document.field.text().to_owned(),
                })
            })
            .collect();
        self.outbox.extend(reopened);
        self
    }

    /// Asks for the grammars of the languages named `names`, which stretches of the open files are written in, from the server of the session the review panel is about.
    pub fn grammars_wanted(mut self, names: Vec<String>) -> Self {
        if let Some(session) = self.review_index() {
            for language in names {
                self.outbox.push(Outgoing::Grammar { session, language });
            }
        }
        self
    }

    /// The state after the language server opened the file at `path` of the session at `session`, saying something unasked after `triggers`, or could not, for `problem`.
    pub fn with_opened(
        mut self,
        session: usize,
        path: &str,
        outcome: Result<pm_protocol::Triggers, String>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            match outcome {
                Ok(triggers) => {
                    document.triggers = triggers.completion;
                    document.signature_triggers = triggers.signature;
                    document.server_problem = None;
                }
                Err(problem) => {
                    document.synced = false;
                    document.server_problem = Some(problem);
                }
            }
        }
        self
    }

    /// The state after the language server of `host` said what it finds wrong with the file at `path` of the folder `directory`.
    pub fn with_diagnostics(
        mut self,
        host: &Option<String>,
        directory: &str,
        path: &str,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let sessions: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                session.directory == directory
                    && self
                        .projects
                        .get(session.project)
                        .is_some_and(|project| project.host() == *host)
            })
            .map(|(place, _)| place)
            .collect();
        for session in sessions {
            if let Some(document) = self.file_mut(session, path) {
                document.diagnostics = diagnostics.clone();
            }
        }
        self
    }

    /// Tells the language server the file at `path` of the session at `session` is closed, when it was told the file is open.
    pub(crate) fn close_document(&mut self, session: usize, viewer: &Viewer) {
        if let Contents::Text(document) = &viewer.contents
            && document.synced
            && !viewer.head
        {
            self.outbox.push(Outgoing::CloseDocument {
                session,
                path: viewer.path.clone(),
            });
        }
    }

    /// Applies `edit` to the file shown, keeping its syntax tree and its language server in step, and the completions open at the caret with it; changing its text keeps its tab open.
    pub(crate) fn file_edited(&mut self, edit: Edit) {
        let settings = self.editor.clone();
        let Some((session, viewer)) = self.shown_mut() else {
            return;
        };
        let told = edit_viewer(session, viewer, edit, &settings);
        self.outbox.extend(told);
    }

    /// Makes `changes`, each a stretch of the text as it is now and what replaces it, to the open file at `path` of the session at `session`, whether its tab is showing or not.
    pub(crate) fn change_open_file(
        &mut self,
        session: usize,
        path: &str,
        changes: Vec<pm_protocol::TextChange>,
    ) {
        let settings = self.editor.clone();
        let mut told = Vec::new();
        if let Some((_, viewer)) = self
            .every_viewer()
            .into_iter()
            .find(|(owner, viewer)| *owner == session && is_file(viewer, path))
        {
            let edits = document_of(viewer)
                .map(|document| document.change_edits(changes))
                .unwrap_or_default();
            for edit in edits {
                told.extend(edit_viewer(session, viewer, edit, &settings));
            }
        }
        self.outbox.extend(told);
    }

    /// Makes `edits` to the file shown, one after another.
    fn file_edits(&mut self, edits: Vec<Edit>) {
        for edit in edits {
            self.file_edited(edit);
        }
    }

    /// Breaks the line at the caret of the file shown, indenting the new line as the language says: one level deeper inside a block opened on the line, and the closing of that block put on a line of its own.
    pub fn editor_newline(mut self) -> Self {
        let (indent, comments) = (
            self.editor.auto_indent,
            self.editor.extend_comment_on_newline,
        );
        let edits = self
            .shown_document()
            .map(|document| {
                if document.field.has_others() {
                    vec![Edit::Insert('\n')]
                } else {
                    document.newline_edits_with(indent, comments)
                }
            })
            .unwrap_or_default();
        self.file_edits(edits);
        self
    }

    /// Types one level of indentation at the caret of the file shown.
    pub fn editor_indent(mut self) -> Self {
        let Some(unit) = self
            .shown_document()
            .map(|document| document.editing.unit(1))
        else {
            return self;
        };
        self.file_edited(Edit::Paste(unit));
        self
    }

    /// Types `character` into the file shown as its language pairs it; a closing bracket typed where the line holds nothing else goes back a level of indentation.
    pub fn editor_typed(mut self, character: char) -> Self {
        if self
            .shown_document()
            .is_some_and(|document| document.field.has_others())
        {
            self.file_edited(Edit::Insert(character));
            return self;
        }
        let (close, surround) = (self.editor.auto_close, self.editor.auto_surround);
        if let Some(edit) = self
            .shown_document()
            .and_then(|document| document.typed_edit_with(character, close, surround))
        {
            self.file_edited(edit);
            return self;
        }
        self.file_edited(Edit::Insert(character));
        let edits = self
            .shown_document()
            .map(|document| document.outdent_edits(character))
            .unwrap_or_default();
        self.file_edits(edits);
        self
    }

    /// Asks the language server what could be written at the caret of the file shown, the list hanging at `at`.
    pub fn completion_requested(mut self, at: (f32, f32)) -> Self {
        let Some((session, viewer)) = self.shown_mut() else {
            return self;
        };
        let path = viewer.path.clone();
        let Some(document) = document_of(viewer).filter(|document| document.synced) else {
            return self;
        };
        let (version, position) = document.open_completion(at);
        self.outbox.push(Outgoing::Completion {
            session,
            path,
            version,
            position,
            trigger: None,
        });
        self
    }

    /// Notes where the list of completions hangs, in logical pixels of the window.
    pub fn with_completion_at(mut self, at: (f32, f32)) -> Self {
        if let Some((_, viewer)) = self.shown_mut()
            && let Some(document) = document_of(viewer)
            && let Some(completion) = &mut document.completion
        {
            completion.at = at;
        }
        self
    }

    /// The state after the language server said what could be written in the file at `path` of the session at `session`, as asked at `version`.
    pub fn with_completions(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        items: Vec<Completion>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_completions(version, items);
        }
        self
    }

    /// Whether the list of completions is open over the file shown.
    pub fn completion_open(&self) -> bool {
        self.shown_document()
            .is_some_and(|document| document.completion.is_some())
    }

    /// Moves the pick of the open list of completions `delta` rows down, upwards when negative, wrapping around.
    pub fn completion_moved(mut self, delta: isize) -> Self {
        if let Some((_, viewer)) = self.shown_mut()
            && let Some(document) = document_of(viewer)
        {
            document.move_completion(delta);
        }
        self
    }

    /// Closes the list of completions over the file shown.
    pub fn completion_closed(mut self) -> Self {
        if let Some((_, viewer)) = self.shown_mut()
            && let Some(document) = document_of(viewer)
        {
            document.completion = None;
        }
        self
    }

    /// Writes the completion picked, or the one at `row` of the list when given, in place of the word it completes, and closes the list.
    pub fn completion_taken(mut self, row: Option<usize>) -> Self {
        let edits = self
            .shown_document()
            .and_then(|document| document.completion_edits(row));
        self = self.completion_closed();
        self.file_edits(edits.unwrap_or_default());
        self.completion_closed()
    }

    /// Shows what is known about the byte `offset` of the file shown, on a card at `at`: what the language server finds wrong there at once, and asks it what it says about the place.
    pub fn hover_requested(mut self, offset: usize, at: (f32, f32)) -> Self {
        let Some((session, viewer)) = self.shown_mut() else {
            return self;
        };
        let path = viewer.path.clone();
        let Some(document) = document_of(viewer) else {
            return self;
        };
        if document
            .hover
            .as_ref()
            .is_some_and(|card| card.offset == offset)
        {
            return self;
        }
        let diagnostics = document.diagnostics_at(offset);
        if !document.synced && diagnostics.is_empty() {
            document.hover = None;
            return self;
        }
        document.hover = Some(HoverCard {
            offset,
            at,
            text: None,
            diagnostics,
            scroll: 0.0,
        });
        if document.synced {
            let request = Outgoing::Hover {
                session,
                path,
                version: document.version,
                position: document.position_of(offset),
            };
            self.outbox.push(request);
        }
        self
    }

    /// The state after the language server said what it knows about `position` of the file at `path` of the session at `session`.
    pub fn with_hover(
        mut self,
        session: usize,
        path: &str,
        position: Position,
        hover: Option<Hover>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            let wanted = document
                .hover
                .as_ref()
                .is_some_and(|card| document.position_of(card.offset) == position);
            if wanted && let Some(card) = &mut document.hover {
                card.text = hover.map(|hover| hover.text);
                if card.text.is_none() && card.diagnostics.is_empty() {
                    document.hover = None;
                }
            }
        }
        self
    }

    /// The state after the card about a place is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_hover(mut self, delta: f32, room: f32) -> Self {
        if let Some((_, viewer)) = self.shown_mut()
            && let Some(document) = document_of(viewer)
            && let Some(card) = &mut document.hover
        {
            card.scroll = (card.scroll + delta).clamp(0.0, room);
        }
        self
    }

    /// Asks the language server to format the file shown, and writes it to the folder afterwards when `save`; a file no language server reads is written as it is.
    pub(crate) fn format_then_save(&mut self, save: bool) {
        let Some((session, viewer)) = self.shown_mut() else {
            return;
        };
        let path = viewer.path.clone();
        let asking = document_of(viewer)
            .filter(|document| document.synced && !document.field.is_read_only())
            .map(|document| (document.version, document.language.clone()));
        let formats =
            |language: &Option<String>| !save || self.formats_on_save(language.as_deref());
        match asking
            .filter(|(_, language)| formats(language))
            .map(|(version, _)| version)
        {
            Some(version) => self.outbox.push(Outgoing::Format {
                session,
                path,
                version,
                save,
            }),
            None if save => self.save_file(),
            None => {}
        }
    }

    /// The state with the file shown formatted as its language says, without writing it.
    pub fn formatted_by_hand(mut self) -> Self {
        self.format_then_save(false);
        self
    }

    /// The state after the language server gave `edits` that format the file at `path` of the session at `session` as it was at `version`: they are made when the file shown is still that file at that version, and the file is written to the folder when `save`.
    pub fn with_formatted(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        edits: Vec<pm_protocol::TextChange>,
        save: bool,
    ) -> Self {
        let current = self.review_index() == Some(session)
            && self
                .shown_viewer()
                .is_some_and(|viewer| viewer.path == path && !viewer.head);
        if !current {
            return self;
        }
        let edits = self
            .shown_document()
            .map(|document| document.format_edits(version, edits))
            .unwrap_or_default();
        self.file_edits(edits);
        if save {
            self.save_file();
        }
        self
    }

    /// Notes the word of the file shown that the pointer is on while Ctrl is held, the stretch `reach`, or that there is none.
    pub fn with_reach(mut self, reach: Option<std::ops::Range<usize>>) -> Self {
        if let Some((_, viewer)) = self.shown_mut()
            && let Some(document) = document_of(viewer)
        {
            document.reach = reach;
        }
        self
    }

    /// Shows `location`, a place in a file of the session at `session`: in the file open already, in a tab of its own for another file of the project, or read only for a file outside it.
    pub(crate) fn go_to_location(
        mut self,
        session: usize,
        location: pm_protocol::Location,
    ) -> Self {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        if self.review_index() != Some(session) {
            return self;
        }
        self.remember_spot();
        if let Some(index) = self
            .git
            .viewers
            .iter()
            .position(|viewer| viewer.path == location.path && !viewer.head)
        {
            self.git.shown = Some(index);
            self.tab = crate::state::Tab::File;
            if let Some(viewer) = self.git.viewers.get_mut(index) {
                match document_of(viewer) {
                    Some(document) => {
                        let scroll =
                            crate::open_files::jump(document, location.span.start, row, columns);
                        viewer.scroll = scroll;
                    }
                    None => viewer.jump = Some(location.span.start),
                }
            }
            return self;
        }
        if location.inside {
            self.open_viewer(location.path.clone(), false);
            if let Some(viewer) = self
                .git
                .shown
                .and_then(|index| self.git.viewers.get_mut(index))
            {
                viewer.jump = Some(location.span.start);
            }
            return self;
        }
        let Some(text) = location.text else {
            return self;
        };
        let mut document = Document::new(&text, true, false);
        let scroll = crate::open_files::jump(&mut document, location.span.start, row, columns);
        let viewer = crate::open_files::Viewer {
            path: location.path.clone(),
            head: false,
            contents: Contents::Text(Box::new(document)),
            scroll,
            preview: true,
            left: None,
            jump: None,
            pinned: false,
        };
        let index = match self.git.viewers.iter().position(|held| held.preview) {
            Some(index) => {
                let replaced = std::mem::replace(&mut self.git.viewers[index], viewer);
                self.close_document(session, &replaced);
                index
            }
            None => {
                self.git.viewers.push(viewer);
                self.git.viewers.len() - 1
            }
        };
        self.git.shown = Some(index);
        self.tab = crate::state::Tab::File;
        self.took_language(session, &location.path, false, location.language);
        self
    }

    /// Takes away what is said about a place of the file shown.
    pub fn hover_closed(mut self) -> Self {
        if let Some((_, viewer)) = self.shown_mut()
            && let Some(document) = document_of(viewer)
        {
            document.hover = None;
        }
        self
    }

    /// Whether a card about a place of the file shown is showing.
    pub fn hover_open(&self) -> bool {
        self.shown_document()
            .is_some_and(|document| document.hover.is_some())
    }
}

/// Applies `edit` to the text of `viewer`, the file at its path of the session at `session`, keeping its syntax tree and the completions open at its caret in step; returns what its language server is to be told and asked. Changing its text keeps its tab open.
fn edit_viewer(
    session: usize,
    viewer: &mut Viewer,
    edit: Edit,
    settings: &pm_editor::EditorSettings,
) -> Vec<Outgoing> {
    let path = viewer.path.clone();
    let Contents::Text(document) = &mut viewer.contents else {
        return Vec::new();
    };
    let Some(edited) = document.edit(edit) else {
        return Vec::new();
    };
    let position = document.position_of(document.field.caret());
    let version = document.version;
    viewer.preview = false;
    let mut told = Vec::new();
    if let Some((version, change)) = edited.change {
        told.push(Outgoing::ChangeDocument {
            session,
            path: path.clone(),
            version,
            changes: vec![change],
        });
    }
    if !settings.completions_on_input && edited.ask.is_some() {
        document.completion = None;
    }
    if edited.signature && settings.signature_help {
        told.push(Outgoing::Signature {
            session,
            path: path.clone(),
            version,
            position,
        });
    }
    if let Some(trigger) = edited.ask.filter(|_| settings.completions_on_input) {
        told.push(Outgoing::Completion {
            session,
            path,
            version,
            position,
            trigger,
        });
    }
    told
}
