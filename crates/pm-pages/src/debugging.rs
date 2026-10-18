//! What the screen does for debugging: the breakpoints set in the gutter, and the program the server debugs.

use pm_protocol::{DebugAction, DebugAdapter, DebugEvent, DebugProcess, DebugScenario, DebugView};

use crate::screen::Screen;
use crate::workspace::WorkspaceState;

impl Screen {
    /// The screen with `change` made to the workspace, when the workspace is showing.
    fn in_workspace_debugging(self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) -> Self {
        match self {
            Self::Workspace(state) => Self::Workspace(change(state)),
            onboarding => onboarding,
        }
    }

    /// The screen starting to debug, listing what the folder can be debugged as at `at`, or running the paused program on.
    pub fn debug_started(self, at: (f32, f32)) -> Self {
        self.in_workspace_debugging(|state| state.debug_started(at))
    }

    /// The screen listing the programs a debugger can attach to, at `at`.
    pub fn debug_attach_requested(self, at: (f32, f32)) -> Self {
        self.in_workspace_debugging(|state| state.debug_attach_requested(at))
    }

    /// The screen after `action` was asked of the program being debugged.
    pub fn debug_do(self, action: DebugAction) -> Self {
        self.in_workspace_debugging(|state| state.debug_do(action))
    }

    /// The screen with the breakpoint on the caret's line set or cleared.
    pub fn breakpoint_toggled_at_caret(self) -> Self {
        self.in_workspace_debugging(WorkspaceState::breakpoint_toggled_at_caret)
    }

    /// The screen after the server listed what the folder of the session at `session` can be debugged as.
    pub fn with_debug_scenarios(self, session: usize, scenarios: Vec<DebugScenario>) -> Self {
        self.in_workspace_debugging(|state| state.with_debug_scenarios(session, scenarios))
    }

    /// The screen after the server listed the programs that can be attached to.
    pub fn with_debug_processes(
        self,
        session: usize,
        adapters: Vec<DebugAdapter>,
        processes: Vec<DebugProcess>,
    ) -> Self {
        self.in_workspace_debugging(|state| {
            state.with_debug_processes(session, adapters, processes)
        })
    }

    /// The screen after the server started debugging for the session at `session`, or said it could not.
    pub fn with_debug_started(
        self,
        session: usize,
        started: Result<(u64, DebugView), String>,
    ) -> Self {
        self.in_workspace_debugging(|state| state.with_debug_started(session, started))
    }

    /// The screen after the program numbered `debug` moved on.
    pub fn with_debug(self, debug: u64, view: DebugView, events: Vec<DebugEvent>) -> Self {
        self.in_workspace_debugging(|state| state.with_debug(debug, view, events))
    }

    /// The screen after the server said it no longer knows the program numbered `debug`.
    pub fn with_debug_lost(self, debug: u64) -> Self {
        self.in_workspace_debugging(|state| state.with_debug_lost(debug))
    }

    /// The screen after one of the debug tab's lists was scrolled.
    pub fn scrolled_debug(self, list: pm_input::FocusId, delta: f32, room: f32) -> Self {
        self.in_workspace_debugging(|state| state.scrolled_debug(list, delta, room))
    }

    /// The number the server knows the program being debugged by, and the session it is debugged in, while there is one.
    pub fn debug_run(&self) -> Option<(u64, usize)> {
        match self {
            Self::Workspace(state) => state.debug.run.as_ref().map(|run| (run.debug, run.session)),
            Self::Onboarding(_) => None,
        }
    }
}
