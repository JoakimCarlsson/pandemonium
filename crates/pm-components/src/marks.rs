//! What is marked over the file shown besides its text: where the thing at the caret is used, marked as the caret settles, the signature of the call the caret is in, shown as it is typed, and how its lines differ from the last commit.

use pm_protocol::{Signature, Span};

use crate::state::{Outgoing, WorkspaceState};

impl WorkspaceState {
    /// Asks the language server where the thing at the caret of the file shown is used in it, unless it was asked for this place of this version already.
    pub fn highlights_requested(mut self) -> Self {
        if !self.editor.occurrence_highlights {
            return self;
        }
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let Some(document) = self.shown_document().filter(|document| document.synced) else {
            return self;
        };
        let request = Outgoing::Highlights {
            session,
            path,
            version: document.version,
            position: document.position_of(document.field.caret()),
        };
        self.outbox.push(request);
        self
    }

    /// Asks the language server which blocks of the file shown can be folded, unless the text it was asked about is the text as it is.
    pub fn folds_requested(mut self) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let Some(document) = self
            .shown_document()
            .filter(|document| document.synced && document.folds_supported)
            .filter(|document| document.server_folds_version() != Some(document.version))
        else {
            return self;
        };
        let request = Outgoing::FoldingRanges {
            session,
            path,
            version: document.version,
        };
        self.outbox.push(request);
        self
    }

    /// The state after the language server said which blocks of the file at `path` of the session at `session` can be folded, as the text was at `version`.
    pub fn with_folds(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        ranges: Vec<pm_protocol::FoldRange>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_folds(version, ranges);
        }
        self
    }

    /// Asks the language server which stretches are edited together with the one the caret of the file shown is in, when the setting is on and the caret is not in one already.
    pub fn links_requested(mut self) -> Self {
        if !self.editor.linked_editing {
            return self;
        }
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let Some(document) = self
            .shown_document()
            .filter(|document| document.synced && document.links_supported)
            .filter(|document| !document.field.is_read_only() && !document.is_linked())
        else {
            return self;
        };
        let request = Outgoing::LinkedRanges {
            session,
            path,
            version: document.version,
            position: document.position_of(document.field.caret()),
        };
        self.outbox.push(request);
        self
    }

    /// The state after the language server said which stretches of the file at `path` of the session at `session` are edited together with the one the caret was in, as the text was at `version`.
    pub fn with_links(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        spans: Vec<Span>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_linked(version, spans);
        }
        self
    }

    /// The state after the language server said where the thing at a place of the file at `path` of the session at `session` is used, as the text was at `version`.
    pub fn with_highlights(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        spans: Vec<Span>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_highlights(version, spans);
        }
        self
    }

    /// The state after the language server said what the call a place of the file at `path` of the session at `session` is in looks like, as the text was at `version`.
    pub fn with_signature(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        signature: Option<Signature>,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_signature(version, signature);
        }
        self
    }

    /// Asks the language server what the call the caret of the file shown is in looks like.
    pub fn signature_requested(mut self) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self.shown_viewer().map(|viewer| viewer.path.clone()) else {
            return self;
        };
        let Some(document) = self.shown_document().filter(|document| document.synced) else {
            return self;
        };
        let request = Outgoing::Signature {
            session,
            path,
            version: document.version,
            position: document.position_of(document.field.caret()),
        };
        self.outbox.push(request);
        self
    }

    /// Whether the signature of a call shows over the file shown.
    pub fn signature_open(&self) -> bool {
        self.shown_document()
            .is_some_and(|document| document.signature.is_some())
    }

    /// Takes away the signature of the call showing over the file shown.
    pub fn signature_closed(mut self) -> Self {
        if let Some(document) = self.shown_text() {
            document.signature = None;
        }
        self
    }

    /// Notes where the signature of the call the caret is in hangs, in logical pixels of the window.
    pub fn with_signature_at(mut self, at: (f32, f32)) -> Self {
        self.signature_at = at;
        self
    }

    /// The state after the file at `path` of the session at `session` was read as the last commit holds it, `base`, or found to be in no commit.
    pub fn with_base(mut self, session: usize, path: &str, base: Option<String>) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.set_base(base);
        }
        self
    }

    /// Reads again, as the last commit holds them, the files of the session at `session` that are open, since the last commit may have changed.
    pub fn bases_wanted(mut self, session: usize) -> Self {
        let paths: Vec<String> = self
            .every_viewer()
            .into_iter()
            .filter(|(owner, viewer)| *owner == session && !viewer.head)
            .filter(|(_, viewer)| matches!(viewer.contents, crate::open_files::Contents::Text(_)))
            .map(|(_, viewer)| viewer.path.clone())
            .collect();
        for path in paths {
            self.outbox.push(Outgoing::ReadBase { session, path });
        }
        self
    }

    /// Reads again the files of the session at `session` that are open as the folder holds them and show text without unsaved changes, since something other than this window, such as an agent, may have changed them.
    pub fn open_files_wanted(mut self, session: usize) -> Self {
        let paths: Vec<String> = self
            .every_viewer()
            .into_iter()
            .filter(|(owner, viewer)| *owner == session && !viewer.head && !viewer.is_dirty())
            .filter(|(_, viewer)| matches!(viewer.contents, crate::open_files::Contents::Text(_)))
            .map(|(_, viewer)| viewer.path.clone())
            .collect();
        for path in paths {
            self.outbox.push(Outgoing::ReadFile {
                session,
                path,
                head: false,
            });
        }
        self
    }

    /// Whether the caret's line says who last changed it: as the editor's setting says, unless git's settings turn it off.
    pub(crate) fn shows_blame(&self) -> bool {
        self.window.git.blame.shows(self.editor.inline_blame)
    }

    /// Whether lines changed since the last commit are marked: as the editor's setting says, unless git's settings turn it off.
    pub(crate) fn shows_gutter(&self) -> bool {
        self.window.git.gutter.shows(self.editor.git_gutter)
    }

    /// Asks who last changed the line of the caret of the file shown, when the file is as the folder holds it.
    pub fn blame_requested(mut self) -> Self {
        if !self.shows_blame() {
            return self;
        }
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self
            .shown_viewer()
            .filter(|viewer| !viewer.head)
            .map(|viewer| viewer.path.clone())
        else {
            return self;
        };
        let Some(document) = self
            .shown_document()
            .filter(|document| !document.is_dirty() && !document.field.is_read_only())
        else {
            return self;
        };
        let line = document.line_of(document.field.caret()) + 1;
        let known = self
            .blame
            .as_ref()
            .is_some_and(|(held, file, at, _)| *held == session && *file == path && *at == line);
        if !known {
            self.outbox.push(Outgoing::Blame {
                session,
                path,
                line,
            });
        }
        self
    }

    /// The state after the server said who last changed line `line` of the file at `path` of the session at `session`.
    pub fn with_blame(
        mut self,
        session: usize,
        path: &str,
        line: usize,
        blame: pm_git::Blame,
    ) -> Self {
        self.blame = Some((session, path.to_owned(), line, blame));
        self
    }

    /// What is said after the line at `index` of the file shown about who last changed it: for the line of the caret, when the file is as the folder holds it and the answer is about it.
    pub fn blame_of_line(&self, index: usize) -> Option<String> {
        if !self.shows_blame() {
            return None;
        }
        let session = self.review_index()?;
        let viewer = self.shown_viewer()?;
        let document = self.shown_document()?;
        if document.is_dirty() || document.line_of(document.field.caret()) != index {
            return None;
        }
        let (held, path, line, blame) = self.blame.as_ref()?;
        if *held != session || *path != viewer.path || *line != index + 1 || blame.id.is_empty() {
            return None;
        }
        let ago = crate::age::label(blame.when.max(0) as u64 * 1000, crate::age::now());
        Some(format!("{}, {ago} • {}", blame.author, blame.summary))
    }
}
