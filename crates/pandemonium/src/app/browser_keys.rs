//! The keys of the keyboard as the browser engine names them: the Windows virtual-key code
//! of each key, found from what it typed and, where that says nothing, from where it sits
//! on the keyboard, with the text each press types into a page.

use pm_browser::{Key as Typed, Modifiers as Held};
use rustle::{Key, KeyCode, KeyEvent, KeyLocation, PhysicalKey};

use crate::keys;

/// The press or release of `event` as the engine is told it, `modifiers` held: the key's
/// virtual-key code and the text it types, which it types only while no modifier that makes
/// a shortcut, Control, Alt or Command, is held.
pub(super) fn typed(event: &KeyEvent, modifiers: Held) -> Typed {
    let plain = modifiers.0 & (Held::CONTROL | Held::ALT | Held::COMMAND) == 0;
    Typed {
        pressed: event.pressed,
        code: code_of(event),
        text: plain.then(|| text_of(event)).flatten(),
        modifiers,
    }
}

/// The text the key of `event` types: its character, a space, a carriage return for Enter
/// and a tab for Tab; nothing for a dead key, which types with the key after it, or for a
/// key that only moves or edits.
fn text_of(event: &KeyEvent) -> Option<String> {
    match &event.key {
        Key::Character(_) if keys::is_space(event) => Some(" ".to_owned()),
        Key::Character(_) | Key::Other => event
            .text
            .clone()
            .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
            .or_else(|| event.code_point().map(String::from)),
        Key::Enter => Some("\r".to_owned()),
        Key::Tab => Some("\t".to_owned()),
        _ => None,
    }
}

/// The virtual-key code of the key of `event`: by its name for a named key, by its place
/// for a key of the number pad, by its character for a letter, digit or punctuation mark of
/// a US layout, and by its place on the keyboard for a dead key, a character of another
/// layout and the media keys; 0 when none of those names it.
fn code_of(event: &KeyEvent) -> i32 {
    match &event.key {
        Key::Character(_) if keys::is_space(event) => 0x20,
        Key::Character(_) if event.location == KeyLocation::Numpad => {
            physical_code(event.physical_key)
                .or_else(|| event.code_point().and_then(character_code))
                .unwrap_or(0)
        }
        Key::Character(_) => event
            .code_point()
            .and_then(character_code)
            .or_else(|| physical_code(event.physical_key))
            .unwrap_or(0),
        key => named_code(key)
            .or_else(|| physical_code(event.physical_key))
            .unwrap_or(0),
    }
}

/// The virtual-key code of the named key `key`, when it is one the engine has a code for.
fn named_code(key: &Key) -> Option<i32> {
    let code = match key {
        Key::Enter => 0x0D,
        Key::Escape => 0x1B,
        Key::Tab => 0x09,
        Key::Backspace => 0x08,
        Key::Delete => 0x2E,
        Key::Insert => 0x2D,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::ArrowLeft => 0x25,
        Key::ArrowUp => 0x26,
        Key::ArrowRight => 0x27,
        Key::ArrowDown => 0x28,
        Key::PrintScreen => 0x2C,
        Key::ContextMenu => 0x5D,
        Key::Pause => 0x13,
        Key::Help => 0x2F,
        Key::Clear => 0x0C,
        Key::CapsLock => 0x14,
        Key::NumLock => 0x90,
        Key::ScrollLock => 0x91,
        Key::Shift => 0x10,
        Key::Control => 0x11,
        Key::Alt => 0x12,
        Key::Super => 0x5B,
        Key::F(number @ 1..=24) => 0x6F + i32::from(*number),
        _ => return None,
    };
    Some(code)
}

/// The virtual-key code of the key that types `character` on a US layout, Shift or not:
/// letters, digits and the punctuation keys; nothing for a character no such key types
/// unshifted or shifted as a punctuation key.
fn character_code(character: char) -> Option<i32> {
    let upper = character.to_ascii_uppercase();
    let code = match upper {
        'A'..='Z' | '0'..='9' => upper as i32,
        ';' | ':' => 0xBA,
        '=' | '+' => 0xBB,
        ',' | '<' => 0xBC,
        '-' | '_' => 0xBD,
        '.' | '>' => 0xBE,
        '/' | '?' => 0xBF,
        '`' | '~' => 0xC0,
        '[' | '{' => 0xDB,
        '\\' | '|' => 0xDC,
        ']' | '}' => 0xDD,
        '\'' | '"' => 0xDE,
        _ => return None,
    };
    Some(code)
}

/// The virtual-key code of the key at `physical` on the keyboard, whatever the layout makes
/// it type: the letters, digits and punctuation keys by their US places, the number pad,
/// the media, volume and browser keys and the locks.
fn physical_code(physical: PhysicalKey) -> Option<i32> {
    let code = physical.code()?;
    if let Some(code) = numbered_code(code.name()) {
        return Some(code);
    }
    let code = match code {
        KeyCode::Backquote => 0xC0,
        KeyCode::Backslash => 0xDC,
        KeyCode::BracketLeft => 0xDB,
        KeyCode::BracketRight => 0xDD,
        KeyCode::Comma => 0xBC,
        KeyCode::Equal => 0xBB,
        KeyCode::IntlBackslash => 0xE2,
        KeyCode::IntlRo => 0xC1,
        KeyCode::Minus => 0xBD,
        KeyCode::Period => 0xBE,
        KeyCode::Quote => 0xDE,
        KeyCode::Semicolon => 0xBA,
        KeyCode::Slash => 0xBF,
        KeyCode::NumpadAdd => 0x6B,
        KeyCode::NumpadSubtract => 0x6D,
        KeyCode::NumpadMultiply | KeyCode::NumpadStar => 0x6A,
        KeyCode::NumpadDivide => 0x6F,
        KeyCode::NumpadDecimal => 0x6E,
        KeyCode::NumpadComma => 0x6C,
        KeyCode::NumpadEnter => 0x0D,
        KeyCode::NumpadEqual => 0xBB,
        KeyCode::MediaPlayPause => 0xB3,
        KeyCode::MediaStop => 0xB2,
        KeyCode::MediaTrackNext => 0xB0,
        KeyCode::MediaTrackPrevious => 0xB1,
        KeyCode::MediaSelect => 0xB5,
        KeyCode::AudioVolumeMute => 0xAD,
        KeyCode::AudioVolumeDown => 0xAE,
        KeyCode::AudioVolumeUp => 0xAF,
        KeyCode::BrowserBack => 0xA6,
        KeyCode::BrowserForward => 0xA7,
        KeyCode::BrowserRefresh => 0xA8,
        KeyCode::BrowserStop => 0xA9,
        KeyCode::BrowserSearch => 0xAA,
        KeyCode::BrowserFavorites => 0xAB,
        KeyCode::BrowserHome => 0xAC,
        KeyCode::LaunchMail => 0xB4,
        KeyCode::LaunchApp1 => 0xB6,
        KeyCode::LaunchApp2 => 0xB7,
        KeyCode::ContextMenu => 0x5D,
        KeyCode::CapsLock => 0x14,
        KeyCode::NumLock => 0x90,
        KeyCode::ScrollLock => 0x91,
        KeyCode::Insert => 0x2D,
        KeyCode::PrintScreen => 0x2C,
        KeyCode::Pause => 0x13,
        KeyCode::Sleep => 0x5F,
        KeyCode::Convert => 0x1C,
        KeyCode::NonConvert => 0x1D,
        KeyCode::KanaMode => 0x15,
        _ => return None,
    };
    Some(code)
}

/// The virtual-key code of the letter, digit or number-pad digit key named `name` as
/// [`KeyCode::name`] names it, such as `KeyA`, `Digit1` or `Numpad1`.
fn numbered_code(name: &str) -> Option<i32> {
    let single = |rest: &str| {
        let mut characters = rest.chars();
        characters.next().filter(|_| characters.next().is_none())
    };
    if let Some(letter) = name.strip_prefix("Key").and_then(single) {
        return letter.is_ascii_uppercase().then_some(letter as i32);
    }
    if let Some(digit) = name.strip_prefix("Digit").and_then(single) {
        return digit.is_ascii_digit().then_some(digit as i32);
    }
    let digit = name.strip_prefix("Numpad").and_then(single)?;
    digit
        .to_digit(10)
        .and_then(|digit| i32::try_from(digit).ok())
        .map(|digit| 0x60 + digit)
}

/// Tests of the codes and text keys are sent to a page with.
#[cfg(test)]
mod tests {
    use super::*;
    use rustle::Modifiers;

    /// A press of `key` at `physical` on the keyboard, in the place `location`.
    fn press(key: Key, physical: KeyCode, location: KeyLocation) -> KeyEvent {
        KeyEvent {
            physical_key: PhysicalKey::Code(physical),
            location,
            ..KeyEvent::press(key, Modifiers::NONE)
        }
    }

    /// Punctuation pressed with Control keeps its key code and types nothing, so shortcuts
    /// such as Control+/ reach the page.
    #[test]
    fn punctuation_with_control_has_its_code() {
        let event = press(
            Key::Character("/".to_owned()),
            KeyCode::Slash,
            KeyLocation::Standard,
        );
        let key = typed(&event, Held(Held::CONTROL));
        assert_eq!(key.code, 0xBF);
        assert_eq!(key.text, None);
    }

    /// A character of another layout takes the code of the key it is on.
    #[test]
    fn other_layouts_use_the_place_of_the_key() {
        let event = press(
            Key::Character("é".to_owned()),
            KeyCode::Digit2,
            KeyLocation::Standard,
        );
        let key = typed(&event, Held::default());
        assert_eq!(key.code, 0x32);
        assert_eq!(key.text.as_deref(), Some("é"));
    }

    /// The digits of the number pad have codes of their own.
    #[test]
    fn number_pad_digits_have_their_codes() {
        let event = press(
            Key::Character("7".to_owned()),
            KeyCode::Numpad7,
            KeyLocation::Numpad,
        );
        assert_eq!(typed(&event, Held::default()).code, 0x67);
    }

    /// Enter types a carriage return alone, but not with Control or Alt held.
    #[test]
    fn enter_types_only_alone() {
        let event = press(Key::Enter, KeyCode::Enter, KeyLocation::Standard);
        assert_eq!(typed(&event, Held::default()).text.as_deref(), Some("\r"));
        assert_eq!(typed(&event, Held(Held::ALT)).text, None);
        assert_eq!(typed(&event, Held(Held::CONTROL)).text, None);
    }

    /// A media key, which has no name of its own, takes the code of its place.
    #[test]
    fn media_keys_have_their_codes() {
        let event = press(Key::Other, KeyCode::MediaPlayPause, KeyLocation::Standard);
        let key = typed(&event, Held::default());
        assert_eq!(key.code, 0xB3);
        assert_eq!(key.text, None);
    }
}
