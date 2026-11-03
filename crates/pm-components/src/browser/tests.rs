//! Tests of what a page found by a server does to the page the browser shows.

use super::{Move, served_move};
use crate::state::WorkspaceState;

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

/// A server that prints its address again, by the same name or another name of this machine, is the page a tab already shows, so the tab loads it again and no tab opens.
#[test]
fn a_repeat_address_reloads_the_tab_that_shows_it() {
    let page = "http://localhost:5173/";
    assert_eq!(served_move("http://localhost:5173/", page), Move::Reload);
    assert_eq!(served_move("http://localhost:5173", page), Move::Reload);
    assert_eq!(served_move("http://127.0.0.1:5173/", page), Move::Reload);
    assert_eq!(served_move("http://[::1]:5173/app?x=1", page), Move::Reload);
    assert_eq!(served_move("http://127.0.0.1:51730/", page), Move::NewTab);
    assert_eq!(served_move("http://127.0.0.1:5174/", page), Move::NewTab);
}

/// A page found again for an address a tab shows reloads that tab and opens no other, and a different address opens a tab behind it.
#[test]
fn a_hot_reload_opens_no_new_tab() {
    let mut state = WorkspaceState::new(&[]);
    state.open_served_page("http://localhost:5173/", None);
    state.open_served_page("http://localhost:5173/", None);
    state.open_served_page("http://localhost:5173/", None);
    assert_eq!(state.browser.as_ref().map(|b| b.tabs.len()), Some(1));
    state.open_served_page("http://localhost:5174/", None);
    assert_eq!(state.browser.as_ref().map(|b| b.tabs.len()), Some(2));
    state.open_served_page("http://localhost:5173/", None);
    state.open_served_page("http://localhost:5174/", None);
    assert_eq!(state.browser.as_ref().map(|b| b.tabs.len()), Some(2));
}
