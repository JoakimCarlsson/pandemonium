//! Tests of which keys the browser takes for itself.

use crate::areas::BrowserEvent;
use crate::target::Target;
use rustle::{Key, KeyEvent, Modifiers};

use super::{Shortcut, shortcut_of};
use crate::browser::{Command, ZoomStep};
use crate::state::WorkspaceState;

/// What the engine has not yet been told to do, without the tabs it is for.
fn commands(state: &mut WorkspaceState) -> Vec<Command> {
    state
        .take_browser_commands()
        .into_iter()
        .map(|(_, command)| command)
        .collect()
}

/// A press of `key` with Control, and Shift when `shift`.
fn control(key: Key, shift: bool) -> KeyEvent {
    let modifiers = if shift {
        Modifiers::CTRL.with_shift()
    } else {
        Modifiers::CTRL
    };
    KeyEvent::press(key, modifiers)
}

/// A press of `key` with no modifier.
fn plain(key: Key) -> KeyEvent {
    KeyEvent::press(key, Modifiers::NONE)
}

/// Control with a letter, a digit or a sign picks the browser's own commands, and plain letters go to the page.
#[test]
fn control_chords_are_the_browsers() {
    assert_eq!(
        shortcut_of(&control(Key::Character("f".into()), false)),
        Some(Shortcut::Find)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("F".into()), true)),
        Some(Shortcut::Find)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("g".into()), false)),
        Some(Shortcut::FindNext)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("g".into()), true)),
        Some(Shortcut::FindPrevious)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("=".into()), false)),
        Some(Shortcut::ZoomIn)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("+".into()), true)),
        Some(Shortcut::ZoomIn)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("-".into()), false)),
        Some(Shortcut::ZoomOut)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("0".into()), false)),
        Some(Shortcut::ZoomReset)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("r".into()), false)),
        Some(Shortcut::Reload)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("R".into()), true)),
        Some(Shortcut::HardReload)
    );
    assert_eq!(shortcut_of(&plain(Key::Character("f".into()))), None);
    assert_eq!(
        shortcut_of(&control(Key::Character("c".into()), false)),
        None
    );
}

/// The function keys do what a browser's do.
#[test]
fn function_keys_step_and_reload() {
    assert_eq!(shortcut_of(&plain(Key::F(3))), Some(Shortcut::FindNext));
    assert_eq!(shortcut_of(&plain(Key::F(5))), Some(Shortcut::Reload));
    assert_eq!(
        shortcut_of(&control(Key::F(5), false)),
        Some(Shortcut::HardReload)
    );
    assert_eq!(shortcut_of(&plain(Key::F(1))), None);
}

/// Finding shows the bar, looks again for the text kept, and closing it clears the marks; stepping asks the engine for the next match.
#[test]
fn find_opens_steps_and_closes() {
    let mut state = WorkspaceState::sample();
    state.open_browser("http://localhost:5173");
    commands(&mut state);
    state.browser_shortcut(Shortcut::Find);
    assert!(state.browser_find_open());
    assert!(commands(&mut state).is_empty());
    state = state.typed(Target::Browser(BrowserEvent::BrowserFindField), "ab");
    assert_eq!(
        commands(&mut state),
        vec![Command::Find {
            text: "ab".to_owned(),
            forward: true,
            again: false
        }]
    );
    state.browser_shortcut(Shortcut::FindPrevious);
    assert_eq!(
        commands(&mut state),
        vec![Command::Find {
            text: "ab".to_owned(),
            forward: false,
            again: true
        }]
    );
    let tab = state.browser.as_ref().unwrap().active_id();
    state.browser_found(tab, 2, 5);
    let search = state.browser.as_ref().and_then(|b| b.search.as_ref());
    assert_eq!(
        search.map(|search| search.tally().0),
        Some("2 of 5".to_owned())
    );
    state.browser_find_closed();
    assert!(!state.browser_find_open());
    assert_eq!(commands(&mut state), vec![Command::StopFind]);
}

/// Zoom keys queue zoom steps for the engine.
#[test]
fn zoom_keys_queue_steps() {
    let mut state = WorkspaceState::sample();
    state.open_browser("http://localhost:5173");
    commands(&mut state);
    state.browser_shortcut(Shortcut::ZoomIn);
    state.browser_shortcut(Shortcut::ZoomReset);
    assert_eq!(
        commands(&mut state),
        vec![Command::Zoom(ZoomStep::In), Command::Zoom(ZoomStep::Reset)]
    );
}

/// The keys of moving through tabs and history, the address bar, printing and the developer tools are the browser's.
#[test]
fn navigation_keys_are_the_browsers() {
    assert_eq!(
        shortcut_of(&control(Key::Character("l".into()), false)),
        Some(Shortcut::FocusAddress)
    );
    assert_eq!(shortcut_of(&plain(Key::F(6))), Some(Shortcut::FocusAddress));
    assert_eq!(shortcut_of(&plain(Key::F(12))), Some(Shortcut::DevTools));
    assert_eq!(
        shortcut_of(&control(Key::Character("I".into()), true)),
        Some(Shortcut::DevTools)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("p".into()), false)),
        Some(Shortcut::Print)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character("T".into()), true)),
        Some(Shortcut::ReopenTab)
    );
    assert_eq!(
        shortcut_of(&control(Key::Tab, false)),
        Some(Shortcut::NextTab)
    );
    assert_eq!(
        shortcut_of(&control(Key::Tab, true)),
        Some(Shortcut::PreviousTab)
    );
    assert_eq!(
        shortcut_of(&control(Key::PageDown, false)),
        Some(Shortcut::NextTab)
    );
    assert_eq!(
        shortcut_of(&control(Key::PageUp, false)),
        Some(Shortcut::PreviousTab)
    );
    assert_eq!(
        shortcut_of(&KeyEvent::press(Key::ArrowLeft, Modifiers::ALT)),
        Some(Shortcut::Back)
    );
    assert_eq!(
        shortcut_of(&KeyEvent::press(Key::ArrowRight, Modifiers::ALT)),
        Some(Shortcut::Forward)
    );
    assert_eq!(shortcut_of(&plain(Key::Tab)), None);
    assert_eq!(shortcut_of(&plain(Key::ArrowLeft)), None);
}
