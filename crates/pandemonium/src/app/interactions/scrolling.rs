//! Scrolling: the wheel over a box and over a box inside another, holding
//! and dragging a scrollbar, and the conversation keeping to its end.

use pm_components::kit::SCROLLBAR_GRIP;
use pm_components::state::{CHAT_SCROLL, PROMPT};

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// How far the box `id` is scrolled, and how far it can be.
fn scrolled(rig: &Rig, id: pm_components::kit::FocusId) -> (f32, f32) {
    let report = rig.report();
    let scroller = report.scroller(id).expect("the box is drawn");
    (scroller.offset, scroller.room)
}

/// The middle of the box `id`.
fn middle(rig: &Rig, id: pm_components::kit::FocusId) -> (f32, f32) {
    let report = rig.report();
    let rect = report.scroller(id).expect("the box is drawn").rect;
    (
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

/// The wheel over the conversation scrolls it, and not past either end.
#[test]
fn the_wheel_scrolls_the_conversation() {
    let mut rig = Rig::shot(Shot::Workspace);
    let (offset, room) = scrolled(&rig, CHAT_SCROLL);
    assert!(room > 100.0);
    assert_eq!(offset, room);
    let at = middle(&rig, CHAT_SCROLL);
    rig.move_to(at);
    rig.wheel(-100.0);
    assert_eq!(scrolled(&rig, CHAT_SCROLL).0, room - 100.0);
    rig.wheel(-10_000.0);
    assert_eq!(scrolled(&rig, CHAT_SCROLL).0, 0.0);
    rig.wheel(10_000.0);
    assert_eq!(scrolled(&rig, CHAT_SCROLL).0, room);
}

/// The wheel over the long prompt scrolls the prompt, not the conversation around it.
#[test]
fn the_wheel_scrolls_the_innermost_box() {
    let mut rig = Rig::shot(Shot::ComposerLong);
    let chat = scrolled(&rig, CHAT_SCROLL).0;
    let prompt = scrolled(&rig, PROMPT).0;
    let at = middle(&rig, PROMPT);
    rig.move_to(at);
    rig.wheel(60.0);
    assert!(scrolled(&rig, PROMPT).0 > prompt);
    assert_eq!(scrolled(&rig, CHAT_SCROLL).0, chat);
}

/// Holding the conversation's scrollbar and dragging it up scrolls towards the start.
#[test]
fn dragging_the_scrollbar_scrolls() {
    let mut rig = Rig::shot(Shot::Workspace);
    let report = rig.report();
    let rect = report.scroller(CHAT_SCROLL).expect("drawn").rect;
    let x = rect.origin.x + rect.size.width - SCROLLBAR_GRIP / 2.0;
    let bottom = rect.origin.y + rect.size.height - 20.0;
    rig.drag((x, bottom), (x, bottom - 120.0), 6);
    let (offset, room) = scrolled(&rig, CHAT_SCROLL);
    assert!(offset < room - 50.0, "{offset} of {room}");
}

/// At its end the conversation keeps to its end when it has less room to show it.
#[test]
fn the_conversation_keeps_to_its_end() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.resize((1280.0, 680.0));
    let (offset, room) = scrolled(&rig, CHAT_SCROLL);
    assert!(room > 0.0);
    assert_eq!(offset, room);
}

/// Scrolled up from its end, the conversation keeps how far it is from the end
/// when it has less room.
#[test]
fn the_conversation_keeps_its_place_from_the_end() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = middle(&rig, CHAT_SCROLL);
    rig.move_to(at);
    rig.wheel(-100.0);
    rig.resize((1280.0, 680.0));
    let (offset, room) = scrolled(&rig, CHAT_SCROLL);
    assert_eq!(room - offset, 100.0);
}
