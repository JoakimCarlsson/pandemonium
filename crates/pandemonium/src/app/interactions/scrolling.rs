//! Scrolling: the wheel over a box and over a box inside another, holding
//! and dragging a scrollbar, and the conversation keeping to its end.

use pm_components::state::PROMPT;
use pm_pages::Screen;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// How wide the strip along a scrolling area's right edge is where the pointer takes hold of its bar.
const GRIP: f32 = 14.0;

/// How far the conversation is scrolled back from its end, and how far back it can go.
fn back(rig: &Rig) -> (f32, f32) {
    rig.read(|screen| {
        let Screen::Workspace(state) = screen;
        let room = state.chat_back.max_offset();
        (state.chat_back.offset().min(room), room)
    })
}

/// Where the conversation is drawn.
fn conversation(rig: &Rig) -> rustle::Rect {
    rig.area(Screen::conversation_area)
        .expect("the conversation is drawn")
}

/// The middle of the conversation.
fn middle(rig: &Rig) -> (f32, f32) {
    let rect = conversation(rig);
    (
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

/// How far the lines of the prompt are scrolled.
fn prompt_offset(rig: &Rig) -> f32 {
    rig.read(|screen| {
        let Screen::Workspace(state) = screen;
        state.prompt_scroll.offset()
    })
}

/// The wheel over the conversation scrolls it, and not past either end.
#[test]
fn the_wheel_scrolls_the_conversation() {
    let mut rig = Rig::shot(Shot::Workspace);
    let (offset, room) = back(&rig);
    assert!(room > 100.0);
    assert_eq!(offset, 0.0);
    let at = middle(&rig);
    rig.move_to(at);
    rig.wheel(-100.0);
    assert_eq!(back(&rig).0, 100.0);
    rig.wheel(-10_000.0);
    assert_eq!(back(&rig).0, room);
    rig.wheel(10_000.0);
    assert_eq!(back(&rig).0, 0.0);
}

/// The wheel over the long prompt scrolls the prompt, not the conversation around it.
#[test]
fn the_wheel_scrolls_the_innermost_box() {
    let mut rig = Rig::shot(Shot::ComposerLong);
    let chat = back(&rig).0;
    let prompt = prompt_offset(&rig);
    let at = rig.center(PROMPT);
    rig.move_to(at);
    rig.wheel(60.0);
    assert!(prompt_offset(&rig) > prompt);
    assert_eq!(back(&rig).0, chat);
}

/// Holding the conversation's scrollbar and dragging it up scrolls towards the start,
/// and selects nothing on the way.
#[test]
fn dragging_the_scrollbar_scrolls() {
    let mut rig = Rig::shot(Shot::Workspace);
    let rect = conversation(&rig);
    let x = rect.origin.x + rect.size.width - GRIP / 2.0;
    let bottom = rect.origin.y + rect.size.height - 20.0;
    rig.drag((x, bottom), (x, bottom - 120.0), 6);
    let (offset, room) = back(&rig);
    assert!(offset > 50.0, "{offset} of {room}");
    assert_eq!(rig.app.borrow().text_selection, None);
}

/// At its end the conversation keeps to its end when it has less room to show it.
#[test]
fn the_conversation_keeps_to_its_end() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.resize((1280.0, 680.0));
    let (offset, room) = back(&rig);
    assert!(room > 0.0);
    assert_eq!(offset, 0.0);
}

/// Scrolled up from its end, the conversation keeps how far it is from the end
/// when it has less room.
#[test]
fn the_conversation_keeps_its_place_from_the_end() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = middle(&rig);
    rig.move_to(at);
    rig.wheel(-100.0);
    rig.resize((1280.0, 680.0));
    assert_eq!(back(&rig).0, 100.0);
}
