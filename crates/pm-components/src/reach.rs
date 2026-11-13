//! Whether the window can hear the server each session runs on.
//!
//! Losing the connection to a server ends nothing on it: its agents keep
//! working. The window only stops showing those sessions as at work, since it
//! cannot hear them, and once the connection is made anew the events the
//! server replays decide again where each session stands.

use crate::state::WorkspaceState;

impl WorkspaceState {
    /// The state after the connection to the server on `host` dropped, with that machine's sessions shown as out of reach.
    pub fn host_lost(self, host: &Option<String>) -> Self {
        self.with_host_reach(host, false)
    }

    /// The state after the connection to the server on `host` was made anew, with that machine's sessions shown as its events say again.
    pub fn host_reached(self, host: &Option<String>) -> Self {
        self.with_host_reach(host, true)
    }

    /// The state with the sessions of the projects on `host` marked as heard when `reached`, and as out of reach when not.
    fn with_host_reach(mut self, host: &Option<String>, reached: bool) -> Self {
        for position in 0..self.sessions.len() {
            if self.is_on(&self.sessions[position], host) {
                self.sessions[position].out_of_reach = !reached;
                if !reached {
                    self.sessions[position].handing_over = false;
                }
            }
        }
        self
    }
}

/// Behaviour of sessions whose server drops out of reach and comes back.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use crate::sample::{Session, Status};
    use pm_harness::Event;

    /// The machine the remote project of [`workspace`] is on.
    const REMOTE: &str = "build";

    /// A workspace with a project on this machine and one on [`REMOTE`], each with a session whose agent is at work on a turn.
    fn workspace() -> WorkspaceState {
        let projects = [
            Project::from_source("/home/reader/here").unwrap(),
            Project::from_source(&format!("ssh://{REMOTE}/home/reader/there")).unwrap(),
        ];
        let mut state = WorkspaceState::new(&projects);
        state.sessions = (0..projects.len())
            .map(|project| {
                let mut session =
                    Session::restored("Task", project, "task", "/project", None, 0, false);
                session.hear(Event::Prompted {
                    text: "Work".to_owned(),
                    files: Vec::new(),
                    checkpoint: false,
                    turn: 1,
                    saved: None,
                });
                session
            })
            .collect();
        state
    }

    /// The machine of [`REMOTE`], as the connections name it.
    fn remote() -> Option<String> {
        Some(REMOTE.to_owned())
    }

    /// Losing the connection to one machine shows only that machine's sessions as out of reach and leaves the turn open.
    #[test]
    fn disconnect_clears_working_for_that_host_only() {
        let state = workspace().host_lost(&remote());
        assert_eq!(state.sessions[0].shown(), Status::Working);
        assert_eq!(state.sessions[1].shown(), Status::Unreachable);
        assert_eq!(state.sessions[1].status, Status::Working);
        let state = state.host_lost(&None);
        assert_eq!(state.sessions[0].shown(), Status::Unreachable);
    }

    /// A turn the server ended while the window could not hear it shows as ended once its events are replayed.
    #[test]
    fn reconnect_with_finished_turn_is_not_working() {
        let state = workspace()
            .host_lost(&remote())
            .host_reached(&remote())
            .heard(1, Event::Halted)
            .heard(
                1,
                Event::Finished(Some(
                    "the server stopped while the agent was working".to_owned(),
                )),
            );
        let session = &state.sessions[1];
        assert_ne!(session.status, Status::Working);
        assert_eq!(session.shown(), Status::Error);
        assert!(!session.out_of_reach);
    }

    /// A turn the server is still running shows as at work again once the connection is back.
    #[test]
    fn reconnect_with_open_turn_is_working() {
        let state = workspace()
            .host_lost(&remote())
            .host_reached(&remote())
            .heard(1, Event::Said("Still going".to_owned()));
        assert_eq!(state.sessions[1].shown(), Status::Working);
        assert_eq!(state.sessions[0].shown(), Status::Working);
    }
}
