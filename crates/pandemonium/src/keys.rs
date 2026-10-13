//! Turning the window's keyboard events into the app's own.

use pm_input::{Key, KeyEvent, Modifiers};
use pm_text::{Edit, Motion};
use winit::event::ElementState;
use winit::keyboard::{Key as WindowKey, ModifiersState, NamedKey};

/// The app's modifiers for the window's.
pub fn modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        shift: state.shift_key(),
        control: state.control_key(),
        alt: state.alt_key(),
        logo: state.super_key(),
    }
}

/// The app's key event for a key the window reported, or nothing for a key
/// the app has no name for.
pub fn key_event(key: &WindowKey, state: ElementState, modifiers: Modifiers) -> Option<KeyEvent> {
    let key = match key {
        WindowKey::Named(NamedKey::Enter) => Key::Enter,
        WindowKey::Named(NamedKey::Escape) => Key::Escape,
        WindowKey::Named(NamedKey::Tab) => Key::Tab,
        WindowKey::Named(NamedKey::Backspace) => Key::Backspace,
        WindowKey::Named(NamedKey::Delete) => Key::Delete,
        WindowKey::Named(NamedKey::Home) => Key::Home,
        WindowKey::Named(NamedKey::End) => Key::End,
        WindowKey::Named(NamedKey::Space) => Key::Space,
        WindowKey::Named(NamedKey::ArrowUp) => Key::ArrowUp,
        WindowKey::Named(NamedKey::ArrowDown) => Key::ArrowDown,
        WindowKey::Named(NamedKey::ArrowLeft) => Key::ArrowLeft,
        WindowKey::Named(NamedKey::ArrowRight) => Key::ArrowRight,
        WindowKey::Character(text) => Key::Character(text.chars().next()?),
        _ => return None,
    };
    Some(KeyEvent {
        key,
        modifiers,
        pressed: state == ElementState::Pressed,
    })
}

/// The edit a key event asks of a text field, or nothing for a key that does
/// not edit text or is a shortcut the window handles itself, such as copy.
pub fn edit(event: &KeyEvent) -> Option<Edit> {
    let modifiers = event.modifiers;
    if !event.pressed || modifiers.alt {
        return None;
    }
    let command = modifiers.control || modifiers.logo;
    let shift = modifiers.shift;
    match event.key {
        Key::Character(character) if command => match character.to_ascii_lowercase() {
            'a' => Some(Edit::SelectAll),
            'z' if shift => Some(Edit::Redo),
            'z' => Some(Edit::Undo),
            'y' => Some(Edit::Redo),
            _ => None,
        },
        Key::Character(character) => Some(Edit::Insert(character)),
        Key::Space if !command => Some(Edit::Insert(' ')),
        Key::Backspace => Some(by_word(command, Edit::Backspace, Edit::BackspaceWord)),
        Key::Delete => Some(by_word(command, Edit::Delete, Edit::DeleteWord)),
        Key::ArrowLeft => Some(Edit::Move(
            by_word(command, Motion::Left, Motion::WordLeft),
            shift,
        )),
        Key::ArrowRight => Some(Edit::Move(
            by_word(command, Motion::Right, Motion::WordRight),
            shift,
        )),
        Key::Home => Some(Edit::Move(Motion::Home, shift)),
        Key::End => Some(Edit::Move(Motion::End, shift)),
        _ => None,
    }
}

/// `by_word` while the command key is held, `plain` otherwise.
fn by_word<T>(command: bool, plain: T, by_word: T) -> T {
    if command { by_word } else { plain }
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
    if !event.pressed || modifiers.alt || !(modifiers.control || modifiers.logo) {
        return None;
    }
    match event.key {
        Key::Character(character) => match character.to_ascii_lowercase() {
            'c' => Some(Clip::Copy),
            'x' => Some(Clip::Cut),
            'v' => Some(Clip::Paste),
            _ => None,
        },
        _ => None,
    }
}
