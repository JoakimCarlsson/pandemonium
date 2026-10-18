//! What the screen does for tasks: listing them, running them, and choosing the toolchains they run with.

use pm_protocol::{Task, ToolchainInfo};

use crate::screen::Screen;
use crate::workspace::WorkspaceState;

impl Screen {
    /// The screen with `change` made to the workspace, when the workspace is showing.
    fn in_workspace_tasking(self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(change(state)),
            onboarding => onboarding,
        }
    }

    /// The screen listing the tasks of the project at `at`, to run one.
    pub fn tasks_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace_tasking(|state| state.tasks_requested(at))
    }

    /// The screen listing the toolchains installed at `at`, to choose one.
    pub fn toolchains_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace_tasking(|state| state.toolchains_requested(at))
    }

    /// The screen running again what was run last.
    pub fn task_rerun(self) -> Self {
        self.in_workspace_tasking(WorkspaceState::task_rerun)
    }

    /// The screen after the server listed the tasks of the folder of the session at `session`.
    pub fn with_tasks(self, session: usize, tasks: Vec<Task>, problem: Option<String>) -> Self {
        self.in_workspace_tasking(|state| state.with_tasks(session, tasks, problem))
    }

    /// The screen after the server listed the toolchains of the machine of the session at `session`.
    pub fn with_toolchains(self, session: usize, toolchains: Vec<ToolchainInfo>) -> Self {
        self.in_workspace_tasking(|state| state.with_toolchains(session, toolchains))
    }
}
