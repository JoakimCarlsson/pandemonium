//! Pressing buttons, rows and tabs, hovering for tooltips and the cursor,
//! and the mouse's back and forward buttons.

use pm_components::focus::FocusId;
use pm_components::state::{Page, SESSION_BASE, SETTINGS, TAB_CHAT, TAB_DIFF, Tab};
use winit::event::MouseButton;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// The row of the `index`th session in the sidebar.
fn session_row(index: u64) -> FocusId {
    FocusId(SESSION_BASE + index)
}

/// Pressing a session's row in the sidebar opens it.
#[test]
fn pressing_a_session_row_opens_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    assert_eq!(rig.state(|state| state.page), Page::Session(0));
    rig.click_on(session_row(1));
    assert_eq!(rig.state(|state| state.page), Page::Session(1));
}

/// A press that is let go of over another control presses neither.
#[test]
fn a_press_let_go_elsewhere_does_nothing() {
    let mut rig = Rig::shot(Shot::Workspace);
    let from = rig.center(session_row(1));
    let to = rig.center(session_row(2));
    rig.move_to(from);
    rig.button_in(None, MouseButton::Left, true, from);
    rig.move_to(to);
    rig.button_in(None, MouseButton::Left, false, to);
    assert_eq!(rig.state(|state| state.page), Page::Session(0));
}

/// The tabs over the session switch between the conversation and the changes.
#[test]
fn tabs_switch_the_view() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(TAB_DIFF);
    assert_eq!(rig.state(|state| state.tab), Tab::Diff);
    rig.click_on(TAB_CHAT);
    assert_eq!(rig.state(|state| state.tab), Tab::Chat);
}

/// A tooltip shows once the pointer has rested on its button, and goes when the pointer leaves.
#[test]
fn a_tooltip_shows_after_a_rest_and_goes_on_leaving() {
    let mut rig = Rig::shot(Shot::Workspace);
    let on = rig.center(SETTINGS);
    rig.move_to(on);
    assert!(!rig.shows("Open Settings"));
    rig.wait(400);
    assert!(rig.shows("Open Settings"));
    rig.move_to((on.0 - 200.0, on.1 + 300.0));
    rig.wait(400);
    assert!(!rig.shows("Open Settings"));
}

/// The mouse's back and forward buttons step through the pages visited.
#[test]
fn the_mouse_steps_back_and_forward() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(session_row(1));
    rig.click_on(session_row(2));
    let at = rig.center(TAB_CHAT);
    rig.button_in(None, MouseButton::Back, true, at);
    rig.button_in(None, MouseButton::Back, false, at);
    assert_eq!(rig.state(|state| state.page), Page::Session(1));
    rig.button_in(None, MouseButton::Forward, true, at);
    rig.button_in(None, MouseButton::Forward, false, at);
    assert_eq!(rig.state(|state| state.page), Page::Session(2));
}

/// Over a row the pointer shows a hand; over nothing in particular, the arrow.
#[test]
fn the_cursor_follows_what_is_under_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let row = rig.center(session_row(1));
    rig.move_to(row);
    assert_eq!(rig.cursor(), rustle::CursorIcon::Pointer);
}
