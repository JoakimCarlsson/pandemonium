//! What a keypress does to a buffer.
//!
//! A pane with the keyboard takes almost every key: the text it is editing
//! is what the keys are for. What it does not take are the window's own
//! chords, which the window has already resolved by the time a keypress
//! reaches here.

use pm_text::Motion;
use winit::keyboard::ModifiersState;
use winit::keyboard::{Key, NamedKey};

/// One thing a keypress asks of the buffer it lands in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Edit {
    /// Put this text in, replacing what is selected.
    Insert(String),
    /// Put a line break in, indented as the line before it is.
    Newline,
    /// Put one step of indentation in.
    Indent,
    /// Take out what is selected, or the character before the cursor.
    Backspace,
    /// Take out what is selected, or the character after the cursor.
    Delete,
    /// Move the cursor, extending the selection when asked.
    Move(Motion, bool),
    /// Select the whole buffer.
    SelectAll,
}

/// What `key` asks of the buffer, given the modifiers held with it.
///
/// A key the buffer has nothing to do with comes back as `None`, so that the
/// window can go on to treat it as focus movement the way it always does.
pub fn edit(key: &Key, modifiers: ModifiersState, rows: usize) -> Option<Edit> {
    let extend = modifiers.shift_key();
    let word = modifiers.control_key();

    if modifiers.control_key() && !modifiers.alt_key() {
        match key.as_ref() {
            Key::Character("a") => return Some(Edit::SelectAll),
            Key::Named(NamedKey::Home) => {
                return Some(Edit::Move(Motion::BufferStart, extend));
            }
            Key::Named(NamedKey::End) => return Some(Edit::Move(Motion::BufferEnd, extend)),
            _ => {}
        }
    }

    match key {
        Key::Named(NamedKey::ArrowLeft) if word => Some(Edit::Move(Motion::WordLeft, extend)),
        Key::Named(NamedKey::ArrowRight) if word => Some(Edit::Move(Motion::WordRight, extend)),
        Key::Named(NamedKey::ArrowLeft) => Some(Edit::Move(Motion::Left, extend)),
        Key::Named(NamedKey::ArrowRight) => Some(Edit::Move(Motion::Right, extend)),
        Key::Named(NamedKey::ArrowUp) => Some(Edit::Move(Motion::Up, extend)),
        Key::Named(NamedKey::ArrowDown) => Some(Edit::Move(Motion::Down, extend)),
        Key::Named(NamedKey::PageUp) => Some(Edit::Move(Motion::PageUp(rows), extend)),
        Key::Named(NamedKey::PageDown) => Some(Edit::Move(Motion::PageDown(rows), extend)),
        Key::Named(NamedKey::Home) => Some(Edit::Move(Motion::LineStart, extend)),
        Key::Named(NamedKey::End) => Some(Edit::Move(Motion::LineEnd, extend)),
        Key::Named(NamedKey::Backspace) => Some(Edit::Backspace),
        Key::Named(NamedKey::Delete) => Some(Edit::Delete),
        Key::Named(NamedKey::Enter) => Some(Edit::Newline),
        Key::Named(NamedKey::Tab) => Some(Edit::Indent),
        Key::Named(NamedKey::Space) if !modifiers.control_key() => {
            Some(Edit::Insert(" ".to_owned()))
        }
        Key::Character(text) if !modifiers.control_key() && !modifiers.super_key() => {
            Some(Edit::Insert(text.to_string()))
        }
        _ => None,
    }
}
