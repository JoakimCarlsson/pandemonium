//! What the window's keyboard events mean to a terminal.
//!
//! The translation is a table and nothing else: winit says which key went
//! down, [`pm_vt`] says what bytes that key sends. Everything about modes and
//! sequences stays in `pm-vt`, and everything about the platform stays here.
//! On macOS the platform's word and line chords are their own table, because
//! a terminal there sends different bytes for them than the generic encoding.

use pm_vt::{Key, Modifiers};
use winit::event::KeyEvent;
use winit::keyboard::{Key as LogicalKey, ModifiersState, NamedKey};

use crate::editor;
use crate::keymap::Action;

/// The terminal key `event` stands for, if a terminal has one for it.
///
/// An Option key that [composed a character](crate::editor::option_text) is
/// that character, so the terminal receives it rather than an escape in front
/// of the key that produced it.
pub fn key(event: &KeyEvent, modifiers: ModifiersState) -> Option<Key> {
    if let Some(text) = editor::option_text(event, modifiers) {
        return text.chars().next().map(Key::Char);
    }
    let key = &event.logical_key;
    let named = match key {
        LogicalKey::Character(text) => return text.chars().next().map(Key::Char),
        LogicalKey::Named(named) => named,
        _ => return None,
    };

    let key = match named {
        NamedKey::Space => Key::Char(' '),
        NamedKey::Enter => Key::Enter,
        NamedKey::Tab => Key::Tab,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Escape => Key::Escape,
        NamedKey::Insert => Key::Insert,
        NamedKey::Delete => Key::Delete,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::F1 => Key::Function(1),
        NamedKey::F2 => Key::Function(2),
        NamedKey::F3 => Key::Function(3),
        NamedKey::F4 => Key::Function(4),
        NamedKey::F5 => Key::Function(5),
        NamedKey::F6 => Key::Function(6),
        NamedKey::F7 => Key::Function(7),
        NamedKey::F8 => Key::Function(8),
        NamedKey::F9 => Key::Function(9),
        NamedKey::F10 => Key::Function(10),
        NamedKey::F11 => Key::Function(11),
        NamedKey::F12 => Key::Function(12),
        _ => return None,
    };
    Some(key)
}

/// The modifiers a terminal cares about, out of the ones being held.
///
/// An Option key that composed a character is that character alone: alt is
/// not sent with it, or the terminal would write an escape in front.
pub fn modifiers(state: ModifiersState, event: &KeyEvent) -> Modifiers {
    Modifiers {
        control: state.control_key(),
        alt: state.alt_key() && editor::option_text(event, state).is_none(),
        shift: state.shift_key(),
    }
}

/// The key a macOS terminal sends for Option or Command word and line chords.
///
/// Option with Left or Right is `ESC b` and `ESC f`, and Command with Left,
/// Right or Backspace is `^A`, `^E` and `^U`. Option with Backspace is left
/// to the ordinary encoding, which already sends `ESC DEL`. Anywhere but
/// macOS this is nothing.
pub fn macos(key: &LogicalKey, state: ModifiersState) -> Option<(Key, Modifiers)> {
    if !cfg!(target_os = "macos") || state.control_key() || state.shift_key() {
        return None;
    }
    let (character, modifiers) = match (key, state.alt_key(), state.super_key()) {
        (LogicalKey::Named(NamedKey::ArrowLeft), true, false) => (
            'b',
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
        ),
        (LogicalKey::Named(NamedKey::ArrowRight), true, false) => (
            'f',
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
        ),
        (LogicalKey::Named(NamedKey::ArrowLeft), false, true) => (
            'a',
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        ),
        (LogicalKey::Named(NamedKey::ArrowRight), false, true) => (
            'e',
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        ),
        (LogicalKey::Named(NamedKey::Backspace), false, true) => (
            'u',
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        ),
        _ => return None,
    };
    Some((Key::Char(character), modifiers))
}

/// The clipboard command `key` stands for in a terminal, if it stands for one.
///
/// Control-Shift-C and Control-Insert copy, and Control-Shift-V pastes.
/// Shift-Insert pastes as it always has, and Control-V pastes too, the way it
/// does everywhere else in the window. Plain Control-C copies only while
/// something is `selected`; otherwise it is the interrupt it has always been.
pub fn clipboard(key: &LogicalKey, state: ModifiersState, selected: bool) -> Option<Action> {
    if state.alt_key() || state.super_key() {
        return None;
    }
    let (control, shift) = (state.control_key(), state.shift_key());
    let letter = match key {
        LogicalKey::Character(text) => text.chars().next().map(|ch| ch.to_ascii_lowercase()),
        LogicalKey::Named(NamedKey::Insert) if control && !shift => return Some(Action::Copy),
        LogicalKey::Named(NamedKey::Insert) if shift && !control => return Some(Action::Paste),
        _ => None,
    }?;
    match (letter, control, shift) {
        ('c', true, true) => Some(Action::Copy),
        ('c', true, false) if selected => Some(Action::Copy),
        ('v', true, _) => Some(Action::Paste),
        _ => None,
    }
}
