//! Selecting the conversation's text: dragging across it, pressing twice
//! and three times, the keys that select all and copy, and files let go
//! over the window from another application.

use pm_components::areas::{ChatEvent, DiffEvent};
use pm_components::target::Target;
use std::path::PathBuf;

use rustle::Rect;
use rustle::{Key, Modifiers};

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// Control held.
const CONTROL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
    meta: false,
};

/// Where the conversation's text was drawn.
fn conversation(rig: &Rig) -> Rect {
    rig.region(Target::Chat(ChatEvent::ChatText))
        .expect("the conversation is drawn")
}

/// The text of the conversation selected.
fn selected(rig: &Rig) -> String {
    rig.app.borrow().selected_text()
}

/// The workspace with its conversation scrolled to the start.
fn at_start() -> Rig {
    let mut rig = Rig::shot(Shot::Workspace);
    let rect = rig
        .area(pm_pages::Screen::conversation_area)
        .expect("the conversation is drawn");
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
    rig.chord(CONTROL, Key::Character("c".into()));
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
    rig.chord(CONTROL, Key::Character("a".into()));
    let all = selected(&rig);
    assert!(
        all.contains("Checkpoints are in.") && all.contains("Why it matters"),
        "{all}"
    );
    rig.click_on(Target::Diff(DiffEvent::TabDiff));
    assert!(rig.app.borrow().text_selection().is_none());
}

/// Files let go over the conversation from another application are
/// attached to the prompt where they were let go, every one of them, without
/// waiting for the pointer to be seen again.
#[test]
fn files_dropped_attach_to_the_prompt() {
    let mut rig = at_start();
    let at = words_at(&rig, "Checkpoints are in.");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paths = vec![manifest.join("Cargo.toml"), manifest.join("src")];
    rig.drop_files(at, paths.clone());
    let attached = rig.state(|state| state.attachments.clone());
    let expected: Vec<String> = paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(attached, expected);
}

/// Dragging from one message of the conversation's list to another selects across both, and
/// what lies between them, as one document.
#[test]
fn dragging_selects_across_messages() {
    let mut rig = at_start();
    let from = words_at(&rig, "Sessions lose their place");
    let to = words_at(&rig, "Checkpoints are in.");
    rig.drag(from, (to.0 + 60.0, to.1), 8);
    let picked = selected(&rig);
    assert!(
        picked.starts_with("Sessions lose their place") && picked.contains("\nCheckpoi"),
        "{picked:?}"
    );
}
