//! The commands that take the conversation of the session shown back or start it over, as the list of commands carries them out: answering the last prompt again, starting afresh, handing the work over to a new conversation and asking a question on the side.

use crate::state::{Outgoing, Page, WorkspaceState, prompt_field};

impl WorkspaceState {
    /// The state with the agent of the session shown asked to answer the reader's last prompt again; the server queues it behind a turn that is running.
    pub fn retry_requested(self) -> Self {
        self.conversation_command(pm_harness::RETRY)
    }

    /// The state with the agent of the session shown asked to start its conversation afresh, keeping its settings; the server queues it behind a turn that is running.
    pub fn fresh_requested(self) -> Self {
        self.conversation_command(pm_harness::FRESH)
    }

    /// The state with the prompt of the session shown starting with `/handoff `, for the reader to say what the new conversation should work toward, or to send it as it is to carry on the same work.
    pub fn handoff_requested(self) -> Self {
        self.prompt_started(&format!("{} ", pm_harness::HANDOFF))
    }

    /// The state with the prompt of the session shown starting with `/btw `, for the reader to write the question to ask on the side.
    pub fn btw_requested(self) -> Self {
        self.prompt_started(&format!("{} ", pm_harness::BTW))
    }

    /// The state with `text`, one of the agent's conversation commands, handed to the agent of the session shown as a prompt with no files; nothing changes when no session is shown.
    fn conversation_command(mut self, text: &str) -> Self {
        let Page::Session(session) = self.page else {
            return self;
        };
        if self.sessions.get(session).is_none() {
            return self;
        }
        self.outbox.push(Outgoing::Prompt {
            session,
            text: text.to_owned(),
            files: Vec::new(),
            now: false,
        });
        self
    }

    /// The state with the prompt of the session shown holding `typed`, the caret after it, when a session is shown; whatever was typed before is replaced.
    fn prompt_started(mut self, typed: &str) -> Self {
        if !matches!(self.page, Page::Session(_)) {
            return self;
        }
        self.prompt = prompt_field(typed);
        self.reset_slash();
        self
    }
}

#[cfg(test)]
/// Tests for the conversation commands of the list of commands.
mod tests {
    use crate::sample::Session;
    use crate::state::{Outgoing, Page, WorkspaceState};

    /// A state showing one session.
    fn showing_a_session() -> WorkspaceState {
        let mut state = WorkspaceState::new(&[]);
        state.sessions = vec![Session::started("Test", 0, "Hello")];
        state.page = Page::Session(0);
        state
    }

    /// Retrying and starting afresh hand the agent their commands as prompts; handing off and asking on the side start the prompt with theirs; with no session shown nothing is sent.
    #[test]
    fn conversation_commands_reach_the_agent() {
        let mut state = showing_a_session().retry_requested().fresh_requested();
        let sent: Vec<String> = state
            .take_outbox()
            .into_iter()
            .filter_map(|outgoing| match outgoing {
                Outgoing::Prompt {
                    session: 0, text, ..
                } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(sent, ["/retry", "/fresh"]);
        assert_eq!(state.handoff_requested().prompt.text(), "/handoff ");
        assert_eq!(showing_a_session().btw_requested().prompt.text(), "/btw ");
        let mut idle = WorkspaceState::new(&[]).retry_requested();
        assert!(
            !idle
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::Prompt { .. }))
        );
    }
}
