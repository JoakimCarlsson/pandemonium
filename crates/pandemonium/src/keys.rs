//! Turning the window's keyboard events into the app's own.

use pm_input::{Key, KeyEvent, Modifiers};
use pm_text::{Edit, Motion};
use winit::event::{ElementState, KeyEvent as WindowKeyEvent};
use winit::keyboard::{Key as WindowKey, ModifiersState, NamedKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

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
pub fn key_event(event: &WindowKeyEvent, modifiers: Modifiers) -> Option<KeyEvent> {
    let key = match &event.logical_key {
        WindowKey::Named(NamedKey::Enter) => Key::Enter,
        WindowKey::Named(NamedKey::Escape) => Key::Escape,
        WindowKey::Named(NamedKey::Tab) => Key::Tab,
        WindowKey::Named(NamedKey::Backspace) => Key::Backspace,
        WindowKey::Named(NamedKey::Delete) => Key::Delete,
        WindowKey::Named(NamedKey::Home) => Key::Home,
        WindowKey::Named(NamedKey::End) => Key::End,
        WindowKey::Named(NamedKey::PageUp) => Key::PageUp,
        WindowKey::Named(NamedKey::PageDown) => Key::PageDown,
        WindowKey::Named(NamedKey::Space) => Key::Space,
        WindowKey::Named(NamedKey::ArrowUp) => Key::ArrowUp,
        WindowKey::Named(NamedKey::ArrowDown) => Key::ArrowDown,
        WindowKey::Named(NamedKey::ArrowLeft) => Key::ArrowLeft,
        WindowKey::Named(NamedKey::ArrowRight) => Key::ArrowRight,
        WindowKey::Named(NamedKey::F1) => Key::Function(1),
        WindowKey::Named(NamedKey::F2) => Key::Function(2),
        WindowKey::Named(NamedKey::F3) => Key::Function(3),
        WindowKey::Named(NamedKey::F4) => Key::Function(4),
        WindowKey::Named(NamedKey::F5) => Key::Function(5),
        WindowKey::Named(NamedKey::F6) => Key::Function(6),
        WindowKey::Named(NamedKey::F7) => Key::Function(7),
        WindowKey::Named(NamedKey::F8) => Key::Function(8),
        WindowKey::Named(NamedKey::F9) => Key::Function(9),
        WindowKey::Named(NamedKey::F10) => Key::Function(10),
        WindowKey::Named(NamedKey::F11) => Key::Function(11),
        WindowKey::Named(NamedKey::F12) => Key::Function(12),
        WindowKey::Character(text) => Key::Character(text.chars().next()?),
        _ => return None,
    };
    Some(KeyEvent {
        key,
        modifiers: unspent(event, modifiers),
        pressed: event.state == ElementState::Pressed,
    })
}

/// `modifiers` without the ones the keyboard layout spent composing the
/// character `event` typed, such as Option turning `a` into `å` on a Mac or
/// AltGr turning `2` into `@` elsewhere, so the character is typed rather
/// than taken for a shortcut.
fn unspent(event: &WindowKeyEvent, modifiers: Modifiers) -> Modifiers {
    let composed = modifiers.alt
        && matches!(
            (&event.logical_key, event.key_without_modifiers()),
            (WindowKey::Character(typed), WindowKey::Character(bare))
                if !typed.eq_ignore_ascii_case(&bare)
        );
    if composed {
        Modifiers {
            alt: false,
            control: false,
            ..modifiers
        }
    } else {
        modifiers
    }
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

/// The clipboard action a key event asks for in a terminal, where control alone belongs to the shell: Command, or Control with Shift.
pub fn terminal_clip(event: &KeyEvent) -> Option<Clip> {
    let modifiers = event.modifiers;
    let chord = modifiers.logo || (modifiers.control && modifiers.shift);
    if !event.pressed || modifiers.alt || !chord {
        return None;
    }
    match event.key {
        Key::Character(character) => match character.to_ascii_lowercase() {
            'c' => Some(Clip::Copy),
            'v' => Some(Clip::Paste),
            _ => None,
        },
        _ => None,
    }
}
