//! Tests of the tabs of the browser: opening, showing, closing, the lock and what is kept across a restart.

use pm_input::FocusId;

use super::{
    BROWSER_LOCK, BROWSER_NEW_TAB, BROWSER_TAB_CLOSE, BROWSER_TAB_SELECT, SavedTab, changes_page,
};
use crate::browser::Command;
use crate::state::{Tab, WorkspaceState};

/// A workspace with the browser open on one tab.
fn opened() -> WorkspaceState {
    let mut state = WorkspaceState::sample();
    state.open_browser("http://localhost:5173");
    state
}

/// Pressing the button for a new tab adds a blank tab and shows it.
#[test]
fn a_new_tab_is_shown() {
    let state = opened().browser_pressed(BROWSER_NEW_TAB);
    let browser = state.browser.as_ref().unwrap();
    assert_eq!(browser.tabs.len(), 2);
    assert_eq!(browser.active, 1);
    assert_eq!(browser.tabs[1].address.text(), "about:blank");
}

/// Selecting a tab shows it and closing the one shown shows its neighbour; the last tab closes the browser.
#[test]
fn tabs_are_selected_and_closed() {
    let mut state = opened().browser_pressed(BROWSER_NEW_TAB);
    state = state.browser_pressed(FocusId(BROWSER_TAB_SELECT));
    assert_eq!(state.browser.as_ref().unwrap().active, 0);
    state = state.browser_pressed(FocusId(BROWSER_TAB_CLOSE));
    let browser = state.browser.as_ref().unwrap();
    assert_eq!((browser.tabs.len(), browser.active), (1, 0));
    assert_eq!(browser.tabs[0].id, 2);
    state = state.browser_pressed(FocusId(BROWSER_TAB_CLOSE));
    assert!(state.browser.is_none());
    assert_ne!(state.tab, Tab::Browser);
}

/// A tab keeps its number when the tabs before it close.
#[test]
fn numbers_stay_with_tabs() {
    let mut state = opened();
    let second = state.browser_tab_opened("http://localhost:3000", None, true, false);
    assert_eq!(second, Some(2));
    assert!(state.browser_tab_closed(1));
    let browser = state.browser.as_ref().unwrap();
    assert_eq!(browser.active_id(), 2);
    assert!(browser.tab(2).unwrap().agent);
    assert!(!state.browser_tab_closed(1));
}

/// The padlock locks the tab shown and unlocks it, and a tab an agent opened cannot be locked.
#[test]
fn the_lock_keeps_agents_out_of_user_tabs_only() {
    let mut state = opened().browser_pressed(BROWSER_LOCK);
    assert!(state.browser.as_ref().unwrap().locked);
    state = state.browser_pressed(BROWSER_LOCK);
    assert!(!state.browser.as_ref().unwrap().locked);
    state.browser_tab_opened("http://localhost:3000", None, true, true);
    state = state.browser_pressed(BROWSER_LOCK);
    assert!(!state.browser.as_ref().unwrap().locked);
}

/// An agent opens a tab behind the one shown, and it becomes the tab the tools work on.
#[test]
fn an_agent_tab_opens_in_the_background() {
    let mut state = opened();
    let id = state
        .browser_tab_opened("http://localhost:3000", None, true, false)
        .unwrap();
    let browser = state.browser.as_ref().unwrap();
    assert_eq!(browser.active_id(), 1);
    assert_eq!(browser.agent_target(), id);
    let infos = browser.tab_infos();
    assert!(infos[1].agent && infos[1].target && !infos[1].viewing);
    assert!(infos[0].viewing && !infos[0].target);
}

/// The tools that only read a page are allowed on a locked tab and every other tool, listed or not, is not.
#[test]
fn only_reading_tools_pass_a_lock() {
    for tool in [
        "browser_snapshot",
        "browser_screenshot",
        "browser_console",
        "browser_network",
        "browser_wait",
    ] {
        assert!(!changes_page(tool), "{tool}");
    }
    for tool in [
        "browser_click",
        "browser_navigate",
        "browser_evaluate",
        "browser_cdp",
        "browser_new_tool",
    ] {
        assert!(changes_page(tool), "{tool}");
    }
}

/// The tabs, the one shown, the locks and the marks of agents' tabs come back after a restart.
#[test]
fn tabs_survive_a_restart() {
    let mut state = opened().browser_pressed(BROWSER_LOCK);
    state.browser_tab_opened(
        "http://localhost:3000",
        Some("ssh://dev".to_owned()),
        true,
        true,
    );
    let (tabs, active, tools, width) = state.browser_saved().unwrap();
    assert_eq!(tabs.len(), 2);
    assert_eq!(active, 1);
    let mut restored = WorkspaceState::sample();
    restored.restore_browser(&tabs, active, tools, width, true);
    let browser = restored.browser.as_ref().unwrap();
    assert_eq!(browser.active, 1);
    assert!(browser.tabs[0].locked && !browser.tabs[0].agent);
    assert!(browser.tabs[1].agent && !browser.tabs[1].locked);
    assert_eq!(browser.tabs[1].host.as_deref(), Some("ssh://dev"));
    let saved = SavedTab {
        address: "http://localhost:3000".to_owned(),
        host: None,
        agent: true,
        locked: true,
    };
    restored.restore_browser(&[saved], 7, false, 0.0, false);
    let browser = restored.browser.as_ref().unwrap();
    assert_eq!(browser.active, 0);
    assert!(!browser.tabs[0].locked);
}

/// The strip draws every tab when each can be as narrow as the least, and otherwise scrolls to keep the tab shown in view.
#[test]
fn the_strip_scrolls_to_the_tab_shown() {
    let mut state = opened();
    for _ in 0..9 {
        state.browser_tab_opened("about:blank", None, false, false);
    }
    let browser = state.browser.as_mut().unwrap();
    let (all, _) = browser.strip_window(4000.0);
    assert_eq!(all, 0..10);
    let (some, each) = browser.strip_window(600.0);
    assert!(some.len() < 10 && each >= 80.0);
    browser.select(9);
    let (some, _) = browser.strip_window(600.0);
    assert!(some.contains(&9));
    browser.select(0);
    let (some, _) = browser.strip_window(600.0);
    assert!(some.contains(&0));
}

/// A page opened while the current one is in use goes to a tab of its own, and commands are tagged with their tab.
#[test]
fn commands_carry_their_tab() {
    let mut state = opened();
    state.take_browser_commands();
    state.open_served_page("http://localhost:9000", None);
    let commands = state.take_browser_commands();
    assert_eq!(
        commands,
        vec![(2, Command::Load("http://localhost:9000".to_owned()))]
    );
    assert_eq!(state.browser.as_ref().unwrap().active, 0);
}
