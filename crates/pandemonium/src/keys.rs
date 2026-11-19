//! What the window's keys mean to the app: which keys the app reacts to, and the text edits and clipboard actions a key event asks for. The app
//! matches on rustle's [`KeyEvent`]s directly; these turn them into the plain values of the
//! crates below it, such as [`pm_field::Edit`].

use pm_field::{Edit, Motion};
use rustle::{Key, KeyEvent};

/// Whether the app reacts to the key of `event`: a character it typed, a named key it has
/// a use for or a function key up to F12; not a modifier pressed on its own, nor text an
/// input method committed.
pub fn is_app_key(event: &KeyEvent) -> bool {
    match &event.key {
        Key::Character(_) => event.code_point().is_some(),
        Key::F(number) => (1..=12).contains(number),
        Key::Enter
        | Key::Escape
        | Key::Tab
        | Key::Backspace
        | Key::Delete
        | Key::Home
        | Key::End
        | Key::PageUp
        | Key::PageDown
        | Key::ArrowUp
        | Key::ArrowDown
        | Key::ArrowLeft
        | Key::ArrowRight => true,
        _ => false,
    }
}

/// Whether `event` is the space bar.
pub fn is_space(event: &KeyEvent) -> bool {
    matches!(&event.key, Key::Character(text) if text == " ")
}

/// The character `event` typed, Shift and the layout applied, when it is a character key
/// other than the space bar.
pub fn character(event: &KeyEvent) -> Option<char> {
    match &event.key {
        Key::Character(_) if !is_space(event) => event.code_point(),
        _ => None,
    }
}

/// The character `event` typed, lower-cased, when it is a character key other than the
/// space bar, as shortcuts that ignore Shift and Caps Lock match it.
pub fn letter(event: &KeyEvent) -> Option<char> {
    character(event).map(|character| character.to_ascii_lowercase())
}

/// The edit a key event asks of a text field, or nothing for a key that does
/// not edit text or is a shortcut the window handles itself, such as copy.
pub fn edit(event: &KeyEvent) -> Option<Edit> {
    let modifiers = event.modifiers;
    let option = cfg!(target_os = "macos")
        && modifiers.alt
        && matches!(
            event.key,
            Key::Backspace | Key::Delete | Key::ArrowLeft | Key::ArrowRight
        );
    if !event.pressed || (modifiers.alt && !option) {
        return None;
    }
    let command = modifiers.ctrl || modifiers.meta;
    let word = command || option;
    let shift = modifiers.shift;
    if is_space(event) {
        return (!command).then_some(Edit::Insert(' '));
    }
    if let Some(character) = character(event) {
        return if command {
            match character.to_ascii_lowercase() {
                'a' => Some(Edit::SelectAll),
                'z' if shift => Some(Edit::Redo),
                'z' => Some(Edit::Undo),
                'y' => Some(Edit::Redo),
                _ => None,
            }
        } else {
            Some(Edit::Insert(character))
        };
    }
    match event.key {
        Key::Backspace => Some(by_word(word, Edit::Backspace, Edit::BackspaceWord)),
        Key::Delete => Some(by_word(word, Edit::Delete, Edit::DeleteWord)),
        Key::ArrowLeft => Some(Edit::Move(
            by_word(word, Motion::Left, Motion::WordLeft),
            shift,
        )),
        Key::ArrowRight => Some(Edit::Move(
            by_word(word, Motion::Right, Motion::WordRight),
            shift,
        )),
        Key::Home => Some(Edit::Move(Motion::Home, shift)),
        Key::End => Some(Edit::Move(Motion::End, shift)),
        _ => None,
    }
}

/// `by_word` while a word modifier is held, `plain` otherwise.
fn by_word<T>(word: bool, plain: T, by_word: T) -> T {
    if word { by_word } else { plain }
}

/// What a shortcut asks of the clipboard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Clip {
    /// Copy what is selected.
    Copy,
    /// Copy what is selected and delete it.
    Cut,
    /// Insert what the clipboard holds.
    Paste,
}

/// The clipboard action a key event asks for, if it is one of the shortcuts for it.
pub fn clip(event: &KeyEvent) -> Option<Clip> {
    let modifiers = event.modifiers;
    if !event.pressed || modifiers.alt || !(modifiers.ctrl || modifiers.meta) {
        return None;
    }
    match letter(event)? {
        'c' => Some(Clip::Copy),
        'x' => Some(Clip::Cut),
        'v' => Some(Clip::Paste),
        _ => None,
    }
}

/// The clipboard action Command with C or V asks for in a terminal, the keys a Mac copies and pastes with; Control belongs to the shell, and its keys for the clipboard are the keymap's.
pub fn terminal_clip(event: &KeyEvent) -> Option<Clip> {
    let modifiers = event.modifiers;
    if !event.pressed || modifiers.alt || modifiers.shift || !modifiers.meta {
        return None;
    }
    match letter(event)? {
        'c' => Some(Clip::Copy),
        'v' => Some(Clip::Paste),
        _ => None,
    }
}
