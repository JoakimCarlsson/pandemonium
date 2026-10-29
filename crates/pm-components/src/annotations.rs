//! What the language server says to show in and beside the lines of the file shown: names coloured by what they are, hints written inside a line, and lenses at the end of one.

use pm_input::FocusId;
use pm_protocol::{CodeLens, FileEdit, InlayHint, Location, Seek, SemanticToken};

use crate::listing::{Items, Listing};
use crate::state::{Outgoing, WorkspaceState};

/// How tall a view is taken to be, in logical pixels, until the one drawn has been measured.
const VIEW_HEIGHT: f32 = 1200.0;

/// The first of the marks standing for lenses, each following it by the place of the lens in the list the server gave.
pub const LENS_BASE: u64 = 8_600_000;

/// One past the last mark standing for lenses.
pub const LENS_END: u64 = 8_700_000;

/// The mark that presses the lens at `index` of the list the server gave.
pub fn lens_mark(index: usize) -> FocusId {
    FocusId(LENS_BASE + index as u64)
}

/// The lens, by its place in the list the server gave, that the mark `id` presses.
pub fn lens_of(id: FocusId) -> Option<usize> {
    (LENS_BASE..LENS_END)
        .contains(&id.0)
        .then(|| (id.0 - LENS_BASE) as usize)
}

impl WorkspaceState {
    /// Asks the language server for what it says to show in and beside the lines of the file shown, whatever it was not asked or did not answer for the text as it is, and lets go of what the reader turned off.
    pub fn annotations_requested(mut self) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let (names, hints, lenses) = (
            self.editor.semantic_highlighting,
            self.editor.inlay_hints,
            self.editor.code_lens,
        );
        let view = self.names_view();
        let Some(document) = self.shown_text() else {
            return self;
        };
        if !names {
            document.forget_semantic_tokens();
        }
        if !hints {
            document.forget_inlay_hints();
        }
        if !lenses {
            document.forget_code_lenses();
        }
        let version = document.version;
        let mut asking = Vec::new();
        if let Some(lines) = view
            && let Some(span) = document.semantic_range_wanted(lines.clone())
        {
            document.semantic_range_asked(version, lines);
            asking.push(Outgoing::SemanticTokens {
                session,
                path: path.clone(),
                version,
                range: Some(span),
            });
        }
        if names && document.wants_semantic_tokens() {
            document.semantic_tokens_asked(version);
            asking.push(Outgoing::SemanticTokens {
                session,
                path: path.clone(),
                version,
                range: None,
            });
        }
        if document.wants_document_links() {
            document.document_links_asked(version);
            asking.push(Outgoing::DocumentLinks {
                session,
                path: path.clone(),
                version,
            });
        }
        if document.wants_colors() {
            document.colors_asked(version);
            asking.push(Outgoing::Colors {
                session,
                path: path.clone(),
                version,
            });
        }
        if hints && document.wants_inlay_hints() {
            document.inlay_hints_asked(version);
            asking.push(Outgoing::InlayHints {
                session,
                path: path.clone(),
                version,
                span: document.whole_span(),
            });
        }
        if lenses && document.wants_code_lenses() {
            document.code_lenses_asked(version);
            asking.push(Outgoing::CodeLenses {
                session,
                path,
                version,
                dependencies: self.editor.dependency_hints,
            });
        }
        self.outbox.extend(asking);
        self.inline_values_requested()
    }

    /// The lines of the file shown, with the margin the settings give above and below, to ask the language server the names of first, when the file is long enough for the settings to ask for the lines in view before the rest and its names are being asked for.
    fn names_view(&self) -> Option<std::ops::Range<usize>> {
        let threshold = self.editor.semantic_range_lines;
        let document = self.shown_document()?;
        if !self.editor.semantic_highlighting || threshold == 0 || document.lines() < threshold {
            return None;
        }
        let row = self.editor.row_height().max(1.0);
        let scroll = self.shown_viewer()?.scroll;
        let height = self.code_height.unwrap_or(VIEW_HEIGHT);
        let first = (scroll / row).floor().max(0.0) as usize;
        let count = (height / row).ceil() as usize + 1;
        let (lines, _) = document.wrapped_lines(first, count, self.wrap_columns());
        let margin = self.editor.semantic_range_margin;
        Some(lines.first()?.saturating_sub(margin)..lines.last()? + 1 + margin)
    }

    /// Whether the language server should be asked for the names of the lines in view alone: the file is long, its names are not all held and the lines in view are not covered by what is.
    pub fn names_in_view_wanted(&self) -> bool {
        let Some(lines) = self.names_view() else {
            return false;
        };
        self.shown_document()
            .is_some_and(|document| document.semantic_range_wanted(lines).is_some())
    }

    /// The state knowing the view of the file shown is `height` logical pixels tall.
    pub fn with_code_height(mut self, height: f32) -> Self {
        self.code_height = Some(height);
        self
    }

    /// The state after the language server said what each name within the stretch `range` of the file at `path` of the session at `session` is, as the text was at `version`.
    pub fn with_semantic_range(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        range: pm_protocol::Span,
        tokens: Vec<SemanticToken>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_semantic_range(version, range, tokens);
        }
        self
    }

    /// The state after the language server said what each name of the file at `path` of the session at `session` is, as the text was at `version`.
    pub fn with_semantic_tokens(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        tokens: Vec<SemanticToken>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_semantic_tokens(version, tokens);
        }
        self
    }

    /// The state after the language server said which stretches of the file at `path` of the session at `session` lead somewhere when pressed, as the text was at `version`.
    pub fn with_document_links(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        links: Vec<pm_protocol::DocumentLink>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_document_links(version, links);
        }
        self
    }

    /// The state after the language server said which stretches of the file at `path` of the session at `session` spell a colour, as the text was at `version`.
    pub fn with_colors(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        colors: Vec<pm_protocol::ColorSwatch>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_colors(version, colors);
        }
        self
    }

    /// The state after the language server said what to show inside the lines of the file at `path` of the session at `session`, as the text was at `version`.
    pub fn with_inlay_hints(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        hints: Vec<InlayHint>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_inlay_hints(version, hints);
        }
        self
    }

    /// The state after the language server said what to show at the end of lines of the file at `path` of the session at `session`, as the text was at `version`.
    pub fn with_code_lenses(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        lenses: Vec<CodeLens>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_code_lenses(version, lenses);
        }
        self
    }

    /// Presses the lens at `index` of the list the language server gave for the file shown, what it points to being listed at `at`.
    pub fn lens_pressed(mut self, index: usize, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let Some(title) = self
            .shown_document()
            .and_then(|document| document.lens_titled(index).map(str::to_owned))
        else {
            return self;
        };
        self.listing = Some(Listing::waiting(
            session,
            Items::Places {
                seek: Seek::References,
                word: title,
                locations: Vec::new(),
            },
            at,
        ));
        self.outbox.push(Outgoing::PressCodeLens {
            session,
            path,
            index,
        });
        self
    }

    /// The state after pressing a lens of a file of the session at `session` came to showing `places`, or to the server making `files`, the changes to the open files.
    pub fn with_lens_pressed(
        mut self,
        session: usize,
        places: Vec<Location>,
        files: Vec<FileEdit>,
    ) -> Self {
        if !files.is_empty() {
            self.listing = None;
            return self.with_edited_files(session, files);
        }
        self.with_places(session, Seek::References, places)
    }
}
