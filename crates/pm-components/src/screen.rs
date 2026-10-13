//! The screen the window shows: onboarding until it is finished, the workspace after.

use pm_input::FocusId;
use pm_text::{Edit, Field};
use pm_ui::Node;

use crate::context_menu::ContextMenu;
use crate::onboarding::{ADD, AgentStatus, OnboardingState, PATH, onboarding};
use crate::project::Project;
use crate::workspace::{SIDEBAR_WIDTH, WorkspaceState, workspace};

/// Which screen is showing, with what it remembers.
#[derive(Clone, Debug, PartialEq)]
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

    /// The screen after the folder at `path` is chosen as a project.
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

    /// Whether an agent is being started or signed in, so the screen should keep redrawing.
    pub fn waiting(&self) -> bool {
        match self {
            Self::Onboarding(state) => state
                .agents
                .iter()
                .any(|status| matches!(status, AgentStatus::Checking | AgentStatus::Working)),
            Self::Workspace(_) => false,
        }
    }
}

/// The whole screen for `state` in a window `width` logical pixels wide.
pub fn screen(state: &Screen, width: f32) -> Node {
    match state {
        Screen::Onboarding(state) => onboarding(state),
        Screen::Workspace(state) => workspace(state, width),
    }
}
