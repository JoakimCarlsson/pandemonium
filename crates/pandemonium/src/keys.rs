//! Turning the window's keyboard events into the app's own.

use pm_input::{Key, KeyEvent, Modifiers};
use pm_text::Edit;
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
/// not edit text or is part of a shortcut.
pub fn edit(event: &KeyEvent) -> Option<Edit> {
    if !event.pressed || event.modifiers.control || event.modifiers.alt || event.modifiers.logo {
        return None;
    }
    match event.key {
        Key::Character(character) => Some(Edit::Insert(character)),
        Key::Space => Some(Edit::Insert(' ')),
        Key::Backspace => Some(Edit::Backspace),
        Key::Delete => Some(Edit::Delete),
        Key::ArrowLeft => Some(Edit::Left),
        Key::ArrowRight => Some(Edit::Right),
        Key::Home => Some(Edit::Home),
        Key::End => Some(Edit::End),
        _ => None,
    }
}
