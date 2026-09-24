//! Turning a winit key event into a [`Chord`].
//!
//! The key a chord names is the one printed on a US keyboard, taken from the
//! physical key rather than the character the layout produces, so `primary+\`
//! stays one binding whatever shift, alt or the layout would otherwise make of
//! it. Layout-aware labels — showing a Swedish keyboard what to press — are a
//! later concern; the binding itself is physical.

use winit::event::KeyEvent;
use winit::keyboard::{Key as LogicalKey, KeyCode, ModifiersState, NamedKey, PhysicalKey};

use crate::keymap::chord::Chord;
use crate::keymap::key::{Key, Modifiers, Named};

/// The chord `event` presses, or nothing if it presses no key a keymap names.
pub fn chord(event: &KeyEvent, modifiers: ModifiersState) -> Option<Chord> {
    let key = match event.physical_key {
        PhysicalKey::Code(code) => from_code(code).or_else(|| from_logical(&event.logical_key)),
        PhysicalKey::Unidentified(_) => from_logical(&event.logical_key),
    }?;

    Some(Chord::new(from_modifiers(modifiers), key))
}

/// The modifiers winit reports, as a keymap holds them.
fn from_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        control: modifiers.control_key(),
        shift: modifiers.shift_key(),
        alt: modifiers.alt_key(),
        command: modifiers.super_key(),
    }
}

/// The key a physical code stands for on a US keyboard.
fn from_code(code: KeyCode) -> Option<Key> {
    let character = match code {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Digit0 | KeyCode::Numpad0 => '0',
        KeyCode::Digit1 | KeyCode::Numpad1 => '1',
        KeyCode::Digit2 | KeyCode::Numpad2 => '2',
        KeyCode::Digit3 | KeyCode::Numpad3 => '3',
        KeyCode::Digit4 | KeyCode::Numpad4 => '4',
        KeyCode::Digit5 | KeyCode::Numpad5 => '5',
        KeyCode::Digit6 | KeyCode::Numpad6 => '6',
        KeyCode::Digit7 | KeyCode::Numpad7 => '7',
        KeyCode::Digit8 | KeyCode::Numpad8 => '8',
        KeyCode::Digit9 | KeyCode::Numpad9 => '9',
        KeyCode::Backquote => '`',
        KeyCode::Backslash => '\\',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Comma | KeyCode::NumpadComma => ',',
        KeyCode::Equal | KeyCode::NumpadEqual => '=',
        KeyCode::Minus | KeyCode::NumpadSubtract => '-',
        KeyCode::Period | KeyCode::NumpadDecimal => '.',
        KeyCode::Quote => '\'',
        KeyCode::Semicolon => ';',
        KeyCode::Slash | KeyCode::NumpadDivide => '/',
        KeyCode::Escape => return Some(Key::Named(Named::Escape)),
        KeyCode::Enter | KeyCode::NumpadEnter => return Some(Key::Named(Named::Enter)),
        KeyCode::Tab => return Some(Key::Named(Named::Tab)),
        KeyCode::Space => return Some(Key::Named(Named::Space)),
        KeyCode::Backspace => return Some(Key::Named(Named::Backspace)),
        KeyCode::Delete => return Some(Key::Named(Named::Delete)),
        KeyCode::Insert => return Some(Key::Named(Named::Insert)),
        KeyCode::Home => return Some(Key::Named(Named::Home)),
        KeyCode::End => return Some(Key::Named(Named::End)),
        KeyCode::PageUp => return Some(Key::Named(Named::PageUp)),
        KeyCode::PageDown => return Some(Key::Named(Named::PageDown)),
        KeyCode::ArrowUp => return Some(Key::Named(Named::Up)),
        KeyCode::ArrowDown => return Some(Key::Named(Named::Down)),
        KeyCode::ArrowLeft => return Some(Key::Named(Named::Left)),
        KeyCode::ArrowRight => return Some(Key::Named(Named::Right)),
        _ => return function_key(code),
    };

    Some(Key::Character(character))
}

/// The function key a physical code stands for, if it is one.
fn function_key(code: KeyCode) -> Option<Key> {
    let number = match code {
        KeyCode::F1 => 1,
        KeyCode::F2 => 2,
        KeyCode::F3 => 3,
        KeyCode::F4 => 4,
        KeyCode::F5 => 5,
        KeyCode::F6 => 6,
        KeyCode::F7 => 7,
        KeyCode::F8 => 8,
        KeyCode::F9 => 9,
        KeyCode::F10 => 10,
        KeyCode::F11 => 11,
        KeyCode::F12 => 12,
        KeyCode::F13 => 13,
        KeyCode::F14 => 14,
        KeyCode::F15 => 15,
        KeyCode::F16 => 16,
        KeyCode::F17 => 17,
        KeyCode::F18 => 18,
        KeyCode::F19 => 19,
        KeyCode::F20 => 20,
        KeyCode::F21 => 21,
        KeyCode::F22 => 22,
        KeyCode::F23 => 23,
        KeyCode::F24 => 24,
        _ => return None,
    };

    Some(Key::Function(number))
}

/// The key a logical key stands for, for events with no physical code.
fn from_logical(key: &LogicalKey) -> Option<Key> {
    match key {
        LogicalKey::Character(text) => {
            let mut characters = text.chars().flat_map(char::to_lowercase);
            match (characters.next(), characters.next()) {
                (Some(character), None) => Some(Key::Character(character)),
                _ => None,
            }
        }
        LogicalKey::Named(named) => named_key(*named),
        _ => None,
    }
}

/// The key a winit named key stands for, if a keymap names it too.
fn named_key(named: NamedKey) -> Option<Key> {
    let key = match named {
        NamedKey::Escape => Named::Escape,
        NamedKey::Enter => Named::Enter,
        NamedKey::Tab => Named::Tab,
        NamedKey::Space => Named::Space,
        NamedKey::Backspace => Named::Backspace,
        NamedKey::Delete => Named::Delete,
        NamedKey::Insert => Named::Insert,
        NamedKey::Home => Named::Home,
        NamedKey::End => Named::End,
        NamedKey::PageUp => Named::PageUp,
        NamedKey::PageDown => Named::PageDown,
        NamedKey::ArrowUp => Named::Up,
        NamedKey::ArrowDown => Named::Down,
        NamedKey::ArrowLeft => Named::Left,
        NamedKey::ArrowRight => Named::Right,
        _ => return None,
    };

    Some(Key::Named(key))
}
