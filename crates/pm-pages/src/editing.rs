//! What the screen does for writing code: the file shown edited as its language says, what its language server answers, and the languages installed on this machine.

use pm_components::areas::SidebarEvent;
use pm_components::target::Target;

use crate::screen::Screen;
use crate::workspace::WorkspaceState;

impl Screen {
    /// The screen with `change` made to the workspace, when the workspace is showing.
    pub fn in_workspace(self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(change(state)),
        }
    }

    /// Whether the workspace is showing and `test` holds of it.
    fn workspace_is(&self, test: impl FnOnce(&WorkspaceState) -> bool) -> bool {
        matches!(self, Self::Workspace(state) if test(state))
    }

    /// The screen after the language server opened the file at `path` of the session at `session`, or could not.
    pub fn with_opened(
        self,
        session: usize,
        path: &str,
        outcome: Result<pm_protocol::Triggers, String>,
    ) -> Self {
        self.in_workspace(|state| state.with_opened(session, path, outcome))
    }

    /// The button that starts a new session in the project shown, when there is a project.
    pub fn new_session_button(&self) -> Option<pm_components::target::Target> {
        match self {
            Self::Workspace(state) if !state.projects.is_empty() => Some(Target::Sidebar(
                SidebarEvent::NewIn(state.current_project()),
            )),
            _ => None,
        }
    }

    /// The screen with the dock showing what is wrong with the files of the project.
    pub fn problems_shown(self) -> Self {
        self.in_workspace(|mut state| {
            state.show_problems();
            state
        })
    }

    /// The screen after the language server said how a colour can be written.
    pub fn with_color_presentations(
        self,
        session: usize,
        path: &str,
        color: [u8; 4],
        presentations: Vec<pm_protocol::ColorPresentation>,
    ) -> Self {
        self.in_workspace(|state| {
            state.with_color_presentations(session, path, color, presentations)
        })
    }

    /// How the editor looks and behaves, while the workspace is showing and its settings kept between runs are in.
    pub fn editor_settings(&self) -> Option<pm_editor::EditorSettings> {
        match self {
            Self::Workspace(state) if state.editor_loaded => Some(state.editor.clone()),
            _ => None,
        }
    }

    /// Whether the editor's settings are in and differ from `saved`, compared in place without
    /// copying them.
    pub fn editor_settings_differ(&self, saved: &pm_editor::EditorSettings) -> bool {
        matches!(self, Self::Workspace(state) if state.editor_loaded && state.editor != *saved)
    }

    /// Whether the window's own settings are in and differ from `saved`, compared in place
    /// without copying them.
    pub fn window_settings_differ(&self, saved: &pm_config::WindowSettings) -> bool {
        matches!(self, Self::Workspace(state) if state.window_loaded && state.window != *saved)
    }

    /// Whether the window is to close: the reader asked to and nothing is left to ask first.
    pub fn quitting(&self) -> bool {
        self.workspace_is(|state| state.quitting)
    }

    /// What the window keeps of its own, while the workspace is showing and those kept between runs are in.
    pub fn window_settings(&self) -> Option<pm_config::WindowSettings> {
        match self {
            Self::Workspace(state) if state.window_loaded => Some(state.window.clone()),
            _ => None,
        }
    }

    /// Whether the workspace is showing without the window's own settings kept between runs put in yet.
    pub fn window_settings_wanted(&self) -> bool {
        matches!(self, Self::Workspace(state) if !state.window_loaded)
    }

    /// Whether the workspace is showing without the editor's settings kept between runs put in yet.
    pub fn editor_settings_wanted(&self) -> bool {
        matches!(self, Self::Workspace(state) if !state.editor_loaded)
    }

    /// Whether the file shown has changes that are not saved.
    pub fn shown_dirty(&self) -> bool {
        match self {
            Self::Workspace(state) => state
                .shown_document()
                .is_some_and(|document| document.is_dirty() && !document.field.is_read_only()),
        }
    }

    /// The screen after the server answered a search of the project of the session at `session` for `query`, with `hits` or with why it could not.
    pub fn with_project_search(
        self,
        session: usize,
        query: &str,
        case: bool,
        regex: bool,
        hits: Result<Vec<pm_protocol::Location>, String>,
    ) -> Self {
        self.in_workspace(|state| state.with_project_search(session, query, case, regex, hits))
    }

    /// The screen after the server gave the sides of the conflicted file at `path` of the session at `session`, opening the resolver on it.
    pub fn with_conflict(
        self,
        session: usize,
        path: &str,
        files: pm_protocol::ConflictFiles,
    ) -> Self {
        self.in_workspace(|state| {
            state.with_conflict(session, path, files.ours, files.theirs, files.base)
        })
    }

    /// The screen after the language server made changes to the open files of a session by a change of a whole file.
    pub fn with_source_edited(self, session: usize, files: Vec<pm_protocol::FileEdit>) -> Self {
        self.in_workspace(|state| state.with_edited_files(session, files))
    }

    /// How tall the view of the file shown was last laid out, as the view told its holder, while the workspace is showing and it was laid out.
    pub fn code_height(&self) -> Option<f32> {
        match self {
            Self::Workspace(state) => state.code_size.get().map(|size| size.height),
        }
    }
}
