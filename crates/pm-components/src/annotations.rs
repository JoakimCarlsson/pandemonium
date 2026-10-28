//! What the language server says to show in and beside the lines of the file shown: names coloured by what they are, hints written inside a line, and lenses at the end of one.

use pm_input::FocusId;
use pm_protocol::{CodeLens, FileEdit, InlayHint, Location, Seek, SemanticToken};

use crate::listing::{Items, Listing};
use crate::state::{Outgoing, WorkspaceState};

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
        if names && document.wants_semantic_tokens() {
            document.semantic_tokens_asked(version);
            asking.push(Outgoing::SemanticTokens {
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
