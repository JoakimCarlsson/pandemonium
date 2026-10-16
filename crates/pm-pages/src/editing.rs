//! What the screen does for writing code: the file shown edited as its language says, what its language server answers, and the languages installed on this machine.

use pm_protocol::{Completion, Diagnostic, ExtensionInfo, Hover, Position};

use crate::screen::Screen;
use crate::workspace::WorkspaceState;

impl Screen {
    /// The screen with `change` made to the workspace, when the workspace is showing.
    fn in_workspace(self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(change(state)),
            onboarding => onboarding,
        }
    }

    /// Whether the workspace is showing and `test` holds of it.
    fn workspace_is(&self, test: impl FnOnce(&WorkspaceState) -> bool) -> bool {
        matches!(self, Self::Workspace(state) if test(state))
    }

    /// The screen after the grammar of `language` was loaded.
    pub fn with_grammar(self, language: &str) -> Self {
        self.in_workspace(|state| state.with_grammar(language))
    }

    /// The screen opening again for its language server every open file of a project on `host` it was told of.
    pub fn reopen_documents(self, host: &Option<String>) -> Self {
        self.in_workspace(|state| state.reopen_documents(host))
    }

    /// The screen asking for the grammars of the languages named `names`.
    pub fn grammars_wanted(self, names: Vec<String>) -> Self {
        self.in_workspace(|state| state.grammars_wanted(names))
    }

    /// The screen after the language server opened the file at `path` of the session at `session`, or could not.
    pub fn with_opened(
        self,
        session: usize,
        path: &str,
        outcome: Result<Vec<String>, String>,
    ) -> Self {
        self.in_workspace(|state| state.with_opened(session, path, outcome))
    }

    /// The screen after the language server of `host` said what it finds wrong with the file at `path` of the folder `directory`.
    pub fn with_diagnostics(
        self,
        host: &Option<String>,
        directory: &str,
        path: &str,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        self.in_workspace(|state| state.with_diagnostics(host, directory, path, diagnostics))
    }

    /// The screen after the language server said what could be written in the file at `path` of the session at `session`.
    pub fn with_completions(
        self,
        session: usize,
        path: &str,
        version: i32,
        items: Vec<Completion>,
    ) -> Self {
        self.in_workspace(|state| state.with_completions(session, path, version, items))
    }

    /// The screen after the language server said what it knows about `position` of the file at `path` of the session at `session`.
    pub fn with_hover(
        self,
        session: usize,
        path: &str,
        position: Position,
        hover: Option<Hover>,
    ) -> Self {
        self.in_workspace(|state| state.with_hover(session, path, position, hover))
    }

    /// The screen asking for what could be written at the caret of the file shown, the list hanging at `at`.
    pub fn completion_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.completion_requested(at))
    }

    /// The screen with the list of completions hanging at `at`.
    pub fn with_completion_at(self, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.with_completion_at(at))
    }

    /// Whether the list of completions is open over the file shown.
    pub fn completion_open(&self) -> bool {
        self.workspace_is(WorkspaceState::completion_open)
    }

    /// The screen with the pick of the list of completions moved `delta` rows.
    pub fn completion_moved(self, delta: isize) -> Self {
        self.in_workspace(|state| state.completion_moved(delta))
    }

    /// The screen with the list of completions closed.
    pub fn completion_closed(self) -> Self {
        self.in_workspace(WorkspaceState::completion_closed)
    }

    /// The screen with the completion picked written in.
    pub fn completion_taken(self) -> Self {
        self.in_workspace(|state| state.completion_taken(None))
    }

    /// The screen showing what is known about the byte `offset` of the file shown, on a card at `at`.
    pub fn hover_requested(self, offset: usize, at: (f32, f32)) -> Self {
        self.in_workspace(|state| state.hover_requested(offset, at))
    }

    /// The screen after the card about a place is scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_hover(self, delta: f32, room: f32) -> Self {
        self.in_workspace(|state| state.scrolled_hover(delta, room))
    }

    /// The screen gone back to where the caret was before the last jump.
    pub fn jumped_back(self) -> Self {
        self.in_workspace(WorkspaceState::jumped_back)
    }

    /// The screen gone forward again to where the caret was before the last step back.
    pub fn jumped_forward(self) -> Self {
        self.in_workspace(WorkspaceState::jumped_forward)
    }

    /// The screen with the file shown formatted as its language says, without writing it.
    pub fn formatted_by_hand(self) -> Self {
        self.in_workspace(WorkspaceState::formatted_by_hand)
    }

    /// The screen after the language server gave the edits that format a file.
    pub fn with_formatted(
        self,
        session: usize,
        path: &str,
        version: i32,
        edits: Vec<pm_protocol::TextChange>,
        save: bool,
    ) -> Self {
        self.in_workspace(|state| state.with_formatted(session, path, version, edits, save))
    }

    /// The screen noting the word of the file shown the pointer is on while Ctrl is held.
    pub fn with_reach(self, reach: Option<std::ops::Range<usize>>) -> Self {
        self.in_workspace(|state| state.with_reach(reach))
    }

    /// The screen asking where the thing at the byte `offset` of the file shown is defined.
    pub fn definition_requested(self, offset: usize) -> Self {
        self.in_workspace(|state| state.definition_requested(offset))
    }

    /// The screen after the language server said where something in a file of the session at `session` is defined.
    pub fn with_definitions(self, session: usize, locations: Vec<pm_protocol::Location>) -> Self {
        self.in_workspace(|state| state.with_definitions(session, locations))
    }

    /// The screen with the card about a place taken away.
    pub fn hover_closed(self) -> Self {
        self.in_workspace(WorkspaceState::hover_closed)
    }

    /// Whether a card about a place of the file shown is showing.
    pub fn hover_open(&self) -> bool {
        self.workspace_is(WorkspaceState::hover_open)
    }

    /// The screen with the line broken at the caret of the file shown, indented as its language says.
    pub fn editor_newline(self) -> Self {
        self.in_workspace(WorkspaceState::editor_newline)
    }

    /// The screen with one level of indentation typed at the caret of the file shown.
    pub fn editor_indent(self) -> Self {
        self.in_workspace(WorkspaceState::editor_indent)
    }

    /// The screen with `character` typed into the file shown.
    pub fn editor_typed(self, character: char) -> Self {
        self.in_workspace(|state| state.editor_typed(character))
    }

    /// The screen after the reader chose the folder at `path` to install as an extension.
    pub fn extension_picked(self, path: String) -> Self {
        self.in_workspace(|state| state.extension_picked(path))
    }

    /// The screen after the extensions page is scrolled by `delta` logical pixels, with `room` pixels to scroll.
    pub fn scrolled_extensions(self, delta: f32, room: f32) -> Self {
        self.in_workspace(|state| state.scrolled_extensions(delta, room))
    }

    /// The screen after the server of `host` said how its languages stand.
    pub fn with_host_extensions(self, host: &Option<String>, languages: &[ExtensionInfo]) -> Self {
        self.in_workspace(|state| state.with_host_extensions(host, languages))
    }

    /// The screen after the server on this machine said which languages there are.
    pub fn with_extensions(self, languages: Vec<ExtensionInfo>) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_extensions(languages)),
            Self::Workspace(state) => Self::Workspace(state.with_extensions(languages)),
        }
    }
}
