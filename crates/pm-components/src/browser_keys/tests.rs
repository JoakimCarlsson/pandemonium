//! Tests of which keys the browser takes for itself.

use pm_input::{Key, KeyEvent, Modifiers};

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
    KeyEvent {
        key,
        modifiers: Modifiers {
            shift,
            control: true,
            alt: false,
            logo: false,
        },
        pressed: true,
    }
}

/// A press of `key` with no modifier.
fn plain(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers::default(),
        pressed: true,
    }
}

/// Control with a letter, a digit or a sign picks the browser's own commands, and plain letters go to the page.
#[test]
fn control_chords_are_the_browsers() {
    assert_eq!(
        shortcut_of(&control(Key::Character('f'), false)),
        Some(Shortcut::Find)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('F'), true)),
        Some(Shortcut::Find)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('g'), false)),
        Some(Shortcut::FindNext)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('g'), true)),
        Some(Shortcut::FindPrevious)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('='), false)),
        Some(Shortcut::ZoomIn)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('+'), true)),
        Some(Shortcut::ZoomIn)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('-'), false)),
        Some(Shortcut::ZoomOut)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('0'), false)),
        Some(Shortcut::ZoomReset)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('r'), false)),
        Some(Shortcut::Reload)
    );
    assert_eq!(
        shortcut_of(&control(Key::Character('R'), true)),
        Some(Shortcut::HardReload)
    );
    assert_eq!(shortcut_of(&plain(Key::Character('f'))), None);
    assert_eq!(shortcut_of(&control(Key::Character('c'), false)), None);
}

/// The function keys do what a browser's do.
#[test]
fn function_keys_step_and_reload() {
    assert_eq!(
        shortcut_of(&plain(Key::Function(3))),
        Some(Shortcut::FindNext)
    );
    assert_eq!(
        shortcut_of(&plain(Key::Function(5))),
        Some(Shortcut::Reload)
    );
    assert_eq!(
        shortcut_of(&control(Key::Function(5), false)),
        Some(Shortcut::HardReload)
    );
    assert_eq!(shortcut_of(&plain(Key::Function(1))), None);
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
    state.browser_find_edited(pm_text::Edit::Paste("ab".to_owned()));
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
    state.browser_found(1, 2, 5);
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
