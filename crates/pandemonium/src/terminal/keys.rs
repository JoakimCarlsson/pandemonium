//! What the window's keyboard events mean to a terminal.
//!
//! The translation is a table and nothing else: winit says which key went
//! down, [`pm_vt`] says what bytes that key sends. Everything about modes and
//! sequences stays in `pm-vt`, and everything about the platform stays here.

use pm_vt::{Key, Modifiers};
use winit::keyboard::{Key as LogicalKey, ModifiersState, NamedKey};

/// The terminal key `key` stands for, if a terminal has one for it.
pub fn key(key: &LogicalKey) -> Option<Key> {
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
pub fn modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        control: state.control_key(),
        alt: state.alt_key(),
        shift: state.shift_key(),
    }
}
