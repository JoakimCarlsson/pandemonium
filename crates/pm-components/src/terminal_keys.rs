//! What the keys of a terminal send the shell.

use pm_input::{Key, KeyEvent};

/// The modifier number the extended forms of a key carry: one, plus one for shift, two for alt and four for control.
fn modifier_code(event: &KeyEvent) -> u8 {
    1 + u8::from(event.modifiers.shift)
        + 2 * u8::from(event.modifiers.alt)
        + 4 * u8::from(event.modifiers.control)
}

/// The sequence for a key that ends in a letter, such as an arrow, in the plain, the application or the modified form.
fn lettered(event: &KeyEvent, letter: char, application: bool) -> Vec<u8> {
    let code = modifier_code(event);
    if code > 1 {
        format!("\x1b[1;{code}{letter}").into_bytes()
    } else if application {
        format!("\x1bO{letter}").into_bytes()
    } else {
        format!("\x1b[{letter}").into_bytes()
    }
}

/// The sequence for a key that ends in a tilde, such as Delete or Page Up.
fn tilded(event: &KeyEvent, number: u8) -> Vec<u8> {
    let code = modifier_code(event);
    if code > 1 {
        format!("\x1b[{number};{code}~").into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

/// The byte a control chord on `character` sends, if it sends one.
fn control_byte(character: char) -> Option<u8> {
    match character {
        'a'..='z' => Some(character as u8 - b'a' + 1),
        'A'..='Z' => Some(character as u8 - b'A' + 1),
        '@' | ' ' => Some(0),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' | '/' => Some(0x1f),
        _ => None,
    }
}

/// The bytes `event` sends a shell, or nothing for a key the shell does not read, a shortcut of the window, or a key coming up.
///
/// `application_cursor` is whether the program asked for the application form of the arrow keys.
pub fn bytes(event: &KeyEvent, application_cursor: bool) -> Option<Vec<u8>> {
    if !event.pressed || event.modifiers.logo {
        return None;
    }
    let alt = event.modifiers.alt;
    let control = event.modifiers.control;
    let word_motion = alt && !control && !event.modifiers.shift;
    let sent = match event.key {
        Key::Enter => b"\r".to_vec(),
        Key::Escape => b"\x1b".to_vec(),
        Key::Tab if event.modifiers.shift => b"\x1b[Z".to_vec(),
        Key::Tab => b"\t".to_vec(),
        Key::Backspace if control => vec![0x08],
        Key::Backspace => vec![0x7f],
        Key::Delete => tilded(event, 3),
        Key::PageUp => tilded(event, 5),
        Key::PageDown => tilded(event, 6),
        Key::ArrowUp => lettered(event, 'A', application_cursor),
        Key::ArrowDown => lettered(event, 'B', application_cursor),
        Key::ArrowRight if word_motion => b"\x1bf".to_vec(),
        Key::ArrowLeft if word_motion => b"\x1bb".to_vec(),
        Key::ArrowRight => lettered(event, 'C', application_cursor),
        Key::ArrowLeft => lettered(event, 'D', application_cursor),
        Key::Home => lettered(event, 'H', application_cursor),
        Key::End => lettered(event, 'F', application_cursor),
        Key::Space if control => vec![0],
        Key::Space => b" ".to_vec(),
        Key::Character(character) if control => vec![control_byte(character)?],
        Key::Character(character) => character.to_string().into_bytes(),
        Key::Function(number @ 1..=4) => vec![0x1b, b'O', b'P' + number - 1],
        Key::Function(number) => tilded(event, function_code(number)?),
    };
    let alt_prefixed = alt && matches!(event.key, Key::Character(_) | Key::Space | Key::Backspace);
    Some(if alt_prefixed {
        [b"\x1b".as_slice(), &sent].concat()
    } else {
        sent
    })
}

/// The number xterm sends in the escape sequence of the function key F`number`, from F5 up.
fn function_code(number: u8) -> Option<u8> {
    match number {
        5 => Some(15),
        6..=10 => Some(number + 11),
        11 | 12 => Some(number + 12),
        _ => None,
    }
}

/// The bytes that paste `text` with its control characters other than tab and line breaks dropped, so it cannot end the paste early, wrapped in the markers a program that asked for them expects.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text: String = text
        .replace("\r\n", "\r")
        .replace('\n', "\r")
        .chars()
        .filter(|character| *character == '\r' || *character == '\t' || !character.is_control())
        .collect();
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.into_bytes()
    }
}
