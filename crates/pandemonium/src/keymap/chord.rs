//! A chord, and the sequence of chords a binding is pressed as.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use crate::keymap::key::{Key, Modifiers, ParseKeyError};

/// One keypress: a key with the modifiers held down with it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Chord {
    /// The modifiers held down.
    pub modifiers: Modifiers,
    /// The key pressed.
    pub key: Key,
}

impl Chord {
    /// A chord with no modifiers held.
    pub const fn plain(key: Key) -> Self {
        Self {
            modifiers: Modifiers::NONE,
            key,
        }
    }

    /// A chord with `modifiers` held.
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }
}

impl Display for Chord {
    /// Writes the chord the way a keymap spells it, such as `ctrl+shift+p`.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.modifiers, self.key)
    }
}

impl FromStr for Chord {
    type Err = ParseChordError;

    /// Reads a chord written as modifiers and a key joined by `+`.
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let mut modifiers = Modifiers::NONE;
        let mut key = None;

        for token in source.split('+').filter(|token| !token.is_empty()) {
            match Modifiers::from_token(&token.to_ascii_lowercase()) {
                Some(held) if key.is_none() => modifiers = modifiers.union(held),
                Some(_) => return Err(ParseChordError::ModifierAfterKey),
                None if key.is_none() => key = Some(token.parse()?),
                None => return Err(ParseChordError::SecondKey),
            }
        }

        key.map(|key| Self { modifiers, key })
            .ok_or(ParseChordError::NoKey)
    }
}

/// Why a chord could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseChordError {
    /// The chord names modifiers but no key.
    NoKey,
    /// The chord names a second key where a modifier belongs.
    SecondKey,
    /// The chord names a modifier after its key.
    ModifierAfterKey,
    /// One of the chord's tokens names nothing.
    Key(ParseKeyError),
}

impl From<ParseKeyError> for ParseChordError {
    /// Carries an unreadable key token up as a chord error.
    fn from(error: ParseKeyError) -> Self {
        Self::Key(error)
    }
}

impl Display for ParseChordError {
    /// Says why the chord could not be read.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoKey => formatter.write_str("chord has no key"),
            Self::SecondKey => formatter.write_str("chord has more than one key"),
            Self::ModifierAfterKey => formatter.write_str("chord has a modifier after its key"),
            Self::Key(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ParseChordError {}

/// The chords a binding is pressed as, in order, such as `ctrl+k ctrl+c`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Sequence(Vec<Chord>);

impl Sequence {
    /// The sequence of a single chord.
    pub fn single(chord: Chord) -> Self {
        Self(vec![chord])
    }

    /// The chords, in the order they are pressed.
    pub fn chords(&self) -> &[Chord] {
        &self.0
    }

    /// How many chords the sequence is pressed as.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether `pressed` is the sequence so far, complete or not.
    pub fn starts_with(&self, pressed: &[Chord]) -> bool {
        self.0.starts_with(pressed)
    }
}

impl Display for Sequence {
    /// Writes the chords separated by spaces.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for (index, chord) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(" ")?;
            }
            write!(formatter, "{chord}")?;
        }
        Ok(())
    }
}

impl FromStr for Sequence {
    type Err = ParseChordError;

    /// Reads chords separated by whitespace.
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let chords = source
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<Vec<_>, _>>()?;

        if chords.is_empty() {
            return Err(ParseChordError::NoKey);
        }
        Ok(Self(chords))
    }
}
