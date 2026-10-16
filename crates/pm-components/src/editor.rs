//! Writing code in the file shown: edits kept in step with the syntax tree and the language server, new lines indented as the language says, completions at the caret and what the server says about a place.

use pm_buffer::Lines;
use pm_protocol::{Completion, Diagnostic, Hover, Position};
use pm_text::Edit;
use pm_treesitter::Syntax;

use crate::document::{Completions, Document, HoverCard};
use crate::open_files::{Contents, Viewer};
use crate::state::{Outgoing, WorkspaceState};

/// How many completions the list shows at once.
pub const COMPLETIONS_SHOWN: usize = 10;

/// Whether `character` is part of a word a completion finishes.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// The positions in `items` of the completions that match `typed`, best first.
fn matching(items: &[Completion], typed: &str) -> Vec<usize> {
    let mut scored: Vec<(u8, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(place, item)| pm_lsp::score(&item.filter, typed).map(|held| (held, place)))
        .collect();
    scored.sort();
    scored.into_iter().map(|(_, place)| place).collect()
}

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
    fn every_viewer(&mut self) -> Vec<(usize, &mut Viewer)> {
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
    fn file_mut(&mut self, session: usize, path: &str) -> Option<&mut Document> {
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
    }

    /// The state after the server of `host` said how its languages stand: an open file of a project on that machine that was read before the language it is written in was installed there takes that language now.
    pub fn with_host_extensions(
        mut self,
        host: &Option<String>,
        languages: &[pm_protocol::ExtensionInfo],
    ) -> Self {
        crate::file_icon::learn_icons(host, languages);
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

    /// The state after the language server opened the file at `path` of the session at `session`, offering completions after `triggers`, or could not, for `problem`.
    pub fn with_opened(
        mut self,
        session: usize,
        path: &str,
        outcome: Result<Vec<String>, String>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            match outcome {
                Ok(triggers) => {
                    document.triggers = triggers;
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
        let Some((session, viewer)) = self.shown_mut() else {
            return;
        };
        let path = viewer.path.clone();
        let Contents::Text(document) = &mut viewer.contents else {
            return;
        };
        let before = document.field.text().to_owned();
        let before_lines: Lines = document.line_index().clone();
        document.field.apply(edit);
        let Some(change) = document.refresh(&before, &before_lines) else {
            refilter(document, false);
            return;
        };
        viewer.preview = false;
        let Contents::Text(document) = &mut viewer.contents else {
            return;
        };
        document.hover = None;
        let typed: Option<char> = (change.range.is_empty() && change.text.chars().count() == 1)
            .then(|| change.text.chars().next())
            .flatten();
        let mut outgoing = Vec::new();
        if document.synced {
            document.version += 1;
            outgoing.push(Outgoing::ChangeDocument {
                session,
                path: path.clone(),
                version: document.version,
                changes: vec![Document::text_change(&change, &before, &before_lines)],
            });
        }
        let trigger = typed.filter(|character| {
            document
                .triggers
                .iter()
                .any(|held| held.ends_with(*character))
        });
        let reopen = refilter(document, trigger.is_some());
        let asking = match (trigger, reopen) {
            (Some(character), _) => Some(Some(character.to_string())),
            (None, true) => Some(None),
            (None, false) => None,
        };
        if let Some(trigger) = asking
            && document.synced
        {
            let caret = document.field.caret();
            let at = document
                .completion
                .as_ref()
                .map_or((0.0, 0.0), |held| held.at);
            document.completion = Some(Completions {
                start: document.word_start(caret),
                items: Vec::new(),
                shown: Vec::new(),
                picked: 0,
                at,
                asked: document.version,
                loading: true,
            });
            outgoing.push(Outgoing::Completion {
                session,
                path,
                version: document.version,
                position: document.position_of(caret),
                trigger,
            });
        }
        self.outbox.extend(outgoing);
    }

    /// Breaks the line at the caret of the file shown, indenting the new line as the language says: one level deeper inside a block opened on the line, and the closing of that block put on a line of its own.
    pub fn editor_newline(mut self) -> Self {
        let Some(document) = self.shown_document() else {
            return self;
        };
        let caret = document.field.caret();
        let text = document.field.text();
        let base = document.indentation_at(caret);
        let unit = document.editing.unit(1);
        let indent = document
            .syntax
            .as_ref()
            .map(|syntax| syntax.indent_at(text, document.line_index(), caret))
            .unwrap_or_default();
        let before = text[document.line(document.line_of(caret)).start..caret].trim_end();
        let opens = before.ends_with(['{', '(', '[', ':']);
        let deeper = indent.deeper || opens;
        let closing = deeper
            && text[caret..]
                .trim_start_matches([' ', '\t'])
                .starts_with(['}', ')', ']']);
        let inner = if deeper {
            format!("{base}{unit}")
        } else {
            base.clone()
        };
        self.file_edited(Edit::Paste(format!("\n{inner}")));
        if closing {
            let at = self
                .shown_document()
                .map_or(0, |document| document.field.caret());
            self.file_edited(Edit::Paste(format!("\n{base}")));
            self.file_edited(Edit::Place(at, false));
        }
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

    /// Types `character` into the file shown; a closing bracket typed where the line holds nothing else goes back a level of indentation.
    pub fn editor_typed(mut self, character: char) -> Self {
        self.file_edited(Edit::Insert(character));
        let Some(document) = self.shown_document() else {
            return self;
        };
        if !matches!(character, '}' | ')' | ']') {
            return self;
        }
        let caret = document.field.caret();
        let at = caret.saturating_sub(character.len_utf8());
        let line = document.line(document.line_of(at));
        let leading = &document.field.text()[line.start..at];
        let closes = document
            .syntax
            .as_ref()
            .is_some_and(|syntax| syntax.closes_at(document.field.text(), at));
        if !closes || leading.is_empty() || !leading.chars().all(|c| c == ' ' || c == '\t') {
            return self;
        }
        let unit = document.editing.unit(1);
        let Some(shorter) = leading.strip_suffix(&unit) else {
            return self;
        };
        let (start, end) = (line.start + shorter.len(), at);
        self.file_edited(Edit::Place(start, false));
        self.file_edited(Edit::Place(end, true));
        self.file_edited(Edit::Backspace);
        let caret = self
            .shown_document()
            .map_or(0, |document| document.field.caret());
        self.file_edited(Edit::Place(caret + character.len_utf8(), false));
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
        let caret = document.field.caret();
        document.completion = Some(Completions {
            start: document.word_start(caret),
            items: Vec::new(),
            shown: Vec::new(),
            picked: 0,
            at,
            asked: document.version,
            loading: true,
        });
        let request = Outgoing::Completion {
            session,
            path,
            version: document.version,
            position: document.position_of(caret),
            trigger: None,
        };
        self.outbox.push(request);
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
        if let Some(document) = self.file_mut(session, path)
            && let Some(completion) = &mut document.completion
            && completion.asked == version
        {
            completion.items = items;
            completion.loading = false;
            refilter(document, false);
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
            && let Some(completion) = &mut document.completion
            && !completion.shown.is_empty()
        {
            let count = completion.shown.len() as isize;
            completion.picked = (completion.picked as isize + delta).rem_euclid(count) as usize;
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
        let Some(document) = self.shown_document() else {
            return self;
        };
        let Some(completion) = &document.completion else {
            return self;
        };
        let Some(item) = completion
            .shown
            .get(row.unwrap_or(completion.picked))
            .and_then(|place| completion.items.get(*place))
            .cloned()
        else {
            return self.completion_closed();
        };
        let caret = document.field.caret();
        let (start, end) = match item.span {
            Some(span) => {
                let range = document.range_of(span);
                (range.start.min(caret), range.end.max(caret))
            }
            None => (completion.start.min(caret), caret),
        };
        self = self.completion_closed();
        self.file_edited(Edit::Place(start, false));
        self.file_edited(Edit::Place(end, true));
        self.file_edited(Edit::Paste(item.text));
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

/// Narrows the completions open over `document` to the word typed before its caret, and closes the list when the caret left the word or nothing matches; says whether a fresh list should be asked for in place of the one closed, as when the reader went on typing a word after the list emptied.
fn refilter(document: &mut Document, triggered: bool) -> bool {
    let caret = document.field.caret();
    let text = document.field.text();
    let Some(completion) = &mut document.completion else {
        let typed_word = text[..caret].chars().next_back().is_some_and(is_word);
        return !triggered
            && typed_word
            && caret - document.word_start(caret) == 1
            && document.synced;
    };
    let in_word = caret >= completion.start
        && text
            .get(completion.start..caret)
            .is_some_and(|typed| typed.chars().all(is_word));
    if !in_word {
        document.completion = None;
        return false;
    }
    if completion.loading {
        return false;
    }
    let typed = &text[completion.start..caret];
    completion.shown = matching(&completion.items, typed);
    completion.picked = completion
        .picked
        .min(completion.shown.len().saturating_sub(1));
    if completion.shown.is_empty() {
        document.completion = None;
    }
    false
}
