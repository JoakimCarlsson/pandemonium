//! The marker beside the line of the caret that says the language server offers changes there, and the question that finds out.

use pm_editor::LightbulbShown;
use pm_input::FocusId;

use crate::state::{Outgoing, WorkspaceState};

/// The marker beside the line of the caret, which opens the list of changes when pressed.
pub const LIGHTBULB: FocusId = FocusId(7_402_200);

impl WorkspaceState {
    /// Asks the language server whether it offers changes at the caret or the selection of the file shown, only to mark the line; nothing when the marker is off, a list of changes is open, or the marker is for problem lines and this line has none.
    pub fn lightbulb_requested(mut self) -> Self {
        let mode = self.editor.lightbulb;
        if mode == LightbulbShown::Off || self.listing.is_some() {
            return self;
        }
        let Some((session, path)) = self.coding_file() else {
            return self;
        };
        let Some(document) = self.shown_document().filter(|document| document.synced) else {
            return self;
        };
        let caret = document.field.caret();
        let line = document.line_of(caret);
        if mode == LightbulbShown::Problems && document.diagnostics_on(line).is_empty() {
            return self;
        }
        let range = document.field.selection().unwrap_or(caret..caret);
        let request = Outgoing::Lightbulb {
            session,
            path,
            version: document.version,
            line,
            span: pm_protocol::Span::new(
                document.position_of(range.start),
                document.position_of(range.end),
            ),
        };
        self.outbox.push(request);
        self
    }

    /// The state after the language server said it offers `offered` changes at the caret of the file at `path` of the session at `session`, on `line` of the text at `version`.
    pub fn with_lightbulb(
        mut self,
        session: usize,
        path: &str,
        version: i32,
        line: usize,
        offered: usize,
    ) -> Self {
        if let Some(document) = self.file_mut(session, path) {
            document.take_bulb(version, line, offered);
        }
        self
    }

    /// The line of the file shown the marker stands beside, when it shows.
    pub fn lightbulb_line(&self) -> Option<usize> {
        let only_problems = match self.editor.lightbulb {
            LightbulbShown::Off => return None,
            LightbulbShown::CaretLine => false,
            LightbulbShown::Problems => true,
        };
        self.shown_document()?.bulb_line(only_problems)
    }
}
