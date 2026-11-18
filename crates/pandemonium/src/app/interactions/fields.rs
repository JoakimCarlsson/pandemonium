//! Text fields: the caret placed by a press, typing, selecting by keys and
//! by dragging, undo, the clipboard and an input method composing.

use pm_components::kit::{Key, Modifiers};
use pm_pages::PROMPT;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// Control held, as the shortcuts that select, undo and use the clipboard want.
const CONTROL: Modifiers = Modifiers {
    shift: false,
    control: true,
    alt: false,
    logo: false,
};

/// Shift held, as selecting by keys wants.
const SHIFT: Modifiers = Modifiers {
    shift: true,
    control: false,
    alt: false,
    logo: false,
};

/// The text of the prompt.
fn prompt(rig: &Rig) -> String {
    rig.read(|screen| screen.field(PROMPT).map(|field| field.text().to_owned()))
        .unwrap_or_default()
}

/// The workspace with the prompt pressed, so it has the keyboard, and emptied.
fn typing() -> Rig {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(PROMPT);
    rig.chord(CONTROL, Key::Character('a'));
    rig.key(Key::Backspace);
    rig
}

/// Pressing the prompt gives it the keyboard; Escape takes it away.
#[test]
fn a_press_focuses_the_field_and_escape_lets_go() {
    let mut rig = Rig::shot(Shot::Workspace);
    assert_ne!(rig.focused(), Some(PROMPT));
    rig.click_on(PROMPT);
    assert_eq!(rig.focused(), Some(PROMPT));
    rig.key(Key::Escape);
    assert_eq!(rig.focused(), None);
}

/// Keys type into the field with the keyboard, and Backspace takes the last back.
#[test]
fn keys_type_and_backspace_erases() {
    let mut rig = typing();
    rig.type_text("hello world");
    assert_eq!(prompt(&rig), "hello world");
    rig.key(Key::Backspace);
    assert_eq!(prompt(&rig), "hello worl");
}

/// Ctrl+Backspace erases a word; arrows move the caret where the next key types.
#[test]
fn words_erase_and_arrows_move_the_caret() {
    let mut rig = typing();
    rig.type_text("one two");
    rig.chord(CONTROL, Key::Backspace);
    assert_eq!(prompt(&rig), "one ");
    rig.key(Key::Home);
    rig.type_text("x");
    assert_eq!(prompt(&rig), "xone ");
    rig.key(Key::End);
    rig.key(Key::ArrowLeft);
    rig.type_text("y");
    assert_eq!(prompt(&rig), "xoney ");
}

/// Undo takes typing back and redo puts it again.
#[test]
fn undo_and_redo() {
    let mut rig = typing();
    rig.type_text("abc");
    rig.chord(CONTROL, Key::Character('z'));
    assert_ne!(prompt(&rig), "abc");
    rig.chord(
        Modifiers {
            shift: true,
            ..CONTROL
        },
        Key::Character('z'),
    );
    assert_eq!(prompt(&rig), "abc");
}

/// Shift with arrows selects, and copying puts the selection on the clipboard;
/// cutting also takes it out, and pasting puts it back where the caret is.
#[test]
fn the_clipboard_copies_cuts_and_pastes() {
    let mut rig = typing();
    rig.type_text("alpha beta");
    rig.hold(SHIFT);
    for _ in 0..4 {
        rig.key(Key::ArrowLeft);
    }
    rig.hold(Modifiers::default());
    rig.chord(CONTROL, Key::Character('c'));
    assert_eq!(rig.clipboard.text.borrow().as_deref(), Some("beta"));
    rig.chord(CONTROL, Key::Character('x'));
    assert_eq!(prompt(&rig), "alpha ");
    rig.key(Key::Home);
    rig.chord(CONTROL, Key::Character('v'));
    assert_eq!(prompt(&rig), "betaalpha ");
}

/// Ctrl+A selects everything, and typing replaces it.
#[test]
fn select_all_then_type_replaces() {
    let mut rig = typing();
    rig.type_text("old words");
    rig.chord(CONTROL, Key::Character('a'));
    rig.type_text("new");
    assert_eq!(prompt(&rig), "new");
}

/// An input method's composition stays out of the text until it commits.
#[test]
fn an_input_method_composes_then_commits() {
    let mut rig = typing();
    rig.type_text("a");
    rig.preedit("にほ");
    assert_eq!(prompt(&rig), "a");
    rig.preedit("");
    rig.commit("日本");
    assert_eq!(prompt(&rig), "a日本");
}

/// Dragging across the text selects what the pointer passed over.
#[test]
fn dragging_selects_text() {
    let mut rig = typing();
    rig.type_text("drag across these words");
    let field = rig
        .report()
        .field(PROMPT)
        .cloned()
        .expect("the prompt is drawn");
    let start = field.caret_at(0);
    let end = field.caret_at(10);
    let y = start.1 + start.2 / 2.0;
    rig.drag((start.0 + 0.5, y), (end.0 + 0.5, y), 4);
    let selected = rig.read(|screen| {
        screen
            .field(PROMPT)
            .map(|field| field.selected().to_owned())
            .unwrap_or_default()
    });
    assert_eq!(selected, "drag acros");
}

/// Pressing twice in a row selects the word pressed on.
#[test]
fn a_double_press_selects_a_word() {
    let mut rig = typing();
    rig.type_text("first second third");
    let field = rig
        .report()
        .field(PROMPT)
        .cloned()
        .expect("the prompt is drawn");
    let inside = field.caret_at(8);
    let at = (inside.0, inside.1 + inside.2 / 2.0);
    rig.click(at);
    rig.click(at);
    let selected = rig.read(|screen| {
        screen
            .field(PROMPT)
            .map(|field| field.selected().to_owned())
            .unwrap_or_default()
    });
    assert_eq!(selected, "second");
}

/// The input method is told where the caret is, in the window's coordinates.
#[test]
fn the_input_method_follows_the_caret() {
    let mut rig = typing();
    rig.type_text("x");
    let first = rig
        .window()
        .ime_area
        .get()
        .expect("the caret area was given");
    rig.type_text("xxxx");
    let later = rig
        .window()
        .ime_area
        .get()
        .expect("the caret area was given");
    assert!(later.0 > first.0, "{first:?} then {later:?}");
}
