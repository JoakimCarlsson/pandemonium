//! The keyboard away from text: Tab walking the focus, Enter and Space
//! pressing what has it, the arrows and Enter in the menu of commands the
//! prompt offers, digits answering a question, and Escape closing what is
//! open before letting go of the focus.

use pm_components::areas::{ComposerEvent, DiffEvent, EditorEvent, SidebarEvent};
use pm_components::state::{Page, Tab};
use pm_components::target::Target;
use rustle::{Key, Modifiers};

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// Shift held.
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
    meta: false,
};

/// The text of the prompt.
fn prompt(rig: &Rig) -> String {
    rig.read(|screen| {
        screen
            .text_field(Target::Composer(ComposerEvent::Prompt))
            .map(rustle::TextFieldState::text)
    })
    .unwrap_or_default()
}

/// Tab moves the focus on from the prompt along the order things are drawn
/// in, and Shift+Tab brings it back.
#[test]
fn tab_walks_the_focus() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    rig.key(Key::Tab);
    let next = rig.focused();
    assert!(
        next.is_some() && next != Some(Target::Composer(ComposerEvent::Prompt)),
        "{next:?}"
    );
    let order = rig.focus_order_in(None);
    let at = |id| order.iter().position(|held| *held == id);
    assert_eq!(
        at(next.unwrap_or(Target::Composer(ComposerEvent::Prompt))),
        at(Target::Composer(ComposerEvent::Prompt)).map(|index| index + 1)
    );
    rig.chord(SHIFT, Key::Tab);
    assert_eq!(rig.focused(), Some(Target::Composer(ComposerEvent::Prompt)));
}

/// Enter and Space press the control that has the keyboard, once the focus the app keeps
/// on it is composed into rustle's.
#[test]
fn enter_and_space_press_what_has_the_keyboard() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.app
        .borrow_mut()
        .focus
        .focus(Target::Diff(DiffEvent::TabDiff));
    rig.settle();
    rig.key(Key::Enter);
    assert_eq!(rig.state(|state| state.tab), Tab::Diff);
    rig.app
        .borrow_mut()
        .focus
        .focus(Target::Sidebar(SidebarEvent::Session(2)));
    rig.settle();
    rig.key(Key::Character(" ".into()));
    assert_eq!(rig.state(|state| state.page), Page::Session(2));
}

/// A slash typed in the prompt offers the agent's commands; the arrows move
/// along them and Enter types the one picked into the prompt.
#[test]
fn the_command_menu_takes_the_arrows_and_enter() {
    let mut rig = Rig::shot(Shot::Slashing);
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    rig.key(Key::End);
    assert!(rig.read(pm_pages::Screen::slash_open));
    let first = rig.state(|state| state.slash);
    rig.key(Key::ArrowDown);
    assert_eq!(rig.state(|state| state.slash), first + 1);
    rig.key(Key::ArrowUp);
    assert_eq!(rig.state(|state| state.slash), first);
    rig.key(Key::Enter);
    assert!(!rig.read(pm_pages::Screen::slash_open));
    let typed = prompt(&rig);
    assert!(typed.starts_with('/') && typed.len() > 1, "{typed}");
}

/// Escape closes the menu of commands before it lets go of the prompt.
#[test]
fn escape_closes_before_letting_go() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    let at = rig.center(Target::Composer(ComposerEvent::Prompt));
    rig.right_click(at);
    assert!(rig.state(|state| state.menu.is_some()));
    rig.key(Key::Escape);
    assert!(rig.state(|state| state.menu.is_none()));
    assert_eq!(rig.focused(), Some(Target::Composer(ComposerEvent::Prompt)));
    rig.key(Key::Escape);
    assert_eq!(rig.focused(), None);
}

/// While a question waits, a digit typed outside any field picks that answer.
#[test]
fn a_digit_answers_a_question() {
    let mut rig = Rig::shot(Shot::Inquiring);
    let picks = |rig: &Rig| rig.state(|state| state.draft().map(|draft| draft.picks.clone()));
    let before = picks(&rig).expect("a question waits");
    rig.key(Key::Character("1".into()));
    let after = picks(&rig).expect("a question waits");
    assert_ne!(before, after);
}

/// Enter in the prompt sends it; Shift+Enter breaks the line instead.
#[test]
fn enter_sends_and_shift_enter_breaks_the_line() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(Target::Composer(ComposerEvent::Prompt));
    rig.type_text("first");
    rig.chord(SHIFT, Key::Enter);
    rig.type_text("second");
    assert_eq!(prompt(&rig), "first\nsecond");
    rig.key(Key::Enter);
    assert_eq!(prompt(&rig), "");
}

/// After a press on a changed file, Ctrl+A marks every changed file rather
/// than selecting text.
#[test]
fn select_all_marks_every_changed_file() {
    let mut rig = Rig::shot(Shot::Workspace);
    let row = Target::Diff(DiffEvent::ChangedFile(0));
    rig.click_on(row);
    rig.chord(
        Modifiers {
            ctrl: true,
            ..Modifiers::default()
        },
        Key::Character("a".into()),
    );
    let marked = rig.state(|state| state.git.marked.len());
    assert!(marked > 1, "{marked}");
    assert!(rig.app.borrow().text_selection().is_none());
}

/// With modal editing on, keys move the caret of the file shown instead of
/// typing, and `i` lets them type.
#[test]
fn modal_editing_moves_then_types() {
    let mut rig = Rig::shot(Shot::Vim);
    let editor = Target::Editor(EditorEvent::EditorField);
    let text = |rig: &Rig| {
        rig.read(|screen| screen.field(editor).map(|field| field.text().to_owned()))
            .unwrap_or_default()
    };
    rig.click_on(editor);
    let before = text(&rig);
    rig.key(Key::Escape);
    rig.key(Key::Character("j".into()));
    assert_eq!(text(&rig), before);
    rig.key(Key::Character("i".into()));
    rig.type_text("q");
    assert_eq!(text(&rig).len(), before.len() + 1);
}
