//! Tests of what the page for starting work hands to the session started from it.

use crate::browser_scope::NO_SCOPE;
use crate::state::{Page, SEND, WorkspaceState, prompt_field};

/// A workspace on the page for starting work in the first project, with its `home` session made, and the position of that session.
fn on_new_page() -> (WorkspaceState, usize) {
    let mut state = WorkspaceState::sample();
    state.start_in(0, None);
    state.read_home(0);
    state.rescope_browser();
    let home = state.review_index().expect("the page has a home session");
    (state, home)
}

/// Sends `text` from the page being shown and returns the state afterwards.
fn sent(mut state: WorkspaceState, text: &str) -> WorkspaceState {
    state.prompt = prompt_field(text);
    let mut state = state.after(SEND);
    state.rescope_browser();
    state
}

/// Starting a session from the page for starting work moves its browser and its shells into the new session and leaves the page with neither.
#[test]
fn starting_a_session_moves_the_browser_and_the_shells() {
    let (mut state, home) = on_new_page();
    state.open_browser("http://localhost:5173");
    state.browser_tab_opened("http://localhost:3000", None, false, true);
    let browser = state.browser_saved().expect("the page has a browser");
    let numbers = state.browser_tab_numbers();
    let first = state.add_terminal(home, 80, 24).unwrap();
    let second = state.add_terminal(home, 80, 24).unwrap();
    state.sessions[home].terminal_active = 0;
    state.sessions[home].terminals[0]
        .emulator
        .write()
        .feed(b"serving on 5173");
    state.terminal = true;

    let mut state = sent(state, "Fix the header");

    let Page::Session(started) = state.page else {
        panic!("the page did not move to the new session");
    };
    assert_eq!(state.browser_scope, state.sessions[started].scope);
    assert_eq!(state.browser_saved().unwrap(), browser);
    assert_eq!(state.browser_tab_numbers(), numbers);
    let session = &state.sessions[started];
    let ids: Vec<u64> = session.terminals.iter().map(|tab| tab.id).collect();
    assert_eq!(ids, [first, second]);
    assert_eq!(session.terminal_active, 0);
    let line: String = session.terminals[0]
        .emulator
        .read()
        .line(0, 0)
        .iter()
        .map(|cell| cell.ch)
        .collect();
    assert!(line.starts_with("serving on 5173"));
    assert_eq!(state.session_of_terminal(first), Some(started));
    assert_eq!(state.session_of_terminal(second), Some(started));
    assert!(state.sessions[home].terminals.is_empty());
    assert!(state.terminal);
    assert!(!state.browsers.contains_key(&NO_SCOPE));

    state.page = Page::New(0);
    state.rescope_browser();
    assert!(state.browser.is_none());
    state.page = Page::Project(0);
    state.rescope_browser();
    assert!(state.browser.is_none());
    assert!(state.docked().unwrap().terminals.is_empty());
}

/// Starting a session from a page with no browser and no shells gives the new session neither.
#[test]
fn starting_a_session_with_nothing_open_moves_nothing() {
    let (state, home) = on_new_page();
    let state = sent(state, "Fix the header");
    let Page::Session(started) = state.page else {
        panic!("the page did not move to the new session");
    };
    assert!(state.browser.is_none());
    assert!(state.sessions[started].terminals.is_empty());
    assert!(state.sessions[home].terminals.is_empty());
}

/// A prompt sent in a session that exists already takes nothing from the page for starting work.
#[test]
fn a_prompt_in_an_existing_session_moves_nothing() {
    let (mut state, home) = on_new_page();
    state.open_browser("http://localhost:5173");
    let shell = state.add_terminal(home, 80, 24).unwrap();
    let mut state = sent(state, "Fix the header");
    let Page::Session(started) = state.page else {
        panic!("the page did not move to the new session");
    };
    state.page = Page::New(0);
    state.rescope_browser();
    state.open_browser("http://localhost:8080");
    let left = state.add_terminal(home, 80, 24).unwrap();
    state.page = Page::Session(started);
    state.rescope_browser();
    state.sessions[started].status = crate::sample::Status::Idle;
    let state = sent(state, "And the footer");
    assert_eq!(state.page, Page::Session(started));
    assert_eq!(state.browser.as_ref().unwrap().tabs.len(), 1);
    assert_eq!(state.session_of_terminal(shell), Some(started));
    assert_eq!(state.session_of_terminal(left), Some(home));
    assert_eq!(state.browsers.get(&NO_SCOPE).unwrap().tabs.len(), 1);
}
