//! A keypress, as modal editing reads one.
//!
//! The window turns whatever its windowing library delivers into these, so
//! nothing here knows where a key came from; what matters is the character
//! it types, or which of the few named keys it is, and whether Ctrl was held.

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

/// One keypress: the key, and whether Ctrl was held with it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Keystroke {
    /// The key itself.
    pub key: Key,
    /// Whether Ctrl was held.
    pub ctrl: bool,
}

impl Keystroke {
    /// `key` pressed on its own.
    pub const fn plain(key: Key) -> Self {
        Self { key, ctrl: false }
    }

    /// The character typed, when this is a character pressed on its own.
    pub fn char(self) -> Option<char> {
        match self {
            Self {
                key: Key::Char(ch),
                ctrl: false,
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
            } => Some(ch.to_ascii_lowercase()),
            _ => None,
        }
    }

    /// Whether this key leaves whatever is being typed, as Escape does.
    pub fn is_escape(self) -> bool {
        self.key == Key::Escape || self.ctrl_char() == Some('[')
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
        match self.ctrl {
            true => format!("^{name}"),
            false => name,
        }
    }
}
