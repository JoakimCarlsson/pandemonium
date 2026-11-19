//! The workspace screen and the actions and auxiliary windows it exposes.

use pm_components::areas::{ChatEvent, DialogEvent, EditorEvent, GitEvent};
use pm_components::events::Change;
use pm_components::events::Event;
use pm_components::state::Asked;
use pm_components::target::Target;
use pm_harness::Event as HarnessEvent;
use rustle::TextFieldState;

use crate::workspace::{Outgoing, WorkspaceState, workspace};
use pm_components::context_menu::{ContextMenu, field_menu};
use pm_components::folder_picker::FolderPicker;
use pm_components::git_controls::Pick;
use pm_components::git_menus::MenuKind;
use pm_components::project::Project;
use pm_components::sash::Grows;
use pm_components::tree_actions::{TreeShortcut, TreeTarget};
use pm_components::{Window, shadowed};

/// Which screen is showing, with what it remembers.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    /// The projects, their sessions and the page being read.
    Workspace(WorkspaceState),
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

    /// The screen with what the reader chose for the agents' settings before.
    pub fn with_choices(self, choices: Vec<(String, String, String)>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_choices(choices)),
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

    /// Takes what the control pressed last asks of the app, leaving nothing asked.
    pub fn take_asked(&mut self) -> Option<Asked> {
        match self {
            Self::Workspace(state) => state.take_asked(),
        }
    }

    /// The screen after `event`, as the workspace's reducer takes it.
    pub fn handle(self, event: Event) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.handle(event)),
        }
    }

    /// Whether the field `id` saves what was written in it when the reader leaves it.
    pub fn saves_on_leaving(id: Target) -> bool {
        WorkspaceState::saves_on_leaving(id)
    }

    /// The code editor's field, which `id` names, while a text file is shown.
    pub fn field(&self, id: Target) -> Option<&pm_field::Field> {
        match self {
            Self::Workspace(state) => state.field(id),
        }
    }

    /// The state of the text field `id` names, if it names one other than the code editor's.
    pub fn text_field(&self, id: Target) -> Option<&TextFieldState> {
        match self {
            Self::Workspace(state) => state.text_field(id),
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

    /// The message of the open session whose area is under the window point `at`, if any.
    pub fn message_at(&self, at: (f32, f32)) -> Option<usize> {
        self.places().shown().into_iter().find_map(|(id, rect)| {
            let inside = at.0 >= rect.origin.x
                && at.1 >= rect.origin.y
                && at.0 < rect.origin.x + rect.size.width
                && at.1 < rect.origin.y + rect.size.height;
            match id {
                Target::Chat(ChatEvent::MessageArea(index)) if inside => Some(index),
                _ => None,
            }
        })
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

    /// Whether `id` names a text field of either screen.
    pub fn is_field(id: Target) -> bool {
        WorkspaceState::is_field(id)
    }

    /// Whether `id` names a text field other than the code editor's: one a rustle text field edits.
    pub fn is_text_field(id: Target) -> bool {
        WorkspaceState::is_field(id) && id != Target::Editor(EditorEvent::EditorField)
    }

    /// Whether the field `id` holds several lines, so Enter can break a line in it.
    pub fn is_multiline(id: Target) -> bool {
        WorkspaceState::is_multiline(id)
    }

    /// Whether the field `id` hides what it holds and keeps it off the clipboard.
    pub fn is_secure(id: Target) -> bool {
        WorkspaceState::is_secure(id)
    }

    /// What pressing Enter inside the field `id` presses.
    pub fn submit_of(id: Target) -> Target {
        WorkspaceState::submit_of(id)
    }

    /// What pressing Control or Command with Enter inside the field `id` presses, when it presses something of its own.
    pub fn hurried_submit_of(id: Target) -> Option<Target> {
        WorkspaceState::hurried_submit_of(id)
    }

    /// Whether the panel for adding a project is open.
    pub fn adder_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.adder_open())
    }

    /// The screen with the menu of the `index`-th server of the settings page open at `at`.
    pub fn with_settings_menu(self, index: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_settings_menu(index, at)),
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
        }
    }

    /// What pressing Enter presses while the panel for adding a project is open.
    pub fn adder_enter(&self) -> Option<Target> {
        match self {
            Self::Workspace(state) => state.adder_enter(),
        }
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

    /// The screen with the folder picker put away, which is what closing its window does.
    pub fn without_folder_picker(self) -> Self {
        match self {
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
            Self::Workspace(state) => Self::Workspace(state.with_folders(requested, listed)),
        }
    }

    /// The menu open over a field, and where, if one is.
    pub fn menu(&self) -> Option<ContextMenu> {
        match self {
            Self::Workspace(state) => state.menu,
        }
    }

    /// The screen with a menu open over the field `field` at `at`.
    pub fn with_menu(self, field: Target, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_menu(field, at)),
        }
    }

    /// The screen with the menu of the project at `project` open at `at`, while the workspace is showing.
    pub fn with_project_menu(self, project: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_project_menu(project, at)),
        }
    }

    /// Which project's header `id` is, while the workspace is showing.
    pub fn header_of(&self, id: Target) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.header_of(id),
        }
    }

    /// The project the element `id` belongs to, while the workspace is showing.
    pub fn project_at(&self, id: Target) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.project_at(id),
        }
    }

    /// The screen with the project at `from` being dragged to the place of the one at `over`, with the pointer at `at`.
    pub fn dragged_over(self, from: usize, over: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.dragged_over(from, over, at)),
        }
    }

    /// The screen after a dragged project was let go.
    pub fn dropped(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.dropped()),
        }
    }

    /// The screen with the menu of panes open with its corner at `at`.
    pub fn with_pane_menu(self, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_pane_menu(at)),
        }
    }

    /// The tab of a pane `id` is, while the workspace is showing.
    pub fn pane_tab_of(&self, id: Target) -> Option<pm_components::panes::Item> {
        match self {
            Self::Workspace(state) => state.pane_tab_of(id),
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
        }
    }

    /// The screen after a dragged tab was let go.
    pub fn pane_dropped(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.pane_dropped()),
        }
    }

    /// Whether a menu is open over a project, a session or a message, so that a press elsewhere has to close it.
    pub fn list_menu_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.project_menu.is_some() || state.session_menu.is_some() || state.message_menu.is_some() || state.terminal_menu.is_some() || state.browser_menu.is_some() || state.git.menu.is_some() || state.settings_servers.menu.is_some() || state.tab_menu.is_some() || state.pane_menu.is_some())
    }

    /// The screen with the menu over the message at `message` of the open
    /// session open at `at`, offering to copy the selection when `selected`,
    /// while the workspace is showing.
    pub fn with_message_menu(self, message: usize, at: (f32, f32), selected: bool) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_message_menu(message, at, selected))
            }
        }
    }

    /// The screen with the menu over the shell showing open at `at`, while the workspace is showing.
    pub fn with_terminal_menu(self, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_terminal_menu(at)),
        }
    }

    /// The screen with the menu over the page of the browser open at `at`, while the workspace is showing.
    pub fn with_browser_menu(self, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_browser_menu(at)),
        }
    }

    /// The screen after the mode of picking an element of the page of tab `tab` of the browser ended, with the pick held for the prompt when there was one.
    pub fn browser_picked(
        self,
        tab: u32,
        element: Option<pm_components::elements::Element>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.browser_picked(tab, element)),
        }
    }

    /// The screen with the menu of what can be added to the prompt from the browser's page open at `at`, while the workspace is showing.
    pub fn with_browser_capture_menu(self, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_browser_capture_menu(at)),
        }
    }

    /// The screen after the helper reported a screenshot or the console of the browser's page for the prompt, or the end of choosing an area without one.
    pub fn browser_captured(
        self,
        tab: u32,
        capture: Option<pm_components::captures::Capture>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.browser_captured(tab, capture)),
        }
    }

    /// The screen after `command` was carried out on the page of the browser, which closes the menu over it.
    pub fn browser_command(self, command: pm_components::browser_menu::BrowserCommand) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.browser_command(command)),
        }
    }

    /// The screen after `command` was carried out on the shell showing, which closes the menu over it, while the workspace is showing.
    pub fn terminal_command(self, command: pm_components::terminal_menu::TerminalCommand) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.terminal_command(command)),
        }
    }

    /// The screen with the menu of the session at `session` open at `at`.
    pub fn with_session_menu(self, session: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_session_menu(session, at)),
        }
    }

    /// The screen with the session at `position` closed, by the server or because no server holds it.
    pub fn closed_elsewhere(self, position: usize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.closed_session(position, false, false)),
        }
    }

    /// The screen with the reader asked whether to close the session at `position` though its worktree has changes that are not committed.
    pub fn asking_to_close(self, position: usize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.asking_to_close(position)),
        }
    }

    /// The screen moved `delta` places along the pages and views visited, towards the past when negative.
    pub fn navigated(self, delta: isize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.navigated(delta)),
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

    /// The screen with no menu open.
    pub fn without_menu(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.without_menu()),
        }
    }

    /// The screen after the reader chose the folder at `path` as a project: added, and open on a new session in it.
    pub fn with_project_opened(self, path: &str) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_project_opened(path)),
        }
    }

    /// The screen with the project at `path` added, which the reader did not ask to see.
    pub fn with_project(self, path: &str) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_project(path)),
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

    /// Whether the pointer holds the bar of one of the screen's scrolling areas.
    pub fn scrollbar_held(&self) -> bool {
        let Self::Workspace(state) = self;
        state.scrollbar_held()
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

    /// Which changed file's row `id` is, while the workspace is showing.
    pub fn change_row_of(&self, id: Target) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.change_row_of(id),
        }
    }

    /// The file or folder of the project's files row `id` is, while the workspace is showing.
    pub fn tree_row_of(&self, id: Target) -> Option<TreeTarget> {
        match self {
            Self::Workspace(state) => state.tree_row_of(id),
        }
    }

    /// The screen with the menu on the project's files open at `at` over `target`.
    pub fn with_tree_menu(self, target: TreeTarget, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_tree_menu(target, at)),
        }
    }

    /// The screen after the server answered a write of the file at `path` of the session at `session`.
    pub fn with_file_written(self, session: usize, path: &str, failed: bool) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_file_written(session, path, failed))
            }
        }
    }

    /// The screen with the file shown scrolled to show its caret, in a view `height` logical pixels tall.
    pub fn with_caret_shown(self, height: f32) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_caret_shown(height)),
        }
    }

    /// The screen after a file's tab or row was pressed twice in quick succession, which keeps the file open.
    pub fn with_file_kept(self, id: Target) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_file_kept(id)),
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
        }
    }

    /// The screen after files were made, renamed, copied or removed and must be listed again.
    pub fn with_tree_stale(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_tree_stale()),
        }
    }

    /// Which folder of the changed files' tree row `id` is, while the workspace is showing.
    pub fn change_folder_row_of(&self, id: Target) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.change_folder_row_of(id),
        }
    }

    /// The screen with the menu on changed files open at `at` over the folder at `position` of their tree.
    pub fn with_folder_change_menu(self, position: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_folder_change_menu(position, at)),
        }
    }

    /// The screen with the menu on changed files open at `at` over the file at `index`.
    pub fn with_change_menu(self, index: usize, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_change_menu(index, at)),
        }
    }

    /// The screen after the changed file at `index` was pressed with the modifier that picks several.
    pub fn change_picked(self, index: usize, how: Pick) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.change_picked(index, how)),
        }
    }

    /// The screen after the tree row `target` was pressed with the modifier that picks several.
    pub fn tree_row_picked(self, target: TreeTarget, how: Pick) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_row_picked(target, how)),
        }
    }

    /// The screen after `shortcut` was pressed over the project's files.
    pub fn tree_shortcut(self, shortcut: TreeShortcut) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_shortcut(shortcut)),
        }
    }

    /// Whether something was cut or copied out of the project's files and waits to be pasted.
    pub fn tree_holding(&self) -> bool {
        match self {
            Self::Workspace(state) => state.tree_holding(),
        }
    }

    /// The screen after the files and folders of this machine at `paths` were dropped on, or pasted over, the row `over` of the project's files, or pasted where the row last picked is.
    pub fn tree_brought_in(self, paths: Vec<String>, over: Option<TreeTarget>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_brought_in(paths, over)),
        }
    }

    /// The screen with the row `from` of the project's files dragged to `at`, over the row `over` if it is over one.
    pub fn tree_dragged(self, from: TreeTarget, over: Option<TreeTarget>, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_dragged(from, over, at)),
        }
    }

    /// The screen with files from outside the window held at `at`, over the row `over` of the project's files if they are over one.
    pub fn tree_hovered(self, over: Option<TreeTarget>, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_hovered(over, at)),
        }
    }

    /// Whether rows of the project's files are being dragged.
    pub fn tree_dragging(&self) -> bool {
        match self {
            Self::Workspace(state) => state.tree_dragging(),
        }
    }

    /// The screen after the rows being dragged were let go, moved into the folder they were over or copied there when `copy`.
    pub fn tree_dropped(self, copy: bool) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_dropped(copy)),
        }
    }

    /// The screen with nothing held over the project's files any more.
    pub fn without_tree_drag(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.without_tree_drag()),
        }
    }

    /// The screen with every row of the project's files that is shown picked.
    pub fn tree_all_picked(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.tree_all_picked()),
        }
    }

    /// The screen with every changed file the list shows picked.
    pub fn changes_all_picked(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.changes_all_picked()),
        }
    }

    /// The screen after the server listed the branches of the folder of the session at `session`.
    pub fn with_session_branches(self, session: usize, names: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_session_branches(session, names)),
        }
    }

    /// The screen asking again for the open files of the session at `session`, which may have changed in the folder.
    pub fn open_files_wanted(self, session: usize) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.open_files_wanted(session)),
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

    /// The screen with every figure let go and those of the file shown asked for again, as when the colours diagrams are drawn in change.
    pub fn without_figures(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.without_figures()),
        }
    }

    /// Whether `id` names a row of the menu of the source control panel that is open.
    pub fn is_menu_row(&self, id: Target) -> bool {
        matches!(self, Self::Workspace(state) if state.is_menu_row(id))
    }

    /// The point at which the menu `kind`, opened from the control occupying `trigger` as `(x, y, width, height)`, lies below it or above it inside a window `window` wide and tall.
    pub fn git_menu_below(
        &self,
        kind: MenuKind,
        trigger: (f32, f32, f32, f32),
        window: (f32, f32),
    ) -> (f32, f32) {
        match self {
            Self::Workspace(state) => state.git_menu_below(kind, trigger, window),
        }
    }

    /// The screen with the menu `kind` open at `at`.
    pub fn with_git_menu(self, kind: MenuKind, at: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_git_menu(kind, at)),
        }
    }

    /// The screen after the server listed the stashes of the repository of the session at `session`.
    pub fn with_stashes(self, session: usize, stashes: Vec<pm_git::Stash>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_stashes(session, stashes)),
        }
    }

    /// The screen after the server answered the question about `branch` of the session at `session` with `info`.
    pub fn with_branch_info(
        self,
        session: usize,
        branch: Option<String>,
        info: pm_git::BranchInfo,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_branch_info(session, branch, info))
            }
        }
    }

    /// How big the panel the handle `id` resizes is now, in a window `window` logical pixels wide and tall, and which way the handle is pulled to grow it.
    pub fn sash(&self, id: Target, window: (f32, f32)) -> Option<(f32, Grows)> {
        match self {
            Self::Workspace(state) => state.sash(id, window),
        }
    }

    /// The screen with the panel the handle `id` resizes made `size` logical pixels big, within what that panel allows in a window `window` logical pixels wide and tall.
    pub fn with_sash_size(self, id: Target, size: f32, window: (f32, f32)) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_sash_size(id, size, window)),
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
        }
    }

    /// The screen after the folder of the session at `session` was seen to hold repositories that stand as `standings` say.
    pub fn with_standings(self, session: usize, standings: Vec<pm_git::Standing>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_standings(session, standings)),
        }
    }

    /// The screen after the server listed the commits of the repository of the session at `session`.
    pub fn with_history(self, session: usize, commits: Vec<pm_git::Commit>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_history(session, commits)),
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

    /// The screen after git refused something asked of the open session's folder, for `reason`: the source control panel says so.
    pub fn git_failed(self, reason: &str) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.git_failed(reason)),
        }
    }

    /// The screen after the server said the branch of the session at `session` has `request` as its newest pull request, or none, on the code host `host`, when it said which.
    pub fn with_pull_request(
        self,
        session: usize,
        request: Option<pm_git::PullRequestStatus>,
        host: Option<pm_git::CodeHost>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_pull_request(session, request, host))
            }
        }
    }

    /// The screen after the server acted on the pull request of the session at `session`, which stands as `request` afterwards unless the action failed for `failure`.
    pub fn pull_request_done(
        self,
        session: usize,
        request: Option<pm_git::PullRequestStatus>,
        failure: Option<String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.pull_request_done(session, request, failure))
            }
        }
    }

    /// The screen after the remote answered the request of the session at `session`.
    pub fn remote_done(self, session: usize, failure: Option<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.remote_done(session, failure)),
        }
    }

    /// The submodule the git panel works on in the folder of the session at `session`, or nothing for the folder's own repository or a session the panel does not show.
    pub fn repository_of(&self, session: usize) -> Option<String> {
        match self {
            Self::Workspace(state) => state.repository_of(session),
        }
    }

    /// Whether the graph is open, and whether it lists every reference.
    pub fn graph_wanted(&self) -> Option<bool> {
        match self {
            Self::Workspace(state) => state.graph_wanted(),
        }
    }

    /// Whether pressing Enter in the field `id` puts a line break in it rather than pressing its button.
    pub fn enter_breaks_line(id: Target) -> bool {
        id == Target::Git(GitEvent::CommitMessage) || id == Target::Editor(EditorEvent::EditorField)
    }

    /// Whether the menu on changed files, the question of whether to discard or the list of branches is open.
    pub fn review_overlay_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.review_overlay_open())
    }

    /// The screen after the pointer went down on the line number `id` of the diff.
    pub fn comment_drag_begun(self, id: Target) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.comment_drag_begun(id)),
        }
    }

    /// The screen after the pointer, held down, moved over the line number `id` of the diff.
    pub fn comment_drag_extended(self, id: Target) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.comment_drag_extended(id)),
        }
    }

    /// The screen after the pointer was let go over the diff.
    pub fn comment_drag_ended(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.comment_drag_ended()),
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
        }
    }

    /// The screen with the highlighted command typed into the prompt.
    pub fn slash_taken(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.slash_taken()),
        }
    }

    /// The screen with the open session's mode changed to the next one.
    pub fn mode_cycled(self) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.mode_cycled()),
        }
    }

    /// Whether a menu of a setting, or the offer to put files back, is open.
    pub fn picker_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.picker.is_some() || state.restoring.is_some() || state.git.branch_menu)
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

    /// The screen after the agent at `agent` told what it lets the reader change and what commands it offers.
    pub fn with_discovery(self, agent: usize, found: pm_harness::Discovery) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_discovery(agent, found)),
        }
    }

    /// The screen after the provider said how much of its allowance the account of the agent at `agent` has used.
    pub fn with_allowance(
        self,
        agent: usize,
        login: Option<String>,
        limits: Option<pm_harness::Limits>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_allowance(agent, login, limits)),
        }
    }

    /// The screen with `paths` mentioned in the prompt, while the workspace is showing.
    pub fn with_attachments(self, paths: &[String]) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_attachments(paths)),
        }
    }

    /// The screen knowing what the picture at `path` looks like, while the workspace is showing.
    pub fn with_preview(self, path: &str, picture: rustle::ImageData) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_preview(path, picture)),
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

    /// The screen holding the settings the agent at `agent` on the machine `host` offers, while the workspace is showing.
    pub fn with_discovery_for(
        self,
        host: Option<String>,
        agent: usize,
        found: pm_harness::Discovery,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_discovery_for(host, agent, found)),
        }
    }

    /// The screen holding the settings the agent `id` on the machine `host` offers, while the workspace is showing.
    pub fn with_discovery_of(
        self,
        host: Option<String>,
        id: &str,
        found: pm_harness::Discovery,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_discovery_of(host, id, found)),
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

    /// The screen holding `files` as those listed for `key`, while the workspace is showing.
    pub fn with_files(
        self,
        key: &str,
        files: Vec<String>,
        folders: Vec<String>,
        ignored: Vec<String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_files(key, files, folders, ignored))
            }
        }
    }

    /// The screen holding `branches` for the project at `project`, while the workspace is showing.
    pub fn with_branches(self, project: usize, branches: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_branches(project, branches)),
        }
    }

    /// The screen knowing who the agent at `agent` is signed in as, while the workspace is showing.
    pub fn with_identity(
        self,
        agent: usize,
        default: Option<String>,
        selected: Option<String>,
        naming: Option<String>,
    ) -> Self {
        match self {
            Self::Workspace(state) => {
                Self::Workspace(state.with_identity(agent, default, selected, naming))
            }
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
        }
    }

    /// The screen after the submodules of the folder of the session at `session` were seen to have `heads` checked out.
    pub fn with_submodule_heads(self, session: usize, heads: Vec<(String, String)>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_submodule_heads(session, heads)),
        }
    }

    /// The screen after the project at `project` was seen to hold the submodules at `paths`.
    pub fn with_submodules(self, project: usize, paths: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_submodules(project, paths)),
        }
    }

    /// The screen after the project at `project` was seen to have, or lack, a `devcontainer.json`.
    pub fn with_devcontainer(self, project: usize, configured: bool) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_devcontainer(project, configured)),
        }
    }

    /// The screen after the folder of the session at `session` was seen to have `branch` checked out, or a detached `head`.
    pub fn checked_out(self, session: usize, branch: Option<&str>, head: Option<&str>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.checked_out(session, branch, head)),
        }
    }

    /// The screen after `text` is said in the conversation of the session at `session` by the app.
    pub fn noted(self, session: usize, text: &str) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.noted(session, text)),
        }
    }

    /// The screen after the server said how continuing the chat at `chat` in a project went: on the session at `started`, or with the chat told why there is none.
    pub fn chat_continued(
        self,
        chat: usize,
        started: Option<usize>,
        failure: Option<&str>,
    ) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.chat_continued(chat, started, failure)),
        }
    }

    /// The box to give keyboard focus after `id` was pressed, when that is the prompt.
    pub fn refocus(id: Target) -> Option<Target> {
        WorkspaceState::refocus(id)
    }

    /// Whether the page for starting work is showing, which is where the prompt takes keyboard focus unasked.
    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.starting())
    }

    /// What the agents have yet to be told, taken off the workspace.
    pub fn take_outbox(&mut self) -> Vec<Outgoing> {
        match self {
            Self::Workspace(state) => state.take_outbox(),
        }
    }

    /// Asks for the diffs of the changed files in or near the view of the diff that the server listed without them.
    pub fn diffs_wanted(&mut self) {
        match self {
            Self::Workspace(state) => state.diffs_wanted(),
        }
    }

    /// The screen after the agent of the session at `session` did `event`.
    pub fn heard(self, session: usize, event: HarnessEvent) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.heard(session, event)),
        }
    }

    /// Takes in what the agent of the session at `session` did, as `event`, without building the screen anew.
    pub fn hear_in_place(&mut self, session: usize, event: HarnessEvent) {
        match self {
            Self::Workspace(state) => state.hear_in_place(session, event),
        }
    }

    /// Whether the workspace has something to send to the servers.
    pub fn has_outbox(&self) -> bool {
        match self {
            Self::Workspace(state) => !state.outbox.is_empty(),
        }
    }

    /// What the key for the number `digit` presses while the agent's questions are open.
    pub fn inquiry_hotkey(&self, digit: usize) -> Option<Target> {
        match self {
            Self::Workspace(state) => state.inquiry_hotkey(digit),
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

    /// Adds a tab for a shell the server already runs for the session at `session`, and returns its name in this window.
    pub fn adopt_terminal(&mut self, session: usize, columns: u16, rows: u16) -> Option<u64> {
        match self {
            Self::Workspace(state) => state.adopt_terminal(session, columns, rows),
        }
    }

    /// The position of the session on the machine `host` whose folder holds `directory`, the innermost when several do.
    pub fn session_in(&self, host: &Option<String>, directory: &str) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.session_in(host, directory),
        }
    }

    /// What the shell showing asks of the keys, if one is showing: whether the arrows send the application form, and whether a paste is wrapped in markers.
    pub fn terminal_modes(&self) -> Option<(bool, bool)> {
        match self {
            Self::Workspace(state) => state.terminal_modes(),
        }
    }

    /// The text marked in the shell showing.
    pub fn terminal_selection(&self) -> Option<String> {
        match self {
            Self::Workspace(state) => state.terminal_selection(),
        }
    }

    /// Marks text in the shell showing, from `anchor` to `head`, each a line of its window and a column.
    pub fn select_in_terminal(&mut self, anchor: (usize, usize), head: (usize, usize)) {
        let Self::Workspace(state) = self;
        state.select_in_terminal(anchor, head);
    }

    /// The line counted from the oldest kept that is the first of the window of the shell showing.
    pub fn terminal_first_line(&self) -> Option<usize> {
        match self {
            Self::Workspace(state) => state.terminal_first_line(),
        }
    }

    /// What `reach` takes around the cell at `at`, a line counted from the oldest kept and a column, of the shell showing, as its first and last cell.
    pub fn terminal_reach(
        &self,
        reach: pm_field::Reach,
        at: (usize, usize),
    ) -> Option<pm_terminal::Span> {
        match self {
            Self::Workspace(state) => state.terminal_reach(reach, at),
        }
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

    /// Whether a web address of the shell showing is underlined as what a press opens.
    pub fn reaching_terminal_link(&self) -> bool {
        match self {
            Self::Workspace(state) => state.reaching_terminal_link(),
        }
    }

    /// Opens in the browser the web address at `at` of the window of the shell showing, a line of it and a column, and says whether there was one.
    pub fn open_terminal_link(&mut self, at: (usize, usize)) -> bool {
        match self {
            Self::Workspace(state) => state.open_terminal_link(at),
        }
    }

    /// Unmarks the text of the shell showing.
    pub fn clear_terminal_selection(&mut self) {
        let Self::Workspace(state) = self;
        state.clear_terminal_selection();
    }

    /// Whether an agent is being started, signed in or at work, so the app should keep looking.
    pub fn waiting(&self) -> bool {
        match self {
            Self::Workspace(state) => state.waiting(),
        }
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
#[track_caller]
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
                field_menu(field, menu, false);
            }
        })
        .build();
}

/// Emits what the settings window, `width` by `height` logical pixels, shows for `state` while settings are open: the root content of that window; emits nothing while settings are closed. The page fills the window's height, so only `width` shapes it.
#[track_caller]
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
                .handle(Event::Sidebar(SidebarEvent::AddProject))
                .adder_open()
        );
        assert!(
            Screen::default()
                .handle(Event::Sidebar(SidebarEvent::Settings))
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
