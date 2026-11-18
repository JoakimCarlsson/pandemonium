//! Selecting the conversation's text: dragging across it, pressing twice
//! and three times, the keys that select all and copy, and files let go
//! over the window from another application.

use std::path::PathBuf;

use pm_components::kit::{Key, Modifiers};
use pm_components::state::{CHAT_TEXT, TAB_DIFF};
use rustle::Rect;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// Control held.
const CONTROL: Modifiers = Modifiers {
    shift: false,
    control: true,
    alt: false,
    logo: false,
};

/// Where the conversation's text was drawn.
fn conversation(rig: &Rig) -> Rect {
    rig.report()
        .selectable_areas
        .iter()
        .find(|(id, _)| *id == CHAT_TEXT)
        .map(|(_, rect)| *rect)
        .expect("the conversation is drawn")
}

/// The text of the conversation selected.
fn selected(rig: &Rig) -> String {
    rig.app
        .borrow()
        .text_selection
        .map(|selection| rig.report().selected_text(&selection))
        .unwrap_or_default()
}

/// The workspace with its conversation scrolled to the start.
fn at_start() -> Rig {
    let mut rig = Rig::shot(Shot::Workspace);
    let report = rig.report();
    let rect = report
        .scroller(pm_components::state::CHAT_SCROLL)
        .expect("the conversation is drawn")
        .rect;
    rig.move_to((rect.origin.x + 100.0, rect.origin.y + 100.0));
    rig.wheel(-10_000.0);
    rig
}

/// Where the words `words` of the conversation were drawn.
fn words_at(rig: &Rig, words: &str) -> (f32, f32) {
    let (x, y) = rig.text_at(None, words).expect("the words are drawn");
    (x + 2.0, y + 8.0)
}

/// Dragging across the conversation selects the text passed over, and Ctrl+C copies it.
#[test]
fn dragging_selects_and_ctrl_c_copies() {
    let mut rig = at_start();
    let area = conversation(&rig);
    let from = words_at(&rig, "Checkpoints are in.");
    rig.drag(from, (from.0 + 90.0, from.1), 6);
    let picked = selected(&rig);
    assert!(
        !picked.is_empty() && picked.len() < 40,
        "{picked:?} in {area:?}"
    );
    rig.chord(CONTROL, Key::Character('c'));
    assert_eq!(rig.clipboard.text.borrow().clone(), Some(picked));
}

/// Pressing twice selects a word; three times, its paragraph.
#[test]
fn double_and_triple_presses() {
    let mut rig = at_start();
    let at = words_at(&rig, "Checkpoints are in.");
    rig.click(at);
    rig.click(at);
    assert_eq!(selected(&rig).trim(), "Checkpoints");
    rig.click(at);
    assert!(
        selected(&rig).starts_with("Checkpoints are in."),
        "{}",
        selected(&rig)
    );
}

/// Ctrl+A outside any field selects the whole conversation, and a press elsewhere lets it go.
#[test]
fn select_all_and_let_go() {
    let mut rig = at_start();
    let at = words_at(&rig, "Checkpoints are in.");
    rig.click(at);
    rig.chord(CONTROL, Key::Character('a'));
    let all = selected(&rig);
    assert!(
        all.contains("Checkpoints are in.") && all.contains("Why it matters"),
        "{all}"
    );
    rig.click_on(TAB_DIFF);
    assert!(rig.app.borrow_mut().text_selection.is_none());
}

/// Files let go over the conversation from another application are
/// attached to the prompt once the pointer is seen again.
#[test]
fn files_dropped_attach_to_the_prompt() {
    let mut rig = at_start();
    let at = words_at(&rig, "Checkpoints are in.");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    rig.drop_files(at, vec![path.clone()]);
    rig.move_to((at.0 + 1.0, at.1));
    let attached = rig.state(|state| state.attachments.clone());
    assert_eq!(attached, vec![path.to_string_lossy().into_owned()]);
}
