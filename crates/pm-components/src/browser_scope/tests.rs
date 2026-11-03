//! Tests of the browsers of sessions: each session keeps tabs of its own, which survive switching away and back.

use pm_input::FocusId;

use crate::browser_tabs::SavedTab;
use crate::state::{SESSION_BASE, Tab, WorkspaceState};

/// A workspace open on its first session, with the browser open there on `address`.
fn browsing(address: &str) -> WorkspaceState {
    let mut state = WorkspaceState::sample().after(FocusId(SESSION_BASE));
    state.open_browser(address);
    state
}

/// Tabs opened in one session are not shown in another, and come back when the session is shown again.
#[test]
fn a_session_keeps_its_own_tabs() {
    let mut state = browsing("http://localhost:5173");
    state.browser_tab_opened("http://localhost:3000", None, false, true);
    let before = state.browser_saved().unwrap();
    state = state.after(FocusId(SESSION_BASE + 1));
    assert!(state.browser.is_none());
    state.open_browser("http://localhost:8080");
    assert_eq!(state.browser.as_ref().unwrap().tabs.len(), 1);
    state = state.after(FocusId(SESSION_BASE));
    assert_eq!(state.browser_saved().unwrap(), before);
    assert_eq!(state.browser_saved().unwrap().1, 1);
    state = state.after(FocusId(SESSION_BASE + 1));
    assert_eq!(
        state.browser_saved().unwrap().0[0].address,
        "http://localhost:8080"
    );
}

/// Every tab of the window has a number of its own, so the pages of different sessions never share one.
#[test]
fn tab_numbers_are_unique_across_sessions() {
    let mut state = browsing("http://localhost:5173");
    state = state.after(FocusId(SESSION_BASE + 1));
    state.open_browser("http://localhost:8080");
    state = state.after(FocusId(SESSION_BASE));
    let mut numbers = state.browser_tab_numbers();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), 2);
}

/// Working in another session's browser leaves the page the reader is on, and what is shown, as it was.
#[test]
fn entering_another_sessions_browser_leaves_the_page_alone() {
    let mut state = browsing("http://localhost:5173");
    state = state.after(FocusId(SESSION_BASE + 1));
    state.open_browser("http://localhost:8080");
    state.tab = Tab::Chat;
    let entered = state.browser_scope_enter(0).unwrap();
    assert_eq!(state.browser.as_ref().unwrap().tabs.len(), 1);
    state.open_served_page("http://localhost:9000", None);
    assert_eq!(state.tab, Tab::Browser);
    state.browser_scope_leave(entered);
    assert_eq!(state.tab, Tab::Chat);
    assert_eq!(
        state.browser_saved().unwrap().0[0].address,
        "http://localhost:8080"
    );
    state = state.after(FocusId(SESSION_BASE));
    let tabs = state.browser_saved().unwrap().0;
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0].address, "http://localhost:5173");
}

/// A browser saved for a session that was not shown is put back into that session only.
#[test]
fn a_saved_browser_goes_to_its_session() {
    let mut state = WorkspaceState::sample().after(FocusId(SESSION_BASE));
    let tabs = [SavedTab {
        address: "http://localhost:4000".to_owned(),
        host: None,
        agent: false,
        locked: false,
    }];
    state.restore_session_browser(1, &tabs, 0, false, 0.0);
    assert!(state.browser.is_none());
    assert!(state.session_browser_saved(1).is_some());
    assert!(state.session_browser_saved(0).is_none());
    state = state.after(FocusId(SESSION_BASE + 1));
    assert_eq!(
        state.browser_saved().unwrap().0[0].address,
        "http://localhost:4000"
    );
}

/// What a page of a session that is not shown reports reaches the tab in that session's browser, and the shown browser is left alone.
#[test]
fn a_report_reaches_the_tab_of_a_session_that_is_not_shown() {
    let mut state = browsing("http://localhost:5173");
    let hidden = state.browser.as_ref().unwrap().active_id();
    state = state.after(FocusId(SESSION_BASE + 1));
    state.open_browser("http://localhost:8080");
    state.browser_scoped_to_tab(hidden, |state| state.browser_zoomed(hidden, 1.5));
    assert_eq!(state.browser.as_ref().unwrap().zoom, 1.0);
    assert_eq!(state.browser_tab_anywhere(hidden).unwrap().zoom, 1.5);
    assert!(state.browser_scoped_to_tab(u32::MAX, |_| ()).is_none());
}
