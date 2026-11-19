//! Tests of the question about clearing cookies and site data.

use crate::areas::BrowserEvent;
use crate::browser::Command;
use crate::state::WorkspaceState;
use crate::target::Target;

/// What the engine has not yet been told to do, without the tabs it is for.
fn commands(state: &mut WorkspaceState) -> Vec<Command> {
    state
        .take_browser_commands()
        .into_iter()
        .map(|(_, command)| command)
        .collect()
}

/// A browser that has been asked to clear and has not been answered.
fn asked() -> WorkspaceState {
    let mut state = WorkspaceState::sample();
    state.open_browser("http://localhost:5173");
    commands(&mut state);
    state.browser_clear_asked();
    state
}

/// Nothing is cleared before the reader agrees, and declining clears nothing.
#[test]
fn declining_clears_nothing() {
    let mut state = asked();
    assert!(state.browser_clearing());
    assert!(commands(&mut state).is_empty());
    let mut state = state.browser_pressed(Target::Browser(BrowserEvent::BrowserClearCancel));
    assert!(!state.browser_clearing());
    assert!(commands(&mut state).is_empty());
}

/// Agreeing queues one clear and closes the question.
#[test]
fn confirming_clears_once() {
    let mut state = asked().browser_pressed(Target::Browser(BrowserEvent::BrowserClearConfirm));
    assert!(!state.browser_clearing());
    assert_eq!(commands(&mut state), vec![Command::ClearData]);
    let mut state = state.browser_pressed(Target::Browser(BrowserEvent::BrowserClearConfirm));
    assert!(commands(&mut state).is_empty());
}
