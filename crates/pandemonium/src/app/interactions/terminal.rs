//! The shell: taking the keyboard when pressed, keys and an input method's
//! text sent to it, marking its text by dragging, its menu, and the wheel.

use pm_components::areas::TerminalEvent;
use pm_components::target::Target;
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
    rig.click_on(Target::Terminal(TerminalEvent::TerminalBody));
    assert_eq!(
        rig.focused(),
        Some(Target::Terminal(TerminalEvent::TerminalBody))
    );
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
    rig.click_on(Target::Terminal(TerminalEvent::TerminalBody));
    let _ = sent(&mut rig);
    rig.commit("é");
    let asked = sent(&mut rig);
    assert!(asked.contains("195, 169"), "{asked}");
}

/// Dragging across the shell marks its text.
#[test]
fn dragging_marks_the_shell_text() {
    let mut rig = Rig::shot(Shot::Terminal);
    let body = rig
        .region(Target::Terminal(TerminalEvent::TerminalBody))
        .expect("the shell is drawn");
    let from = (body.origin.x + 4.0, body.origin.y + 6.0);
    rig.drag(from, (from.0 + 80.0, from.1), 5);
    let marked = rig.read(|screen| screen.terminal_selection());
    assert!(marked.is_some_and(|text| !text.is_empty()));
}

/// The secondary button over the shell opens its menu.
#[test]
fn the_shell_menu_opens() {
    let mut rig = Rig::shot(Shot::Terminal);
    let at = rig.center(Target::Terminal(TerminalEvent::TerminalBody));
    rig.right_click(at);
    assert!(rig.state(|state| state.terminal_menu.is_some()));
    rig.key(Key::Escape);
}

/// The shell of the shell's workspace with `printed` written to it, as the shell would print it.
fn printing(printed: &str) -> Rig {
    let mut rig = Rig::shot(Shot::Terminal);
    let tab = rig
        .state(|state| state.active_terminal().map(|tab| tab.id))
        .expect("a shell shows");
    let model = rig.app.borrow().model.clone();
    model.change(|mut state| {
        state.terminal_output(tab, printed.as_bytes());
        state
    });
    rig.settle();
    rig
}

/// The wheel over the shell scrolls it back through what scrolled off its window, and
/// forward again.
#[test]
fn the_wheel_scrolls_the_shell_back() {
    let lines: String = (0..40).map(|line| format!("line {line}\r\n")).collect();
    let mut rig = printing(&lines);
    let back = |rig: &Rig| rig.read(|screen| screen.terminal_window().map(|(.., back)| back));
    assert_eq!(back(&rig), Some(0));
    rig.move_to(rig.center(Target::Terminal(TerminalEvent::TerminalBody)));
    rig.wheel(-120.0);
    let scrolled = back(&rig).expect("the shell shows");
    assert!(scrolled > 0, "{scrolled}");
    rig.wheel(120.0);
    assert_eq!(back(&rig), Some(0));
}

/// Holding Control over a web address in the shell underlines it with the pointing hand,
/// and letting go takes the underline away.
#[test]
fn control_reaches_a_link_in_the_shell() {
    let mut rig = printing("https://example.com/docs\r\n");
    let at = rig
        .text_at(None, "https://example.com/docs")
        .expect("the address is drawn");
    let reached =
        |rig: &Rig| rig.state(|state| state.active_terminal().map(|tab| tab.link.is_some()));
    rig.move_to((at.0 + 30.0, at.1 + 6.0));
    assert_eq!(reached(&rig), Some(false));
    rig.hold(rustle::Modifiers {
        ctrl: true,
        ..rustle::Modifiers::NONE
    });
    assert_eq!(reached(&rig), Some(true));
    assert_eq!(rig.cursor(), rustle::CursorIcon::Pointer);
    rig.hold(rustle::Modifiers::NONE);
    assert_eq!(reached(&rig), Some(false));
    assert_eq!(rig.cursor(), rustle::CursorIcon::Text);
}
