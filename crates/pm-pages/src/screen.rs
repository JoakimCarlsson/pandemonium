//! The screen the window shows: onboarding until it is finished, the workspace after.

use pm_acp::Event;
use pm_input::FocusId;
use pm_text::{Edit, Field, Shaper};
use pm_ui::Node;

use crate::onboarding::{ADD, AgentStatus, OnboardingState, PATH, onboarding};
use crate::workspace::{Outgoing, WorkspaceState, workspace};
use pm_components::context_menu::{ContextMenu, field_menu};
use pm_components::folder_picker::FolderPicker;
use pm_components::git_controls::Pick;
use pm_components::git_menus::MenuKind;
use pm_components::project::Project;
use pm_components::sash::Grows;
use pm_components::tree_actions::TreeTarget;

/// Which screen is showing, with what it remembers.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Screen {
    /// Adding a project and connecting agents.
    Onboarding(OnboardingState),
    /// The projects, their sessions and the page being read.
    Workspace(WorkspaceState),
}

impl Default for Screen {
    /// The first step of onboarding.
    fn default() -> Self {
        Self::Onboarding(OnboardingState::default())
    }
}

impl Screen {
    /// The workspace of the projects at `paths`, for a launch after the app was set up.
    pub fn opened(paths: &[String]) -> Self {
        let projects: Vec<Project> = paths
            .iter()
            .filter_map(|path| Project::from_source(path))
            .collect();
        Self::Workspace(WorkspaceState::new(&projects))
    }

    /// The screen listing `agents` as the ones that can be started.
    pub fn with_agent_list(self, agents: Vec<pm_protocol::AgentServer>) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.listing(&agents)),
            Self::Workspace(state) => Self::Workspace(state.with_agent_servers(agents)),
        }
    }

    /// The screen with what the reader chose for the agents' settings before.
    pub fn with_choices(self, choices: Vec<(String, String, String)>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_choices(choices)),
            onboarding => onboarding,
        }
    }

    /// What the reader chose for the agents' settings, while the workspace is showing.
    pub fn choices(&self) -> Option<&[(String, String, String)]> {
        match self {
            Self::Workspace(state) => Some(&state.choices),
            Self::Onboarding(_) => None,
        }
    }

    /// The paths of the projects of the workspace, or nothing while onboarding is showing.
    pub fn project_paths(&self) -> Option<Vec<String>> {
        match self {
            Self::Onboarding(_) => None,
            Self::Workspace(state) => Some(
                state
                    .projects
                    .iter()
                    .map(|project| project.path.clone())
                    .collect(),
            ),
        }
    }

    /// The screen after `action` was pressed; finishing onboarding opens the
    /// workspace with the projects it added.
    pub fn after(self, action: FocusId) -> Self {
        match self {
            Self::Onboarding(state) => {
                let state = state.after(action);
                if state.finished {
                    Self::Workspace(WorkspaceState::new(&state.projects))
                } else {
                    Self::Onboarding(state)
                }
            }
            Self::Workspace(state) => Self::Workspace(state.after(action)),
        }
    }

    /// The screen after `edit` is made to the field named `id`.
    pub fn edited(self, id: FocusId, edit: Edit) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.edited(id, edit)),
            Self::Workspace(state) => Self::Workspace(state.edited(id, edit)),
        }
    }

    /// The text field `id` names, if the screen has one by that name.
    pub fn field(&self, id: FocusId) -> Option<&Field> {
        match self {
            Self::Onboarding(state) => state.field(id),
            Self::Workspace(state) => state.field(id),
        }
    }

    /// Whether `id` names a text field of either screen.
    pub fn is_field(id: FocusId) -> bool {
        OnboardingState::is_field(id) || WorkspaceState::is_field(id)
    }

    /// Which agent the button `id` acts on, if it is one of onboarding's.
    pub fn agent_of(id: FocusId) -> Option<usize> {
        OnboardingState::agent_of(id)
    }

    /// What pressing Enter inside the field `id` presses.
    pub fn submit_of(id: FocusId) -> FocusId {
        if id == PATH {
            ADD
        } else {
            WorkspaceState::submit_of(id)
        }
    }

    /// What pressing Control or Command with Enter inside the field `id` presses, when it presses something of its own.
    pub fn hurried_submit_of(id: FocusId) -> Option<FocusId> {
        WorkspaceState::hurried_submit_of(id)
    }

    /// Whether the panel for adding a project is open.
    pub fn adder_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.adder_open())
    }

    /// Which server's menu button `id` is, while the settings panel is open.
    pub fn settings_more_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.settings_more_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the menu of the `index`-th server of the settings page open at `at`.
    pub fn with_settings_menu(self, index: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_settings_menu(index, at)),
            other => other,
        }
    }

    /// Whether settings are open.
    pub fn settings_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.settings_open())
    }

    /// The screen with settings closed, which is what closing their window does.
    pub fn without_settings(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.without_settings()),
            other => other,
        }
    }

    /// What pressing Enter presses while the panel for adding a project is open.
    pub fn adder_enter(&self) -> Option<FocusId> {
        match self {
            Self::Workspace(state) => state.adder_enter(),
            Self::Onboarding(_) => None,
        }
    }

    /// The folder picker, while it is open.
    pub fn folder_picker(&self) -> Option<&FolderPicker> {
        match self {
            Self::Onboarding(state) => state.folder_picker.as_ref(),
            Self::Workspace(state) => state.folder_picker.as_ref(),
        }
    }

    /// Whether the folder picker is open.
    pub fn folder_picker_open(&self) -> bool {
        self.folder_picker().is_some()
    }

    /// What pressing Enter presses while the folder picker is open.
    pub fn folder_picker_enter(&self) -> Option<FocusId> {
        self.folder_picker().map(FolderPicker::enter)
    }

    /// The screen with the folder picker put away, which is what closing its window does.
    pub fn without_folder_picker(self) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.without_folder_picker()),
            Self::Workspace(state) => Self::Workspace(state.without_folder_picker()),
        }
    }

    /// The screen with the folders the server listed for `requested`, or why it could not, when that is still the folder the picker wants.
    pub fn with_folders(
        self,
        requested: &str,
        listed: Result<(String, Vec<String>), String>,
    ) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_folders(requested, listed)),
            Self::Workspace(state) => Self::Workspace(state.with_folders(requested, listed)),
        }
    }

    /// The screen after the list of the folder picker is scrolled by `delta`
    /// logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_folder_picker(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.scrolled_folder_picker(delta, room)),
            Self::Workspace(state) => Self::Workspace(state.scrolled_folder_picker(delta, room)),
        }
    }

    /// The menu open over a field, and where, if one is.
    pub fn menu(&self) -> Option<ContextMenu> {
        match self {
            Self::Onboarding(state) => state.menu,
            Self::Workspace(state) => state.menu,
        }
    }

    /// The screen with a menu open over the field `field` at `at`.
    pub fn with_menu(self, field: FocusId, at: (f32, f32)) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_menu(field, at)),
            Self::Workspace(state) => Self::Workspace(state.with_menu(field, at)),
        }
    }

    /// The screen with the menu of the project at `project` open at `at`, while the workspace is showing.
    pub fn with_project_menu(self, project: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_project_menu(project, at)),
            onboarding => onboarding,
        }
    }

    /// Which project's menu button `id` is, while the workspace is showing.
    pub fn more_button_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.more_button_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// Which project's header `id` is, while the workspace is showing.
    pub fn header_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.header_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The project the element `id` belongs to, while the workspace is showing.
    pub fn project_at(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.project_at(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the project at `from` being dragged to the place of the one at `over`, with the pointer at `at`.
    pub fn dragged_over(self, from: usize, over: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.dragged_over(from, over, at)),
            onboarding => onboarding,
        }
    }

    /// The screen after a dragged project was let go.
    pub fn dropped(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.dropped()),
            onboarding => onboarding,
        }
    }

    /// The screen with the menu of panes open with its corner at `at`.
    pub fn with_pane_menu(self, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_pane_menu(at)),
            onboarding => onboarding,
        }
    }

    /// The tab of a pane `id` is, while the workspace is showing.
    pub fn pane_tab_of(&self, id: FocusId) -> Option<pm_components::panes::Item> {
        match self {
            Self::Workspace(state) => state.pane_tab_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the tab `item` being dragged, over the `over` pane and edge, if any, with the pointer at `at`.
    pub fn pane_dragged(
        self,
        item: pm_components::panes::Item,
        over: Option<(usize, pm_components::panes::Side)>,
        at: (f32, f32),
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.pane_dragged(item, over, at)),
            onboarding => onboarding,
        }
    }

    /// The screen after a dragged tab was let go.
    pub fn pane_dropped(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.pane_dropped()),
            onboarding => onboarding,
        }
    }

    /// The screen after the file at `index` in a pane other than the focused one is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_peek(self, index: usize, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_peek(index, delta, room)),
            onboarding => onboarding,
        }
    }

    /// Whether a menu is open over a project, a session or a message, so that a press elsewhere has to close it.
    pub fn list_menu_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.project_menu.is_some() || state.session_menu.is_some() || state.message_menu.is_some() || state.terminal_menu.is_some() || state.git.menu.is_some() || state.settings_servers.menu.is_some() || state.tab_menu.is_some() || state.pane_menu.is_some())
    }

    /// Which session the row `id` of the sidebar opens, while the workspace is showing.
    pub fn session_row_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.session_row_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the menu over the message at `message` of the open
    /// session open at `at`, offering to copy the selection when `selected`,
    /// while the workspace is showing.
    pub fn with_message_menu(self, message: usize, at: (f32, f32), selected: bool) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_message_menu(message, at, selected))
            }
            onboarding => onboarding,
        }
    }

    /// The screen with the menu over the shell showing open at `at`, while the workspace is showing.
    pub fn with_terminal_menu(self, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_terminal_menu(at)),
            onboarding => onboarding,
        }
    }

    /// The screen after `command` was carried out on the shell showing, which closes the menu over it, while the workspace is showing.
    pub fn terminal_command(self, command: pm_components::terminal_menu::TerminalCommand) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.terminal_command(command)),
            onboarding => onboarding,
        }
    }

    /// The screen with the menu of the session at `session` open at `at`.
    pub fn with_session_menu(self, session: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_session_menu(session, at)),
            onboarding => onboarding,
        }
    }

    /// The screen with the session at `position` closed by another client.
    pub fn closed_elsewhere(self, position: usize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.closed_session(position, false, false)),
            onboarding => onboarding,
        }
    }

    /// The screen moved `delta` places along the pages and views visited, towards the past when negative.
    pub fn navigated(self, delta: isize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.navigated(delta)),
            onboarding => onboarding,
        }
    }

    /// The screen with the page showing recorded among the pages visited.
    pub fn with_place_recorded(self) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.track();
                Self::Workspace(state)
            }
            onboarding => onboarding,
        }
    }

    /// Whether the page showing is yet to be recorded among the pages visited.
    pub fn place_unrecorded(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.untracked())
    }

    /// The screen with no menu open.
    pub fn without_menu(self) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.without_menu()),
            Self::Workspace(state) => Self::Workspace(state.without_menu()),
        }
    }

    /// The screen after the reader chose the folder at `path` as a project: added, and open on a new session in it.
    pub fn with_project_opened(self, path: &str) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_project(path)),
            Self::Workspace(state) => Self::Workspace(state.with_project_opened(path)),
        }
    }

    /// The screen with the project at `path` added, which the reader did not ask to see.
    pub fn with_project(self, path: &str) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_project(path)),
            Self::Workspace(state) => Self::Workspace(state.with_project(path)),
        }
    }

    /// The screen with the agent at `index` standing at `status`.
    pub fn with_agent(self, index: usize, status: AgentStatus) -> Self {
        match self {
            Self::Onboarding(state) => Self::Onboarding(state.with_agent(index, status)),
            workspace => workspace,
        }
    }

    /// Where the agent at `index` stands, while onboarding shows them.
    pub fn agent(&self, index: usize) -> Option<&AgentStatus> {
        match self {
            Self::Onboarding(state) => state.agents.get(index),
            Self::Workspace(_) => None,
        }
    }

    /// The screen after the wheel turned `delta` logical pixels at `x` in a
    /// window `height` logical pixels tall.
    pub fn scrolled(self, x: f32, delta: f32, height: f32) -> Self {
        match self {
            Self::Workspace(state) if x < state.sidebar_room() => {
                Self::Workspace(state.scrolled(delta, height))
            }
            other => other,
        }
    }

    /// The screen after the conversation is scrolled by `delta` logical
    /// pixels, downwards when positive, with `room` pixels to scroll back.
    pub fn scrolled_chat(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_chat(delta, room)),
            other => other,
        }
    }

    /// The screen after the list of values in the open menu is scrolled by
    /// `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_menu(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_menu(delta, room)),
            other => other,
        }
    }

    /// The screen after the plan panel is scrolled by `delta` logical
    /// pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_plan(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_plan(delta, room)),
            other => other,
        }
    }

    /// The screen after the diff is scrolled by `delta` logical pixels,
    /// downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_diff(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_diff(delta, room)),
            other => other,
        }
    }

    /// The screen after the list of sections of the settings window is scrolled by `delta`
    /// logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_settings_nav(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_settings_nav(delta, room)),
            other => other,
        }
    }

    /// The screen after the page showing in the settings panel is scrolled by `delta`
    /// logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_settings(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_settings(delta, room)),
            other => other,
        }
    }

    /// The screen after the review panel's list is scrolled by `delta`
    /// logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_tree(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_tree(delta, room)),
            other => other,
        }
    }

    /// Which changed file's row `id` is, while the workspace is showing.
    pub fn change_row_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.change_row_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The file or folder of the project's files row `id` is, while the workspace is showing.
    pub fn tree_row_of(&self, id: FocusId) -> Option<TreeTarget> {
        match self {
            Self::Workspace(state) => state.tree_row_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the menu on the project's files open at `at` over `target`.
    pub fn with_tree_menu(self, target: TreeTarget, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_tree_menu(target, at)),
            other => other,
        }
    }

    /// The screen after the server answered a write of the file at `path` of the session at `session`.
    pub fn with_file_written(self, session: usize, path: &str, failed: bool) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_file_written(session, path, failed))
            }
            other => other,
        }
    }

    /// The screen with the file shown scrolled to show its caret, in a view `height` logical pixels tall.
    pub fn with_caret_shown(self, height: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_caret_shown(height)),
            other => other,
        }
    }

    /// The screen after a file's tab or row was pressed twice in quick succession, which keeps the file open.
    pub fn with_file_kept(self, id: FocusId) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_file_kept(id)),
            other => other,
        }
    }

    /// Whether the tree is asking for a name in a field.
    pub fn is_naming(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.is_naming())
    }

    /// The screen with the name being typed in the tree given up.
    pub fn without_naming(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.without_naming()),
            other => other,
        }
    }

    /// The screen after files were made, renamed, copied or removed and must be listed again.
    pub fn with_tree_stale(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_tree_stale()),
            other => other,
        }
    }

    /// Which folder of the changed files' tree row `id` is, while the workspace is showing.
    pub fn change_folder_row_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.change_folder_row_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the menu on changed files open at `at` over the folder at `position` of their tree.
    pub fn with_folder_change_menu(self, position: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_folder_change_menu(position, at)),
            other => other,
        }
    }

    /// The screen with the menu on changed files open at `at` over the file at `index`.
    pub fn with_change_menu(self, index: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_change_menu(index, at)),
            other => other,
        }
    }

    /// The screen after the changed file at `index` was pressed with the modifier that picks several.
    pub fn change_picked(self, index: usize, how: Pick) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.change_picked(index, how)),
            other => other,
        }
    }

    /// The screen after the server listed the branches of the folder of the session at `session`.
    pub fn with_session_branches(self, session: usize, names: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_session_branches(session, names)),
            other => other,
        }
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
            other => other,
        }
    }

    /// The screen after the server answered for the picture file at `path` of the session at `session`, decoded, or why it could not be.
    pub fn with_picture(
        self,
        session: usize,
        path: &str,
        head: bool,
        read: Result<pm_ui::Picture, String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_picture(session, path, head, read))
            }
            other => other,
        }
    }

    /// The screen after the text of the vector picture at `path` of the session at `session` was drawn, or could not be.
    pub fn with_drawn(
        self,
        session: usize,
        path: &str,
        head: bool,
        drawn: Result<pm_ui::Picture, String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_drawn(session, path, head, drawn)),
            other => other,
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
            other => other,
        }
    }

    /// The screen with every figure let go and those of the file shown asked for again, as when the colours diagrams are drawn in change.
    pub fn without_figures(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.without_figures()),
            other => other,
        }
    }

    /// Which menu the button `id` opens, if it is one.
    pub fn menu_button_of(id: FocusId) -> Option<MenuKind> {
        WorkspaceState::menu_button_of(id)
    }

    /// Which commit of the graph the row `id` is, while the workspace is showing.
    pub fn history_row_of(&self, id: FocusId) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.history_row_of(id),
            Self::Onboarding(_) => None,
        }
    }

    /// Whether `id` names a row of the menu of the source control panel that is open.
    pub fn is_menu_row(&self, id: FocusId) -> bool {
        matches!(self, Self::Workspace(state) if state.is_menu_row(id))
    }

    /// The screen with the menu `kind` open at `at`.
    pub fn with_git_menu(self, kind: MenuKind, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_git_menu(kind, at)),
            other => other,
        }
    }

    /// The screen after the server listed the stashes of the repository of the session at `session`.
    pub fn with_stashes(self, session: usize, stashes: Vec<pm_git::Stash>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_stashes(session, stashes)),
            other => other,
        }
    }

    /// How big the panel the handle `id` resizes is now, in a window `window` logical pixels wide and tall, and which way the handle is pulled to grow it.
    pub fn sash(&self, id: FocusId, window: (f32, f32)) -> Option<(f32, Grows)> {
        match self {
            Self::Workspace(state) => state.sash(id, window),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen with the panel the handle `id` resizes made `size` logical pixels big, within what that panel allows in a window `window` logical pixels wide and tall.
    pub fn with_sash_size(self, id: FocusId, size: f32, window: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_sash_size(id, size, window)),
            other => other,
        }
    }

    /// The screen after the branch of the session at `session` was seen to stand as `tracking` says.
    pub fn tracked(
        self,
        session: usize,
        branch: Option<&str>,
        tracking: pm_git::Tracking,
        operation: Option<pm_git::Operation>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.tracked(session, branch, tracking, operation))
            }
            other => other,
        }
    }

    /// The screen after the server listed the commits of the repository of the session at `session`.
    pub fn with_history(self, session: usize, commits: Vec<pm_git::Commit>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_history(session, commits)),
            other => other,
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
            other => other,
        }
    }

    /// The screen after git refused something asked of the open session's folder, for `reason`: the source control panel says so.
    pub fn git_failed(self, reason: &str) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.git_failed(reason)),
            other => other,
        }
    }

    /// The screen after the remote answered the request of the session at `session`.
    pub fn remote_done(self, session: usize, failure: Option<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.remote_done(session, failure)),
            other => other,
        }
    }

    /// Whether the graph is open, and whether it lists every reference.
    pub fn graph_wanted(&self) -> Option<bool> {
        match self {
            Self::Workspace(state) => state.graph_wanted(),
            Self::Onboarding(_) => None,
        }
    }

    /// The screen after the graph is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_graph(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_graph(delta, room)),
            other => other,
        }
    }

    /// Whether pressing Enter in the field `id` puts a line break in it rather than pressing its button.
    pub fn enter_breaks_line(id: FocusId) -> bool {
        id == pm_components::state::COMMIT_MESSAGE || id == pm_components::open_files::EDITOR_FIELD
    }

    /// Whether the menu on changed files, the question of whether to discard or the list of branches is open.
    pub fn review_overlay_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.review_overlay_open())
    }

    /// The screen after the excerpts are scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_excerpts(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_excerpts(delta, room)),
            other => other,
        }
    }

    /// The screen after the file being read is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_viewer(self, delta: f32, room: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.scrolled_viewer(delta, room)),
            other => other,
        }
    }

    /// The screen after the pointer went down on the line number `id` of the diff.
    pub fn comment_drag_begun(self, id: FocusId) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.comment_drag_begun(id)),
            other => other,
        }
    }

    /// The screen after the pointer, held down, moved over the line number `id` of the diff.
    pub fn comment_drag_extended(self, id: FocusId) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.comment_drag_extended(id)),
            other => other,
        }
    }

    /// The screen after the pointer was let go over the diff.
    pub fn comment_drag_ended(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.comment_drag_ended()),
            other => other,
        }
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

    /// The screen with the highlight in the list of commands moved by `delta` rows.
    pub fn slash_moved(self, delta: isize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.slash_moved(delta)),
            other => other,
        }
    }

    /// The screen with the highlighted command typed into the prompt.
    pub fn slash_taken(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.slash_taken()),
            other => other,
        }
    }

    /// The screen with the open session's mode changed to the next one.
    pub fn mode_cycled(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.mode_cycled()),
            other => other,
        }
    }

    /// Whether a menu of a setting, or the offer to put files back, is open.
    pub fn picker_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.picker.is_some() || state.restoring.is_some() || state.git.branch_menu)
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
            other => other,
        }
    }

    /// The screen after the agent at `agent` told what it lets the reader change and what commands it offers.
    pub fn with_discovery(self, agent: usize, found: pm_acp::Discovery) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_discovery(agent, found)),
            onboarding => onboarding,
        }
    }

    /// The screen with `paths` mentioned in the prompt, while the workspace is showing.
    pub fn with_attachments(self, paths: &[String]) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_attachments(paths)),
            onboarding => onboarding,
        }
    }

    /// The screen knowing what the picture at `path` looks like, while the workspace is showing.
    pub fn with_preview(self, path: &str, picture: pm_ui::Picture) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_preview(path, picture)),
            onboarding => onboarding,
        }
    }

    /// The screen with the logins the reader added before and the one in use for each agent.
    pub fn with_accounts(
        self,
        logins: Vec<(String, String, String)>,
        in_use: Vec<(String, String)>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_accounts(logins, in_use)),
            onboarding => onboarding,
        }
    }

    /// The screen after the agent at `agent` signed in, which may have changed who it is.
    pub fn signed_in(self, agent: usize) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.forget_identity(agent);
                state.outbox_identity(agent);
                Self::Workspace(state)
            }
            onboarding => onboarding,
        }
    }

    /// The screen holding the settings the agent at `agent` on the machine `host` offers, while the workspace is showing.
    pub fn with_discovery_for(
        self,
        host: Option<String>,
        agent: usize,
        found: pm_acp::Discovery,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_discovery_for(host, agent, found)),
            onboarding => onboarding,
        }
    }

    /// The screen holding the settings the agent `id` on the machine `host` offers, while the workspace is showing.
    pub fn with_discovery_of(
        self,
        host: Option<String>,
        id: &str,
        found: pm_acp::Discovery,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_discovery_of(host, id, found)),
            onboarding => onboarding,
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
            onboarding => (onboarding, Vec::new()),
        }
    }

    /// The screen holding `files` as those listed for `key`, while the workspace is showing.
    pub fn with_files(self, key: &str, files: Vec<String>, folders: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_files(key, files, folders)),
            onboarding => onboarding,
        }
    }

    /// The screen holding `branches` for the project at `project`, while the workspace is showing.
    pub fn with_branches(self, project: usize, branches: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_branches(project, branches)),
            onboarding => onboarding,
        }
    }

    /// The screen holding the skills found for `agent` in the project at `project`, while the workspace is showing.
    pub fn with_skills(
        self,
        agent: usize,
        project: usize,
        skills: Vec<pm_acp::Slash>,
        names: std::collections::HashSet<String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_skills(agent, project, skills, names))
            }
            onboarding => onboarding,
        }
    }

    /// The screen knowing who the agent at `agent` is signed in as, while the workspace is showing.
    pub fn with_identity(
        self,
        agent: usize,
        default: Option<String>,
        selected: Option<String>,
        naming: bool,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_identity(agent, default, selected, naming))
            }
            onboarding => onboarding,
        }
    }

    /// The screen after the folder of the session at `session` is ready: it
    /// works in `directory`, on `branch` or on a detached `head`.
    pub fn worktree_ready(
        self,
        session: usize,
        directory: &str,
        branch: Option<&str>,
        head: Option<&str>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.worktree_ready(session, directory, branch, head))
            }
            onboarding => onboarding,
        }
    }

    /// The screen after the folder of the session at `session` was looked at again.
    pub fn refreshed(
        self,
        session: usize,
        changes: &[pm_git::Change],
        commits: Option<Vec<String>>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.refreshed(session, changes, commits)),
            onboarding => onboarding,
        }
    }

    /// The screen after the folder of the session at `session` was seen to have `branch` checked out, or a detached `head`.
    pub fn checked_out(self, session: usize, branch: Option<&str>, head: Option<&str>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.checked_out(session, branch, head)),
            onboarding => onboarding,
        }
    }

    /// The screen after `text` is said in the conversation of the session at `session` by the app.
    pub fn noted(self, session: usize, text: &str) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.noted(session, text)),
            onboarding => onboarding,
        }
    }

    /// The box to give keyboard focus after `id` was pressed, when that is the prompt.
    pub fn refocus(id: FocusId) -> Option<FocusId> {
        WorkspaceState::refocus(id)
    }

    /// Whether the page for starting work is showing, which is where the prompt takes keyboard focus unasked.
    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.starting())
    }

    /// Whether the workspace is showing.
    pub fn is_workspace(&self) -> bool {
        matches!(self, Self::Workspace(_))
    }

    /// What the agents have yet to be told, taken off the workspace.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        match self {
            Self::Onboarding(state) => state.take_outbox(),
            Self::Workspace(state) => state.take_outbox(),
        }
    }

    /// The screen after the agent of the session at `session` did `event`.
    pub fn heard(self, session: usize, event: Event) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.heard(session, event)),
            onboarding => onboarding,
        }
    }

    /// Whether the workspace has something to send to the servers.
    pub fn has_outbox(&self) -> bool {
        match self {
            Self::Onboarding(state) => !state.outbox.is_empty(),
            Self::Workspace(state) => !state.outbox.is_empty(),
        }
    }

    /// What the key for the number `digit` presses while the agent's questions are open.
    pub fn inquiry_hotkey(&self, digit: usize) -> Option<FocusId> {
        match self {
            Self::Workspace(state) => state.inquiry_hotkey(digit),
            Self::Onboarding(_) => None,
        }
    }

    /// Whether pressing `id` shows a shell that should then take the keyboard.
    pub fn takes_keyboard(id: FocusId) -> bool {
        WorkspaceState::takes_keyboard(id)
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
        if let Self::Workspace(state) = self {
            state.terminal_output(tab, data);
        }
    }

    /// Takes the shell of the tab `tab` out of the workspace because it ended.
    pub fn terminal_exited(&mut self, tab: u64) {
        if let Self::Workspace(state) = self {
            state.terminal_exited(tab);
        }
    }

    /// Types `data` into the shell showing.
    pub fn type_into_terminal(&mut self, data: Vec<u8>) {
        if let Self::Workspace(state) = self {
            state.type_into_terminal(data);
        }
    }

    /// Shows `text` an input method is composing at the cursor of the shell showing, or stops showing it when empty.
    pub fn compose_in_terminal(&mut self, text: &str) {
        if let Self::Workspace(state) = self {
            state.compose_in_terminal(text);
        }
    }

    /// Scrolls the shell showing `lines` lines back up, or down when negative.
    pub fn scroll_terminal(&mut self, lines: isize) {
        if let Self::Workspace(state) = self {
            state.scroll_terminal(lines);
        }
    }

    /// Tells the shell showing that its window is `columns` by `rows`.
    pub fn fit_terminal(&mut self, columns: u16, rows: u16) {
        if let Self::Workspace(state) = self {
            state.fit_terminal(columns, rows);
        }
    }

    /// Adds a tab for a shell the server already runs for the session at `session`, and returns its name in this window.
    pub fn adopt_terminal(&mut self, session: usize, columns: u16, rows: u16) -> Option<u64> {
        match self {
            Self::Workspace(state) => state.adopt_terminal(session, columns, rows),
            Self::Onboarding(_) => None,
        }
    }

    /// Starts a shell in the open session when it is on the machine `host` and the dock shows none.
    pub fn revive_terminal(&mut self, host: &Option<String>) {
        if let Self::Workspace(state) = self {
            state.revive_terminal(host);
        }
    }

    /// The position of the session on the machine `host` whose folder holds `directory`, the innermost when several do.
    pub fn session_in(&self, host: &Option<String>, directory: &str) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.session_in(host, directory),
            Self::Onboarding(_) => None,
        }
    }

    /// What the shell showing asks of the keys, if one is showing: whether the arrows send the application form, and whether a paste is wrapped in markers.
    pub fn terminal_modes(&self) -> Option<(bool, bool)> {
        match self {
            Self::Workspace(state) => state.terminal_modes(),
            Self::Onboarding(_) => None,
        }
    }

    /// The text marked in the shell showing.
    pub fn terminal_selection(&self) -> Option<String> {
        match self {
            Self::Workspace(state) => state.terminal_selection(),
            Self::Onboarding(_) => None,
        }
    }

    /// Marks text in the shell showing, from `anchor` to `head`, each a line of its window and a column.
    pub fn select_in_terminal(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        if let Self::Workspace(state) = self {
            state.select_in_terminal(anchor, head);
        }
    }

    /// Underlines the web address at `at` of the window of the shell showing, a line of it and a column, or none when `at` is nothing.
    pub fn reach_terminal_link(&mut self, at: Option<(usize, usize)>) {
        if let Self::Workspace(state) = self {
            state.reach_terminal_link(at);
        }
    }

    /// Whether a web address of the shell showing is underlined as what a press opens.
    pub fn reaching_terminal_link(&self) -> bool {
        match self {
            Self::Workspace(state) => state.reaching_terminal_link(),
            Self::Onboarding(_) => false,
        }
    }

    /// Opens in the browser the web address at `at` of the window of the shell showing, a line of it and a column, and says whether there was one.
    pub fn open_terminal_link(&mut self, at: (usize, usize)) -> bool {
        match self {
            Self::Workspace(state) => state.open_terminal_link(at),
            Self::Onboarding(_) => false,
        }
    }

    /// Unmarks the text of the shell showing.
    pub fn clear_terminal_selection(&mut self) {
        if let Self::Workspace(state) = self {
            state.clear_terminal_selection();
        }
    }

    /// Whether an agent is being started, signed in or at work, so the app should keep looking.
    pub fn waiting(&self) -> bool {
        match self {
            Self::Onboarding(state) => state
                .agents
                .iter()
                .any(|status| matches!(status, AgentStatus::Checking | AgentStatus::Working)),
            Self::Workspace(state) => state.waiting(),
        }
    }
}

/// The whole screen for `state` in a window `width` by `height` logical pixels.
pub fn screen(state: &Screen, width: f32, height: f32, shaper: &mut Shaper) -> Node {
    match state {
        Screen::Onboarding(state) => onboarding(state),
        Screen::Workspace(state) => workspace(state, width, height, shaper),
    }
}

/// What the folder picker's window shows for `state` while the picker is open.
pub fn picker_screen(state: &Screen) -> Option<Node> {
    let picker = state.folder_picker()?;
    Some(crate::picking::picker_window(
        picker,
        state
            .menu()
            .filter(|menu| menu.field == pm_components::folder_picker::PICKER_FIELD)
            .and_then(|menu| Some(field_menu(state.field(menu.field)?, menu))),
    ))
}

/// What the settings window, `width` logical pixels across, shows for `state` while settings are open.
pub fn settings_screen(state: &Screen, width: f32) -> Option<Node> {
    match state {
        Screen::Workspace(state) => crate::workspace::settings_window(state, width),
        Screen::Onboarding(_) => None,
    }
}
