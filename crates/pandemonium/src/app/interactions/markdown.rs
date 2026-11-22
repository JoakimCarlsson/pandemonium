//! Markdown set as rustle's annotated text: a link pressed follows its address, and
//! selecting across inline code and across a line the wrapping broke copies the words as
//! the source says them.

use rustle::{Key, Modifiers};

use crate::app::headless::{Rig, SIZE};
use crate::snapshot::{Shot, previewing};

/// Control held.
const CONTROL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
    meta: false,
};

/// A markdown file whose links lead to other files, one of them set in code.
const LINKED: &str = "# Links\n\nRead [the protocol](docs/protocol.md) before changing `pm-protocol`.\n\nThe [`extensions`](docs/extensions.md) guide comes next.\n";

/// How wide `words` are set in running markdown text 14 pixels tall.
fn width_of(words: &str) -> f32 {
    rustle::layout_text(words, &pm_components::theme::text_style(14, 1.57), None)
        .size()
        .width
}

/// The workspace previewing [`LINKED`] as `README.md`.
fn linked() -> Rig {
    Rig::new(previewing("README.md", LINKED), SIZE)
}

/// The path of the file shown.
fn shown(rig: &Rig) -> Option<String> {
    rig.state(|state| state.shown_viewer().map(|viewer| viewer.path.clone()))
}

/// Where the paragraph holding `words` starts, a little inside its first line.
fn start_of(rig: &Rig, words: &str) -> (f32, f32) {
    let (x, y) = rig.text_at(None, words).expect("the words are drawn");
    (x + 1.0, y + 8.0)
}

/// Pressing a link of a markdown file's preview opens the file it leads to.
#[test]
fn pressing_a_link_of_the_preview_follows_it() {
    let mut rig = linked();
    let (x, y) = start_of(&rig, "Read the protocol");
    rig.click((x + width_of("Read ") + 12.0, y));
    assert_eq!(shown(&rig).as_deref(), Some("docs/protocol.md"));
}

/// Pressing a link set in code, on its chip, follows it.
#[test]
fn pressing_a_link_set_in_code_follows_it() {
    let mut rig = linked();
    let (x, y) = start_of(&rig, "guide comes next");
    rig.click((x + width_of("The ") + 20.0, y));
    assert_eq!(shown(&rig).as_deref(), Some("docs/extensions.md"));
}

/// Pressing beside a link follows nothing.
#[test]
fn pressing_beside_a_link_follows_nothing() {
    let mut rig = linked();
    let (x, y) = start_of(&rig, "Read the protocol");
    rig.click((x + 4.0, y));
    assert_eq!(shown(&rig).as_deref(), Some("README.md"));
}

/// Dragging across inline code selects the code with the words around it, and Ctrl+C
/// copies them as the source says them.
#[test]
fn selecting_across_inline_code_copies_it() {
    let mut rig = Rig::shot(Shot::Workspace);
    let (x, y) = start_of(&rig, "again undoes it");
    let past = width_of("Restoring before again und");
    rig.drag((x, y), (x + past, y), 6);
    let picked = rig.app.borrow().selected_text();
    assert!(picked.starts_with("Restoring before again"), "{picked:?}");
    rig.chord(CONTROL, Key::Character("c".into()));
    assert_eq!(rig.clipboard.text.borrow().clone(), Some(picked));
}

/// Text selected across a line the wrapping broke is copied as one line.
#[test]
fn selecting_across_a_wrapped_line_copies_one_line() {
    let mut rig = Rig::shot(Shot::Workspace);
    let (x, y) = start_of(&rig, "again undoes it");
    rig.drag((x, y), (x + 40.0, y + 22.0), 6);
    let picked = rig.app.borrow().selected_text();
    assert!(
        picked.starts_with("Restoring before again") && picked.contains("the way keeps the"),
        "{picked:?}"
    );
    assert!(!picked.contains('\n'), "{picked:?}");
}
