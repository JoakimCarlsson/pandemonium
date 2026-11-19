//! The shell: taking the keyboard when pressed, keys and an input method's
//! text sent to it, marking its text by dragging, its menu, and the wheel.

use pm_components::state::TERMINAL_BODY;
use rustle::Key;

use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// What the shell was asked to be sent since the last time, as written out.
fn sent(rig: &mut Rig) -> String {
    format!("{:?}", rig.app.borrow_mut().work.asked())
}

/// The shell's workspace, its shell taken to be open on the server as the first.
fn opened() -> Rig {
    let rig = Rig::shot(Shot::Terminal);
    let tab = rig
        .state(|state| state.active_terminal().map(|tab| tab.id))
        .expect("a shell shows");
    let model = rig.app.borrow().model.clone();
    rig.app.borrow_mut().work.terminal_opened(&model, tab, 1);
    rig
}

/// Pressing the shell gives it the keyboard; keys typed then go to it, not to a field.
#[test]
fn the_shell_takes_the_keyboard_and_keys() {
    let mut rig = opened();
    rig.click_on(TERMINAL_BODY);
    assert_eq!(rig.focused(), Some(TERMINAL_BODY));
    let _ = sent(&mut rig);
    rig.type_text("ls");
    rig.key(Key::Enter);
    let asked = sent(&mut rig);
    assert!(
        asked.contains("[108]") && asked.contains("[115]") && asked.contains("[13]"),
        "{asked}"
    );
}

/// An input method's text goes to the shell while it has the keyboard.
#[test]
fn an_input_method_types_into_the_shell() {
    let mut rig = opened();
    rig.click_on(TERMINAL_BODY);
    let _ = sent(&mut rig);
    rig.commit("é");
    let asked = sent(&mut rig);
    assert!(asked.contains("195, 169"), "{asked}");
}

/// Dragging across the shell marks its text.
#[test]
fn dragging_marks_the_shell_text() {
    let mut rig = Rig::shot(Shot::Terminal);
    let body = rig.region(TERMINAL_BODY).expect("the shell is drawn");
    let from = (body.origin.x + 4.0, body.origin.y + 6.0);
    rig.drag(from, (from.0 + 80.0, from.1), 5);
    let marked = rig.read(|screen| screen.terminal_selection());
    assert!(marked.is_some_and(|text| !text.is_empty()));
}

/// The secondary button over the shell opens its menu.
#[test]
fn the_shell_menu_opens() {
    let mut rig = Rig::shot(Shot::Terminal);
    let at = rig.center(TERMINAL_BODY);
    rig.right_click(at);
    assert!(rig.state(|state| state.terminal_menu.is_some()));
    rig.key(Key::Escape);
}
