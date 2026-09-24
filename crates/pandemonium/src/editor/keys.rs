//! What a keypress does to a buffer.
//!
//! A pane with the keyboard takes almost every key: the text it is editing
//! is what the keys are for. What it does not take are the window's own
//! chords, which the window has already resolved by the time a keypress
//! reaches here — so everything left is either a character to put in, a
//! character to take out, or a way of moving the cursor.

use pm_text::Motion;
use winit::keyboard::ModifiersState;

use crate::keymap::Travel;
use winit::keyboard::{Key, NamedKey};

/// One thing a keypress asks of the buffer it lands in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Edit {
    /// Put this character in, closing whatever it opens.
    Type(char),
    /// Put this text in, replacing what is selected.
    Insert(String),
    /// Put a line break in, indented as the line before it is.
    Newline,
    /// Put one step of indentation in, or indent what is selected.
    Indent,
    /// Take one step of indentation off what is selected.
    Outdent,
    /// Take out what is selected, or the character before the cursor.
    Backspace,
    /// Take out what is selected, or the character after the cursor.
    Delete,
    /// Take out the word before the cursor.
    DeleteWordLeft,
    /// Take out the word after the cursor.
    DeleteWordRight,
    /// Take out what lies between the cursor and where this motion takes it.
    DeleteTo(Motion),
    /// Move the cursor, extending the selection when asked.
    Move(Motion, bool),
}

impl Edit {
    /// Does the edit to `buffer` at its cursor.
    pub fn apply(&self, buffer: &mut pm_text::Buffer) {
        match self {
            Self::Type(ch) => buffer.insert_typed(*ch),
            Self::Insert(text) => buffer.insert(text),
            Self::Newline => buffer.insert_newline(),
            Self::Indent => buffer.insert_indent(),
            Self::Outdent => buffer.outdent_lines(),
            Self::Backspace => buffer.backspace(),
            Self::Delete => buffer.delete(),
            Self::DeleteWordLeft => buffer.delete_word_left(),
            Self::DeleteWordRight => buffer.delete_word_right(),
            Self::DeleteTo(motion) => {
                buffer.move_cursor(*motion, true);
                if !buffer.selection().is_empty() {
                    buffer.backspace();
                }
            }
            Self::Move(motion, extend) => buffer.move_cursor(*motion, *extend),
        }
    }
}

/// The motion `travel` names, a page being `rows` lines.
pub fn motion(travel: Travel, rows: usize) -> Motion {
    match travel {
        Travel::Left => Motion::Left,
        Travel::Right => Motion::Right,
        Travel::Up => Motion::Up,
        Travel::Down => Motion::Down,
        Travel::WordLeft => Motion::WordLeft,
        Travel::WordRight => Motion::WordRight,
        Travel::LineStart => Motion::LineStart,
        Travel::LineEnd => Motion::LineEnd,
        Travel::BufferStart => Motion::BufferStart,
        Travel::BufferEnd => Motion::BufferEnd,
        Travel::PageUp => Motion::PageUp(rows),
        Travel::PageDown => Motion::PageDown(rows),
    }
}

/// What `key` asks of the buffer, given the modifiers held with it.
///
/// A key the buffer has nothing to do with comes back as `None`, so that the
/// window can go on to treat it as focus movement the way it always does.
pub fn edit(key: &Key, modifiers: ModifiersState, rows: usize) -> Option<Edit> {
    let extend = modifiers.shift_key();
    let word = modifiers.control_key() && !modifiers.alt_key();
    let composed = modifiers.control_key() && modifiers.alt_key();
    let plain = !modifiers.control_key() && !modifiers.super_key() && !modifiers.alt_key();

    if word {
        match key.as_ref() {
            Key::Named(NamedKey::Home) => {
                return Some(Edit::Move(Motion::BufferStart, extend));
            }
            Key::Named(NamedKey::End) => return Some(Edit::Move(Motion::BufferEnd, extend)),
            Key::Named(NamedKey::Backspace) => return Some(Edit::DeleteWordLeft),
            Key::Named(NamedKey::Delete) => return Some(Edit::DeleteWordRight),
            _ => {}
        }
    }

    match key.as_ref() {
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
        Key::Named(NamedKey::Tab) if extend => Some(Edit::Outdent),
        Key::Named(NamedKey::Tab) => Some(Edit::Indent),
        Key::Named(NamedKey::Space) if plain => Some(Edit::Insert(" ".to_owned())),
        Key::Character(text) if plain || composed => Some(typed(text)),
        _ => None,
    }
}

/// `key` as modal editing reads it, given the modifiers held with it.
///
/// Alt and the platform key are the window's, so a key held with either is
/// none of modal editing's business; Ctrl with Alt is how some layouts type
/// a character, and counts as typing it.
pub fn keystroke(key: &Key, modifiers: ModifiersState) -> Option<pm_vim::Keystroke> {
    let composed = modifiers.control_key() && modifiers.alt_key();
    if modifiers.super_key() || (modifiers.alt_key() && !composed) {
        return None;
    }
    let pressed = match key.as_ref() {
        Key::Character(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => pm_vim::Key::Char(ch),
                _ => return None,
            }
        }
        Key::Named(NamedKey::Space) => pm_vim::Key::Char(' '),
        Key::Named(NamedKey::Escape) => pm_vim::Key::Escape,
        Key::Named(NamedKey::Enter) => pm_vim::Key::Enter,
        Key::Named(NamedKey::Backspace) => pm_vim::Key::Backspace,
        Key::Named(NamedKey::Delete) => pm_vim::Key::Delete,
        Key::Named(NamedKey::Insert) => pm_vim::Key::Insert,
        Key::Named(NamedKey::Tab) => pm_vim::Key::Tab,
        Key::Named(NamedKey::ArrowLeft) => pm_vim::Key::Left,
        Key::Named(NamedKey::ArrowRight) => pm_vim::Key::Right,
        Key::Named(NamedKey::ArrowUp) => pm_vim::Key::Up,
        Key::Named(NamedKey::ArrowDown) => pm_vim::Key::Down,
        Key::Named(NamedKey::Home) => pm_vim::Key::Home,
        Key::Named(NamedKey::End) => pm_vim::Key::End,
        Key::Named(NamedKey::PageUp) => pm_vim::Key::PageUp,
        Key::Named(NamedKey::PageDown) => pm_vim::Key::PageDown,
        _ => return None,
    };
    let ctrl = modifiers.control_key() && !composed;
    let pressed = match (pressed, ctrl) {
        (pm_vim::Key::Char(ch), true) => pm_vim::Key::Char(ch.to_ascii_lowercase()),
        (pressed, _) => pressed,
    };
    Some(pm_vim::Keystroke {
        key: pressed,
        ctrl,
        shift: modifiers.shift_key() && !matches!(pressed, pm_vim::Key::Char(_)),
    })
}

/// What typing `text` asks for: one character closes its pair, more do not.
fn typed(text: &str) -> Edit {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Edit::Type(ch),
        _ => Edit::Insert(text.to_owned()),
    }
}
