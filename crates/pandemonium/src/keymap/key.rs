//! The keys and modifiers a chord is built from.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// A key that is not a character: a named key or a function key.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Named {
    /// The escape key.
    Escape,
    /// The return or enter key.
    Enter,
    /// The tab key.
    Tab,
    /// The space bar.
    Space,
    /// The backspace key.
    Backspace,
    /// The forward delete key.
    Delete,
    /// The insert key.
    Insert,
    /// The home key.
    Home,
    /// The end key.
    End,
    /// The page up key.
    PageUp,
    /// The page down key.
    PageDown,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
}

impl Named {
    /// Every named key, in the order they are written above.
    pub const ALL: [Self; 15] = [
        Self::Escape,
        Self::Enter,
        Self::Tab,
        Self::Space,
        Self::Backspace,
        Self::Delete,
        Self::Insert,
        Self::Home,
        Self::End,
        Self::PageUp,
        Self::PageDown,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
    ];

    /// The token the key is written as in a keymap.
    pub fn token(self) -> &'static str {
        match self {
            Self::Escape => "escape",
            Self::Enter => "enter",
            Self::Tab => "tab",
            Self::Space => "space",
            Self::Backspace => "backspace",
            Self::Delete => "delete",
            Self::Insert => "insert",
            Self::Home => "home",
            Self::End => "end",
            Self::PageUp => "pageup",
            Self::PageDown => "pagedown",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// The key half of a chord, with the modifiers held out of it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    /// A character key, always lowercase: shift lives in the modifiers.
    Character(char),
    /// A named key.
    Named(Named),
    /// A function key, numbered from one.
    Function(u8),
}

impl Display for Key {
    /// Writes the key the way a keymap spells it.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Character(character) => write!(formatter, "{character}"),
            Self::Named(named) => formatter.write_str(named.token()),
            Self::Function(number) => write!(formatter, "f{number}"),
        }
    }
}

impl FromStr for Key {
    type Err = ParseKeyError;

    /// Reads a key token, as written in a keymap.
    fn from_str(token: &str) -> Result<Self, Self::Err> {
        let lowered = token.to_ascii_lowercase();

        if let Some(named) = Named::ALL
            .into_iter()
            .find(|named| named.token() == lowered)
        {
            return Ok(Self::Named(named));
        }

        if let Some(number) = function_number(&lowered) {
            return Ok(Self::Function(number));
        }

        let mut characters = lowered.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => Ok(Self::Character(character)),
            _ => Err(ParseKeyError {
                token: token.to_owned(),
            }),
        }
    }
}

/// The number of a function-key token, if it is one.
fn function_number(token: &str) -> Option<u8> {
    let digits = token.strip_prefix('f')?;
    let number = digits.parse().ok()?;
    (1..=24).contains(&number).then_some(number)
}

/// A key token that names nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseKeyError {
    /// The token that could not be read.
    pub token: String,
}

impl Display for ParseKeyError {
    /// Names the token that could not be read.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown key `{}`", self.token)
    }
}

impl std::error::Error for ParseKeyError {}

/// The modifier keys held down with a key.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Modifiers {
    /// Whether control is held.
    pub control: bool,
    /// Whether shift is held.
    pub shift: bool,
    /// Whether alt, or option, is held.
    pub alt: bool,
    /// Whether command, super or the windows key is held.
    pub command: bool,
}

impl Modifiers {
    /// No modifiers at all.
    pub const NONE: Self = Self {
        control: false,
        shift: false,
        alt: false,
        command: false,
    };

    /// The modifier a keymap means by `primary`: command on macOS, control elsewhere.
    pub const fn primary() -> Self {
        let mut modifiers = Self::NONE;
        if cfg!(target_os = "macos") {
            modifiers.command = true;
        } else {
            modifiers.control = true;
        }
        modifiers
    }

    /// The two sets of modifiers held together.
    pub const fn union(self, other: Self) -> Self {
        Self {
            control: self.control || other.control,
            shift: self.shift || other.shift,
            alt: self.alt || other.alt,
            command: self.command || other.command,
        }
    }

    /// Whether no modifier at all is held.
    pub const fn is_empty(self) -> bool {
        !(self.control || self.shift || self.alt || self.command)
    }

    /// The modifier a token names, or nothing if it names a key instead.
    pub(super) fn from_token(token: &str) -> Option<Self> {
        let mut modifiers = Self::NONE;
        match token {
            "ctrl" | "control" => modifiers.control = true,
            "shift" => modifiers.shift = true,
            "alt" | "option" => modifiers.alt = true,
            "cmd" | "command" | "super" | "meta" | "win" => modifiers.command = true,
            "primary" => return Some(Self::primary()),
            _ => return None,
        }
        Some(modifiers)
    }
}

impl Display for Modifiers {
    /// Writes each held modifier, in the order a keymap spells them.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for (held, token) in [
            (self.control, "ctrl"),
            (self.shift, "shift"),
            (self.alt, "alt"),
            (self.command, "cmd"),
        ] {
            if held {
                write!(formatter, "{token}+")?;
            }
        }
        Ok(())
    }
}
