//! A keypress, as modal editing reads one, and the notation bindings write
//! one in.
//!
//! The window turns whatever its windowing library delivers into these, so
//! nothing here knows where a key came from; what matters is the character
//! it types, or which of the few named keys it is, and which modifiers were
//! held. Bindings are written the way Zed's vim keymap writes them —
//! `ctrl-w`, `shift-g`, `g g`, `space` — so a table can be read either way.

/// Which key was pressed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    /// A key that types a character, Shift already applied.
    Char(char),
    /// The Escape key.
    Escape,
    /// The Enter key.
    Enter,
    /// The Backspace key.
    Backspace,
    /// The Delete key.
    Delete,
    /// The Insert key.
    Insert,
    /// The Tab key.
    Tab,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// The Home key.
    Home,
    /// The End key.
    End,
    /// The Page Up key.
    PageUp,
    /// The Page Down key.
    PageDown,
}

/// One keypress: the key, and whether Ctrl or Shift was held with it.
///
/// Shift is only kept for the named keys; a character already carries it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Keystroke {
    /// The key itself.
    pub key: Key,
    /// Whether Ctrl was held.
    pub ctrl: bool,
    /// Whether Shift was held with a named key.
    pub shift: bool,
}

/// The names the notation gives the named keys, and the key each stands for.
const NAMES: [(&str, Key); 15] = [
    ("escape", Key::Escape),
    ("enter", Key::Enter),
    ("backspace", Key::Backspace),
    ("delete", Key::Delete),
    ("insert", Key::Insert),
    ("tab", Key::Tab),
    ("left", Key::Left),
    ("right", Key::Right),
    ("up", Key::Up),
    ("down", Key::Down),
    ("home", Key::Home),
    ("end", Key::End),
    ("pageup", Key::PageUp),
    ("pagedown", Key::PageDown),
    ("space", Key::Char(' ')),
];

impl Keystroke {
    /// `key` pressed on its own.
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
        }
    }

    /// The character typed, when this is a character pressed on its own.
    pub fn char(self) -> Option<char> {
        match self {
            Self {
                key: Key::Char(ch),
                ctrl: false,
                ..
            } => Some(ch),
            _ => None,
        }
    }

    /// The character pressed with Ctrl, when this is one.
    pub fn ctrl_char(self) -> Option<char> {
        match self {
            Self {
                key: Key::Char(ch),
                ctrl: true,
                ..
            } => Some(ch.to_ascii_lowercase()),
            _ => None,
        }
    }

    /// Whether this key leaves whatever is being typed, as Escape does.
    pub fn is_escape(self) -> bool {
        self.key == Key::Escape || self.ctrl_char() == Some('[')
    }

    /// Reads one keystroke of the notation: `a`, `G`, `shift-g`, `ctrl-w`,
    /// `enter`, `space`.
    pub fn parse(written: &str) -> Option<Self> {
        let mut ctrl = false;
        let mut shift = false;
        let mut rest = written;
        loop {
            if let Some(after) = rest.strip_prefix("ctrl-").filter(|after| !after.is_empty()) {
                ctrl = true;
                rest = after;
            } else if let Some(after) = rest
                .strip_prefix("shift-")
                .filter(|after| !after.is_empty())
            {
                shift = true;
                rest = after;
            } else {
                break;
            }
        }
        let key = match NAMES.iter().find(|(name, _)| *name == rest) {
            Some((_, key)) => *key,
            None => {
                let mut chars = rest.chars();
                let (Some(ch), None) = (chars.next(), chars.next()) else {
                    return None;
                };
                Key::Char(ch)
            }
        };
        Some(match (key, shift) {
            (Key::Char(ch), true) => Self {
                key: Key::Char(ch.to_ascii_uppercase()),
                ctrl,
                shift: false,
            },
            (key, shift) => Self { key, ctrl, shift },
        })
    }

    /// Reads a sequence of keystrokes written apart by spaces: `g g`,
    /// `ctrl-w h`.
    pub fn parse_sequence(written: &str) -> Option<Vec<Self>> {
        written.split_whitespace().map(Self::parse).collect()
    }

    /// How the key reads in the pending-keys readout of the status bar.
    pub fn label(self) -> String {
        let name = match self.key {
            Key::Char(' ') => "␣".to_owned(),
            Key::Char(ch) => ch.to_string(),
            Key::Escape => "esc".to_owned(),
            Key::Enter => "⏎".to_owned(),
            Key::Backspace => "⌫".to_owned(),
            Key::Delete => "del".to_owned(),
            Key::Insert => "ins".to_owned(),
            Key::Tab => "⇥".to_owned(),
            Key::Left => "←".to_owned(),
            Key::Right => "→".to_owned(),
            Key::Up => "↑".to_owned(),
            Key::Down => "↓".to_owned(),
            Key::Home => "home".to_owned(),
            Key::End => "end".to_owned(),
            Key::PageUp => "pgup".to_owned(),
            Key::PageDown => "pgdn".to_owned(),
        };
        match (self.ctrl, self.shift) {
            (true, _) => format!("^{name}"),
            (false, true) => format!("⇧{name}"),
            (false, false) => name,
        }
    }
}
