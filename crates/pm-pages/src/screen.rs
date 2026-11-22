//! The workspace screen and the actions and auxiliary windows it exposes.

use pm_components::areas::DialogEvent;
use pm_components::events::Change;
use pm_components::target::Target;

use crate::workspace::{WorkspaceState, workspace};
use pm_components::context_menu::{ContextMenu, field_menu};
use pm_components::folder_picker::FolderPicker;
use pm_components::project::Project;
use pm_components::{Window, shadowed};

/// Which screen is showing, with what it remembers.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    /// The projects, their sessions and the page being read.
    Workspace(WorkspaceState),
}

impl std::ops::Deref for Screen {
    /// The workspace state a screen gives access to.
    type Target = WorkspaceState;

    /// The workspace the screen shows, so that what can be asked of it can be
    /// asked of the screen.
    fn deref(&self) -> &WorkspaceState {
        match self {
            Self::Workspace(state) => state,
        }
    }
}

impl std::ops::DerefMut for Screen {
    /// The workspace the screen shows, to change in place.
    fn deref_mut(&mut self) -> &mut WorkspaceState {
        match self {
            Self::Workspace(state) => state,
        }
    }
}

impl Default for Screen {
    /// An empty workspace, ready to add a project.
    fn default() -> Self {
        Self::opened(&[])
    }
}

impl Screen {
    /// The workspace of the projects at `paths`, including an empty first launch.
    pub fn opened(paths: &[String]) -> Self {
        let projects: Vec<Project> = paths
            .iter()
            .filter_map(|path| Project::from_source(path))
            .collect();
        Self::Workspace(WorkspaceState::new(&projects))
    }

    /// The screen listing `agents` as the ones that can be started.
    pub fn with_agent_list(self, agents: Vec<pm_components::agent_list::AgentEntry>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_agent_servers(agents)),
        }
    }

    /// What the reader chose for the agents' settings, while the workspace is showing.
    pub fn choices(&self) -> Option<&[(String, String, String)]> {
        match self {
            Self::Workspace(state) => Some(&state.choices),
        }
    }

    /// The paths of the projects in the workspace.
    pub fn project_paths(&self) -> Option<Vec<String>> {
        match self {
            Self::Workspace(state) => Some(
                state
                    .projects
                    .iter()
                    .map(|project| project.path.clone())
                    .collect(),
            ),
        }
    }

    /// The screen after `change` was made to the workspace.
    pub fn changed(self, change: Change) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(change(state)),
        }
    }

    /// The selection of the conversation's text.
    pub fn chat_selection(&self) -> rustle::SelectionState {
        match self {
            Self::Workspace(state) => state.chat_selection.clone(),
        }
    }

    /// The selection of the text of the card about a place in the file shown.
    pub fn hover_selection(&self) -> rustle::SelectionState {
        match self {
            Self::Workspace(state) => state.hover_selection.clone(),
        }
    }

    /// Where the areas the app asks about by name were drawn in the latest frame.
    pub fn places(&self) -> pm_components::bounds::Places {
        match self {
            Self::Workspace(state) => state.places.clone(),
        }
    }

    /// Where the lines of code text were drawn in the latest frame.
    pub fn code_lines(&self) -> pm_components::text_lines::TextLines {
        match self {
            Self::Workspace(state) => state.code_lines.clone(),
        }
    }

    /// Whether the panel for adding a project is open.
    pub fn adder_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.adder_open())
    }

    /// Whether settings are open.
    pub fn settings_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.settings_open())
    }

    /// The folder picker, while it is open.
    pub fn folder_picker(&self) -> Option<&FolderPicker> {
        match self {
            Self::Workspace(state) => state.folder_picker.as_ref(),
        }
    }

    /// Which opening of the folder picker is showing, while it is open.
    pub fn folder_picker_serial(&self) -> Option<u64> {
        self.folder_picker().map(|open| open.serial)
    }

    /// Whether the folder picker is open.
    pub fn folder_picker_open(&self) -> bool {
        self.folder_picker().is_some()
    }

    /// What pressing Enter presses while the folder picker is open.
    pub fn folder_picker_enter(&self) -> Option<Target> {
        self.folder_picker().map(FolderPicker::enter)
    }

    /// The screen with the folders the server listed for `requested`, or why it could not, when that is still the folder the picker wants.
    pub fn with_folders(
        self,
        requested: &str,
        listed: Result<(String, Vec<String>), String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_folders(requested, listed)),
        }
    }

    /// The menu open over a field, and where, if one is.
    pub fn menu(&self) -> Option<ContextMenu> {
        match self {
            Self::Workspace(state) => state.menu,
        }
    }

    /// The screen with the session at `position` closed, by the server or because no server holds it.
    pub fn closed_elsewhere(self, position: usize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.closed_session(position, false, false)),
        }
    }

    /// The screen with the page showing recorded among the pages visited.
    pub fn with_place_recorded(self) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.track();
                Self::Workspace(state)
            }
        }
    }

    /// Whether the page showing is yet to be recorded among the pages visited.
    pub fn place_unrecorded(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.untracked())
    }

    /// The lists whose scrolls the screen follows: the diff's and the conversation's.
    pub fn followed_lists(&self) -> (rustle::LazyListState, rustle::LazyListState) {
        let Self::Workspace(state) = self;
        (state.diff_list.clone(), state.chat_list.clone())
    }

    /// Whether the reader scrolled what the screen follows since it last looked.
    pub fn scrolls_moved(&self) -> bool {
        let Self::Workspace(state) = self;
        state.scrolls_moved()
    }

    /// Catches the screen up with what the reader scrolled, and says whether
    /// the diff moved, so the diffs that came into view can be asked for.
    pub fn follow_scrolls(&mut self) -> bool {
        let Self::Workspace(state) = self;
        state.follow_scrolls()
    }

    /// Whether the conversation is shown at its end, give or take a pixel.
    pub fn conversation_at_end(&self) -> bool {
        let Self::Workspace(state) = self;
        state.conversation_at_end()
    }

    /// Where the conversation is drawn in its window, while it is shown.
    pub fn conversation_area(&self) -> Option<rustle::Rect> {
        let Self::Workspace(state) = self;
        state.chat_bounds.get()
    }

    /// Where the view of the file shown is drawn in its window, while it is shown.
    pub fn code_area(&self) -> Option<rustle::Rect> {
        let Self::Workspace(state) = self;
        state.code_bounds.get()
    }

    /// Whether the tree is asking for a name in a field.
    pub fn is_naming(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.is_naming())
    }

    /// The screen after the server answered for the file at `path` of the session at `session`.
    pub fn with_file(
        self,
        session: usize,
        path: &str,
        head: bool,
        read: Result<(Option<String>, bool, Option<String>), String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_file(session, path, head, read)),
        }
    }

    /// The screen after the server answered for the picture file at `path` of the session at `session`, decoded, or why it could not be.
    pub fn with_picture(
        self,
        session: usize,
        path: &str,
        head: bool,
        read: Result<rustle::ImageData, String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_picture(session, path, head, read))
            }
        }
    }

    /// The screen after the text of the vector picture at `path` of the session at `session` was drawn, or could not be.
    pub fn with_drawn(
        self,
        session: usize,
        path: &str,
        head: bool,
        drawn: Result<rustle::ImageData, String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_drawn(session, path, head, drawn)),
        }
    }

    /// The screen after the figure kept under `key` was drawn or fetched, or could not be.
    pub fn with_figure(
        self,
        key: u64,
        drawn: Result<pm_components::figures::Drawing, String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_figure(key, drawn)),
        }
    }

    /// Whether the server is busy with something asked about the open session's folder.
    pub fn git_working(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.git.working)
    }

    /// The screen knowing whether the server is busy with something asked about a folder; once it is not, nothing is waited on.
    pub fn with_git_working(self, working: bool) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.git.working = working;
                if !working {
                    state.git.busy = None;
                }
                Self::Workspace(state)
            }
        }
    }

    /// Whether the menu on changed files, the question of whether to discard or the list of branches is open.
    pub fn review_overlay_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.review_overlay_open())
    }

    /// Whether lines of the diff are being dragged over.
    pub fn comment_dragging(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.comment_dragging())
    }

    /// Whether the box for a comment on the diff is open.
    pub fn commenting(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.commenting())
    }

    /// Whether a list of commands is showing over the prompt.
    pub fn slash_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.slash_open())
    }

    /// Whether the offer to put files back as they were before a turn is open.
    pub fn restore_offered(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.restoring.is_some())
    }

    /// The screen with no setting menu, and no offer to put files back, open.
    pub fn without_picker(self) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.picker = None;
                state.restoring = None;
                state.git.branch_menu = false;
                Self::Workspace(state)
            }
        }
    }

    /// The screen after the agent at `agent` signed in under the login `login`, which may have changed who it is.
    pub fn signed_in(self, agent: usize, login: Option<String>) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.forget_identity(agent);
                state.outbox_identity(agent, login);
                Self::Workspace(state.sign_in_code_ended())
            }
        }
    }

    /// The screen with the sessions the server of `host` holds added, and each one's position with the server's number for it.
    pub fn restored(
        self,
        host: &Option<String>,
        infos: &[pm_protocol::SessionInfo],
    ) -> (Self, Vec<(usize, u64)>) {
        match self {
            Self::Workspace(mut state) => {
                let added = state.restore_sessions(host, infos);
                (Self::Workspace(state), added)
            }
        }
    }

    /// Whether the page for starting work is showing, which is where the prompt takes keyboard focus unasked.
    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.starting())
    }

    /// Whether the workspace has something to send to the servers.
    pub fn has_outbox(&self) -> bool {
        match self {
            Self::Workspace(state) => !state.outbox.is_empty(),
        }
    }

    /// The columns and lines of the window of the shell showing, and how far back it is scrolled.
    pub fn terminal_window(&self) -> Option<(usize, usize, usize)> {
        match self {
            Self::Workspace(state) if state.terminal => state.terminal_window(),
            _ => None,
        }
    }

    /// Reads what the shell of the tab `tab` printed, while the workspace is showing.
    pub fn terminal_output(&mut self, tab: u64, data: &[u8]) {
        let Self::Workspace(state) = self;
        state.terminal_output(tab, data);
    }

    /// Takes the shell of the tab `tab` out of the workspace because it ended.
    pub fn terminal_exited(&mut self, tab: u64) {
        let Self::Workspace(state) = self;
        state.terminal_exited(tab);
    }

    /// Types `data` into the shell showing.
    pub fn type_into_terminal(&mut self, data: Vec<u8>) {
        let Self::Workspace(state) = self;
        state.type_into_terminal(data);
    }

    /// Shows `text` an input method is composing at the cursor of the shell showing, or stops showing it when empty.
    pub fn compose_in_terminal(&mut self, text: &str) {
        let Self::Workspace(state) = self;
        state.compose_in_terminal(text);
    }

    /// Scrolls the shell showing `lines` lines back up, or down when negative.
    pub fn scroll_terminal(&mut self, lines: isize) {
        let Self::Workspace(state) = self;
        state.scroll_terminal(lines);
    }

    /// Tells the shell showing that its window is `columns` by `rows`.
    pub fn fit_terminal(&mut self, columns: u16, rows: u16) {
        let Self::Workspace(state) = self;
        state.fit_terminal(columns, rows);
    }

    /// Marks text in the shell showing, from `anchor` to `head`, each a line of its window and a column.
    pub fn select_in_terminal(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        let Self::Workspace(state) = self;
        state.select_in_terminal(anchor, head);
    }

    /// Marks text in the shell showing, from `anchor` to `head`, each a line counted from the oldest kept and a column.
    pub fn select_terminal_span(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        let Self::Workspace(state) = self;
        state.select_terminal_span(anchor, head);
    }

    /// Underlines the web address at `at` of the window of the shell showing, a line of it and a column, or none when `at` is nothing.
    pub fn reach_terminal_link(&mut self, at: Option<(usize, usize)>) {
        let Self::Workspace(state) = self;
        state.reach_terminal_link(at);
    }

    /// Unmarks the text of the shell showing.
    pub fn clear_terminal_selection(&mut self) {
        let Self::Workspace(state) = self;
        state.clear_terminal_selection();
    }
}

/// Emits the whole screen for `state` in a window `width` by `height` logical pixels: the root content of the main window, in its frame.
#[track_caller]
pub fn screen(state: &Screen, width: f32, height: f32) {
    match state {
        Screen::Workspace(state) => shadowed(|| workspace(state, width, height)),
    }
}

/// Emits what the folder picker's window, `width` by `height` logical pixels, shows for `state` while the picker is open: the root content of that window; emits nothing while the picker is closed. The picker fills the window whatever its size, so the size is taken only to match the other roots.
pub fn picker_screen(state: &Screen, width: f32, height: f32) {
    let _ = (width, height);
    let Some(picker) = state.folder_picker() else {
        return;
    };
    Window::new()
        .body(move || picker.view())
        .over(move || {
            if let Some(menu) = state
                .menu()
                .filter(|menu| menu.field == Target::Dialog(DialogEvent::PickerField))
                && let Some(field) = state.text_field(menu.field)
            {
                field_menu(field, menu);
            }
        })
        .build();
}

/// Emits what the settings window, `width` by `height` logical pixels, shows for `state` while settings are open: the root content of that window; emits nothing while settings are closed. The page fills the window's height, so only `width` shapes it.
pub fn settings_screen(state: &Screen, width: f32, height: f32) {
    let _ = height;
    match state {
        Screen::Workspace(state) => crate::workspace::settings_window(state, width),
    }
}

/// Emits the settings window's root content, as [`settings_screen`] does; the name the host calls it by.
#[track_caller]
pub fn settings_window(state: &Screen, width: f32, height: f32) {
    settings_screen(state, width, height);
}

/// First launch and restored workspaces share the same entry point.
#[cfg(test)]
mod tests {
    use super::*;
    use pm_components::areas::SidebarEvent;
    use pm_components::events::Event;

    /// A fresh installation opens the workspace without waiting for agent setup.
    #[test]
    fn first_launch_opens_empty_workspace() {
        let Screen::Workspace(state) = Screen::default();
        assert!(state.projects.is_empty());
        assert!(state.sessions.is_empty());
        assert!(!state.offered_agents().is_empty());
    }

    /// Project selection and settings are accessible immediately on first launch.
    #[test]
    fn first_launch_can_add_projects_and_open_settings() {
        assert!(
            Screen::default()
                .in_workspace(|state| state.handle(Event::Sidebar(SidebarEvent::AddProject)))
                .adder_open()
        );
        assert!(
            Screen::default()
                .in_workspace(|state| state.handle(Event::Sidebar(SidebarEvent::Settings)))
                .settings_open()
        );
    }

    /// Existing local and remote projects still open directly without setup.
    #[test]
    fn saved_projects_open_directly() {
        let paths = vec!["/project".to_owned(), "ssh://builder/project".to_owned()];
        let screen = Screen::opened(&paths);
        assert_eq!(screen.project_paths(), Some(paths));
    }
}
