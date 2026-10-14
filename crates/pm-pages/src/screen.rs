//! The screen the window shows: onboarding until it is finished, the workspace after.

use pm_acp::Event;
use pm_input::FocusId;
use pm_text::{Edit, Field, Shaper};
use pm_ui::Node;

use crate::onboarding::{ADD, AgentStatus, OnboardingState, PATH, onboarding};
use crate::workspace::{Outgoing, SIDEBAR_WIDTH, WorkspaceState, workspace};
use pm_components::context_menu::ContextMenu;
use pm_components::project::Project;

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
            Self::Workspace(state) if x < SIDEBAR_WIDTH => {
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

    /// Whether a menu of a setting is open.
    pub fn picker_open(&self) -> bool {
        matches!(self, Self::Workspace(state) if state.picker.is_some())
    }

    /// The screen with no setting menu open.
    pub fn without_picker(self) -> Self {
        match self {
            Self::Workspace(mut state) => {
                state.picker = None;
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
    pub fn with_files(self, key: &str, files: Vec<String>) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(state.with_files(key, files)),
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
            Self::Onboarding(_) => Vec::new(),
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
