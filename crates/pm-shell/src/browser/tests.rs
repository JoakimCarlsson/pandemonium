//! Tests of the browser's tabs and of what a page found by a server does to them.

use super::{Browser, Command, MOST_TABS, Move, SavedTab, served_move};

/// A browser on nothing or an error loads the page, one on a page of the same server loads it again where it is, and one on another page opens a tab of its own.
#[test]
fn a_page_in_use_is_left_alone() {
    let page = "http://localhost:8766/";
    assert_eq!(served_move("", page), Move::Load);
    assert_eq!(served_move("about:blank", page), Move::Load);
    assert_eq!(
        served_move("chrome-error://chromewebdata/", page),
        Move::Load
    );
    assert_eq!(served_move("http://localhost:8766/", page), Move::Reload);
    assert_eq!(
        served_move("http://localhost:8766/form.html?a=1", page),
        Move::Reload
    );
    assert_eq!(served_move("http://localhost:8765/", page), Move::NewTab);
    assert_eq!(served_move("http://localhost:87660/", page), Move::NewTab);
    assert_eq!(served_move("https://example.com/", page), Move::NewTab);
}

/// Another name of this machine is the same server.
#[test]
fn loopback_names_are_the_same_server() {
    let page = "http://localhost:5173/";
    assert_eq!(served_move("http://127.0.0.1:5173/", page), Move::Reload);
    assert_eq!(served_move("http://[::1]:5173/app?x=1", page), Move::Reload);
    assert_eq!(served_move("http://127.0.0.1:51730/", page), Move::NewTab);
}

/// A page found again for an address a tab shows reloads that tab and opens no other, and a different address opens a tab behind it.
#[test]
fn a_hot_reload_opens_no_new_tab() {
    let mut browser = Browser::new("http://localhost:5173/", None);
    browser.take_commands();
    browser.serve("http://localhost:5173/", None);
    browser.serve("http://localhost:5173/", None);
    assert_eq!(browser.tabs.len(), 1);
    browser.serve("http://localhost:5174/", None);
    assert_eq!(browser.tabs.len(), 2);
    assert_eq!(browser.active, 0);
    browser.serve("http://localhost:5174/", None);
    assert_eq!(browser.tabs.len(), 2);
}

/// Closing the shown tab shows its neighbour, and closing the last says so.
#[test]
fn closing_tabs_moves_the_shown_one() {
    let mut browser = Browser::new("about:blank", None);
    browser.open_tab("http://localhost:1", None, false, true);
    browser.open_tab("http://localhost:2", None, false, true);
    assert_eq!(browser.active, 2);
    assert!(!browser.close_tab(2));
    assert_eq!(browser.active, 1);
    assert!(!browser.close_tab(0));
    assert_eq!(browser.active, 0);
    assert!(browser.close_tab(0));
}

/// The browser holds no more than its most tabs.
#[test]
fn tabs_are_capped() {
    let mut browser = Browser::new("about:blank", None);
    for _ in 1..MOST_TABS {
        assert!(
            browser
                .open_tab("about:blank", None, false, false)
                .is_some()
        );
    }
    assert!(
        browser
            .open_tab("about:blank", None, false, false)
            .is_none()
    );
}

/// Showing another tab while the developer tools are open moves them to it.
#[test]
fn the_developer_tools_follow_the_tab_shown() {
    let mut browser = Browser::new("about:blank", None);
    browser.open_tab("about:blank", None, false, false);
    browser.take_commands();
    browser.toggle_tools();
    browser.select(1);
    let commands = browser.take_commands();
    let second = browser.tabs[1].id;
    assert!(commands.contains(&(second, Command::OpenTools)));
}

/// A restored browser keeps its tabs, the one shown and its lock, but never locks an agent's tab.
#[test]
fn restoring_keeps_the_tabs() {
    let saved = vec![
        SavedTab {
            address: "http://localhost:1".to_owned(),
            host: None,
            agent: false,
            locked: true,
        },
        SavedTab {
            address: "http://localhost:2".to_owned(),
            host: Some("box".to_owned()),
            agent: true,
            locked: true,
        },
    ];
    let browser = Browser::restored(&saved, 1, false, 0.0).expect("tabs were saved");
    assert_eq!(browser.active, 1);
    assert!(browser.tabs[0].locked);
    assert!(!browser.tabs[1].locked);
    assert_eq!(browser.saved_tabs()[1].host.as_deref(), Some("box"));
    assert!(Browser::restored(&[], 0, false, 0.0).is_none());
}

/// Typing in the find bar looks for the text, stepping looks again, and closing stops.
#[test]
fn finding_queues_searches() {
    let mut browser = Browser::new("about:blank", None);
    browser.take_commands();
    let tab = browser.shown_mut();
    tab.find_opened();
    tab.find_typed("needle".to_owned());
    tab.find_stepped(true);
    tab.found(1, 3);
    assert_eq!(
        tab.search.as_ref().map(|s| s.tally().0),
        Some("1 of 3".to_owned())
    );
    tab.find_closed();
    let commands: Vec<Command> = std::mem::take(&mut tab.commands);
    assert_eq!(
        commands,
        vec![
            Command::Find {
                text: "needle".to_owned(),
                forward: true,
                again: false
            },
            Command::Find {
                text: "needle".to_owned(),
                forward: false,
                again: true
            },
            Command::StopFind,
        ]
    );
}
