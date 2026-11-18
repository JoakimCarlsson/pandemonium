//! Words: the runs of one kind of character that a double click selects and the caret jumps over.

use std::ops::Range;

/// What kind of character a word is made of.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Class {
    /// Space.
    Space,
    /// Letters, digits and the underscore.
    Word,
    /// Anything else.
    Other,
}

impl Class {
    /// The class `character` belongs to, the characters of `extra` counting as letters.
    pub(crate) fn of(character: char, extra: &str) -> Self {
        if character.is_whitespace() {
            Self::Space
        } else if character.is_alphanumeric() || character == '_' || extra.contains(character) {
            Self::Word
        } else {
            Self::Other
        }
    }
}

/// The byte range of the word of `text` at byte `byte`: the run of
/// characters of the class of the one starting there, or of the one before
/// it at the end of the text. Empty only when `text` is.
pub fn word_at(text: &str, byte: usize) -> Range<usize> {
    word_at_with(text, byte, "")
}

/// [`word_at`] where the characters of `extra` are part of a word as letters are.
pub fn word_at_with(text: &str, byte: usize, extra: &str) -> Range<usize> {
    let byte = byte.min(text.len());
    let Some(character) = text[byte..]
        .chars()
        .next()
        .or_else(|| text[..byte].chars().next_back())
    else {
        return byte..byte;
    };
    let class = Class::of(character, extra);
    let start = text[..byte]
        .char_indices()
        .rev()
        .take_while(|(_, before)| Class::of(*before, extra) == class)
        .last()
        .map_or(byte, |(at, _)| at);
    let end = text[byte..]
        .char_indices()
        .find(|(_, after)| Class::of(*after, extra) != class)
        .map_or(text.len(), |(at, _)| byte + at);
    start..end.max(start)
}
