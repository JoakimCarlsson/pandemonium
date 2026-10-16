//! What the screen does for writing code: the file shown edited as its language says, what its language server answers, and the languages installed on this machine.

use pm_protocol::{Completion, Diagnostic, Hover, LanguageInfo, Position};

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

    /// The screen after the server of `host` said how its languages stand.
    pub fn with_host_languages(self, host: &Option<String>, languages: &[LanguageInfo]) -> Self {
        self.in_workspace(|state| state.with_host_languages(host, languages))
    }

    /// The screen after the server on this machine said which languages there are.
    pub fn with_languages(self, languages: Vec<LanguageInfo>) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_languages(languages)),
            Self::Workspace(state) => Self::Workspace(state.with_languages(languages)),
        }
    }
}
