//! What a keypress sends to the child: the sequences a terminal has always sent.
//!
//! The window knows what key was pressed; only the terminal knows what bytes
//! that means, because the answer depends on modes the program has set. The
//! key itself arrives as [`Key`], which is the platform's event with the
//! platform taken out of it.

use crate::modes::Modes;

/// Which modifiers were held with a key.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    /// Control.
    pub control: bool,
    /// Alt, which a terminal sends as a leading escape.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
}

impl Modifiers {
    /// Whether no modifier of consequence was held.
    fn is_empty(self) -> bool {
        !self.control && !self.alt && !self.shift
    }

    /// The modifier parameter a sequence carries, which counts from one.
    ///
    /// One means no modifier, and each modifier adds its own bit: shift one,
    /// alt two, control four. `CSI 1 ; 6 A` is therefore control-shift-up.
    fn parameter(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.control)
    }
}

/// One key, as the terminal cares about it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    /// A key that produced a character.
    Char(char),
    /// Return.
    Enter,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Insert.
    Insert,
    /// Delete.
    Delete,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Cursor up.
    Up,
    /// Cursor down.
    Down,
    /// Cursor left.
    Left,
    /// Cursor right.
    Right,
    /// A function key, numbered from one.
    Function(u8),
}

/// The bytes `key` sends, given the modifiers held and the modes in force.
pub fn encode(key: Key, modifiers: Modifiers, modes: Modes) -> Option<Vec<u8>> {
    let bytes = match key {
        Key::Char(c) => character(c, modifiers)?,
        Key::Enter => escaped(b"\r".to_vec(), modifiers),
        Key::Tab if modifiers.shift => b"\x1b[Z".to_vec(),
        Key::Tab => escaped(b"\t".to_vec(), modifiers),
        Key::Backspace if modifiers.control => escaped(b"\x08".to_vec(), modifiers),
        Key::Backspace => escaped(b"\x7f".to_vec(), modifiers),
        Key::Escape => b"\x1b".to_vec(),
        Key::Up => cursor(b'A', modifiers, modes),
        Key::Down => cursor(b'B', modifiers, modes),
        Key::Right => cursor(b'C', modifiers, modes),
        Key::Left => cursor(b'D', modifiers, modes),
        Key::Home => cursor(b'H', modifiers, modes),
        Key::End => cursor(b'F', modifiers, modes),
        Key::Insert => tilde(2, modifiers),
        Key::Delete => tilde(3, modifiers),
        Key::PageUp => tilde(5, modifiers),
        Key::PageDown => tilde(6, modifiers),
        Key::Function(number) => function(number, modifiers)?,
    };
    Some(bytes)
}

/// The bytes `text` is pasted as, fenced when the program asked for that.
pub fn paste(text: &str, modes: Modes) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if modes.bracketed_paste {
        let mut bytes = b"\x1b[200~".to_vec();
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        bytes
    } else {
        text.into_bytes()
    }
}

/// The bytes a character key sends, control and alt included.
fn character(c: char, modifiers: Modifiers) -> Option<Vec<u8>> {
    let bytes = match (modifiers.control, c) {
        (true, ' ') | (true, '@') => vec![0x00],
        (true, 'a'..='z') => vec![c as u8 - b'a' + 1],
        (true, 'A'..='Z') => vec![c as u8 - b'A' + 1],
        (true, '[') => vec![0x1b],
        (true, '\\') => vec![0x1c],
        (true, ']') => vec![0x1d],
        (true, '^') => vec![0x1e],
        (true, '_') => vec![0x1f],
        (true, '?') => vec![0x7f],
        (true, _) => return None,
        (false, _) => c.to_string().into_bytes(),
    };
    Some(escaped(bytes, modifiers))
}

/// The bytes a cursor or home key sends, in either cursor-key mode.
fn cursor(final_byte: u8, modifiers: Modifiers, modes: Modes) -> Vec<u8> {
    if modifiers.is_empty() {
        let introducer: &[u8] = if modes.application_cursor {
            b"\x1bO"
        } else {
            b"\x1b["
        };
        return [introducer, &[final_byte]].concat();
    }
    format!("\x1b[1;{}{}", modifiers.parameter(), final_byte as char).into_bytes()
}

/// The bytes one of the `CSI n ~` keys sends.
fn tilde(number: u8, modifiers: Modifiers) -> Vec<u8> {
    if modifiers.is_empty() {
        return format!("\x1b[{number}~").into_bytes();
    }
    format!("\x1b[{number};{}~", modifiers.parameter()).into_bytes()
}

/// The bytes function key `number` sends.
fn function(number: u8, modifiers: Modifiers) -> Option<Vec<u8>> {
    let bytes = match number {
        1..=4 if modifiers.is_empty() => {
            format!("\x1bO{}", (b'P' + number - 1) as char).into_bytes()
        }
        1..=4 => format!(
            "\x1b[1;{}{}",
            modifiers.parameter(),
            (b'P' + number - 1) as char
        )
        .into_bytes(),
        5..=12 => tilde(TILDE_FUNCTIONS[number as usize - 5], modifiers),
        _ => return None,
    };
    Some(bytes)
}

/// Prefixes `bytes` with an escape when alt was held, as a terminal does.
fn escaped(bytes: Vec<u8>, modifiers: Modifiers) -> Vec<u8> {
    if !modifiers.alt {
        return bytes;
    }
    let mut escaped = vec![0x1b];
    escaped.extend_from_slice(&bytes);
    escaped
}

/// The `CSI n ~` numbers of F5 through F12, which skip two along the way.
const TILDE_FUNCTIONS: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
