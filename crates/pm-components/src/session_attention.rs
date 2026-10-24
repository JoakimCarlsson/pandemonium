//! Read acknowledgements and explicit unread reminders for workspace conversations.

use crate::state::WorkspaceState;

impl WorkspaceState {
    /// Whether observing this frame would acknowledge an update or release an unread reminder.
    pub fn conversation_observation_pending(
        &self,
        visible: bool,
        at_end: bool,
        focused: bool,
    ) -> bool {
        let open = if visible { self.open_index() } else { None };
        self.sessions.iter().enumerate().any(|(index, session)| {
            if Some(index) != open {
                session.hold_unread
            } else {
                at_end && focused && session.unread() && !session.hold_unread
            }
        })
    }

    /// Records whether the rendered conversation shows its latest update in a focused window.
    pub fn observe_conversation(&mut self, visible: bool, at_end: bool, focused: bool) -> bool {
        let open = if visible { self.open_index() } else { None };
        let mut changed = false;
        for (index, session) in self.sessions.iter_mut().enumerate() {
            if Some(index) != open {
                changed |= session.hold_unread;
                session.hold_unread = false;
            } else if at_end && focused && session.unread() && !session.hold_unread {
                session.mark_read();
                changed = true;
            }
        }
        changed
    }
}

/// Behavioral coverage for workspace attention and read acknowledgements.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrange::SESSION_MENU_READ;
    use crate::sample::{Session, Status};
    use crate::state::{Page, SESSION_BASE};
    use pm_acp::{Event, Permission};
    use pm_input::FocusId;

    /// A fresh session whose server events can be replayed deterministically.
    fn session() -> Session {
        Session::restored("Task", 0, "task", "/project", None, 0, false)
    }

    /// Completion is unread even without changes, and reading an error retains its explanation.
    #[test]
    fn responses_and_errors_return_to_idle_after_reading() {
        let mut held = session();
        assert_eq!(held.shown(), Status::Idle);
        held.hear(Event::Said("Answer".into()));
        held.hear(Event::Finished(None));
        assert_eq!(held.shown(), Status::Unread);
        held.mark_read();
        assert_eq!(held.shown(), Status::Idle);
        held.hear(Event::Failed("Connection lost".into()));
        assert_eq!(held.shown(), Status::Error);
        assert!(
            held.messages
                .last()
                .unwrap()
                .chunks()
                .iter()
                .any(|(_, text)| text.contains("Connection lost"))
        );
        held.mark_read();
        assert_eq!(held.shown(), Status::Idle);
        assert!(
            held.messages
                .last()
                .unwrap()
                .text
                .contains("Connection lost")
        );
        held.hear(Event::Said("Recovered".into()));
        held.hear(Event::Finished(None));
        assert_eq!(held.shown(), Status::Unread);
    }

    /// Activity takes precedence over unread content, while questions take precedence over activity.
    #[test]
    fn activity_and_unread_are_independent() {
        let mut held = session();
        held.status = Status::Working;
        held.hear(Event::Said("Progress".into()));
        assert!(held.unread());
        assert_eq!(held.shown(), Status::Working);
        held.hear(Event::Ask(Permission {
            ticket: 1,
            title: "Allow command?".into(),
            command: None,
            plan: None,
            choices: Vec::new(),
        }));
        held.mark_read();
        assert_eq!(held.shown(), Status::NeedsInput);
        assert_eq!(held.status, Status::Working);
    }

    /// Opening a conversation is insufficient while its latest content is offscreen or unfocused.
    #[test]
    fn only_visible_latest_content_is_read() {
        let mut state = WorkspaceState::sample();
        state.sessions[0] = session();
        state = state.heard(0, Event::Finished(None));
        state.page = Page::Session(0);
        state.observe_conversation(false, true, true);
        state.observe_conversation(true, false, true);
        state.observe_conversation(true, true, false);
        assert!(state.sessions[0].unread());
        state.observe_conversation(true, true, true);
        assert!(!state.sessions[0].unread());
        state = state.heard(0, Event::Said("New response".into()));
        assert!(state.sessions[0].unread());
    }

    /// The explicit unread reminder survives redraws until the conversation is left and revisited.
    #[test]
    fn manual_unread_survives_until_return() {
        let mut state = WorkspaceState::sample();
        state.sessions[0] = session();
        state.page = Page::Session(0);
        state = state
            .with_session_menu(0, (0.0, 0.0))
            .after(SESSION_MENU_READ);
        state.observe_conversation(true, true, true);
        assert!(state.sessions[0].unread());
        state.page = Page::Project(0);
        state.observe_conversation(false, false, true);
        state = state.after(FocusId(SESSION_BASE));
        state.observe_conversation(true, true, true);
        assert!(!state.sessions[0].unread());
        state = state
            .with_session_menu(0, (0.0, 0.0))
            .after(SESSION_MENU_READ);
        assert!(state.sessions[0].unread());
        state = state
            .with_session_menu(0, (0.0, 0.0))
            .after(SESSION_MENU_READ);
        assert!(!state.sessions[0].unread());
    }

    /// Returning to a session restores its scroll position rather than acknowledging unseen updates.
    #[test]
    fn switching_sessions_preserves_scrollback() {
        let mut state = WorkspaceState::sample().after(FocusId(SESSION_BASE));
        state = state.scrolled_chat(-200.0, 500.0);
        state = state
            .after(FocusId(SESSION_BASE + 1))
            .after(FocusId(SESSION_BASE));
        assert_eq!(state.chat_back, 200.0);
    }

    /// Saved acknowledgements survive replay and a subsequent response becomes unread.
    #[test]
    fn receipts_survive_replay() {
        let mut original = session();
        original.hear(Event::Said("Answer".into()));
        original.hear(Event::Finished(None));
        original.mark_read();
        let mut restored = session();
        restored.read_through = original.read_through;
        restored.hear(Event::Said("Answer".into()));
        restored.hear(Event::Finished(None));
        assert!(!restored.unread());
        restored.hear(Event::Failed("New error".into()));
        assert_eq!(restored.shown(), Status::Error);
    }
}
