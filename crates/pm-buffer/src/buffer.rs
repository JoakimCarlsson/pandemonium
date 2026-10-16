//! A text that is changed piece by piece, numbered by version, as a language server is told of it.

use std::ops::Range;

use crate::change::Change;
use crate::lines::Lines;
use crate::position::{Position, Span};

/// A text, the index of its lines and the version it is at.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Buffer {
    /// The text.
    text: String,
    /// Where each of its lines starts.
    lines: Lines,
    /// How many times it was changed, counted by whoever changes it.
    version: i32,
}

impl Buffer {
    /// A buffer holding `text` at `version`.
    pub fn new(text: impl Into<String>, version: i32) -> Self {
        let text = text.into();
        Self {
            lines: Lines::new(&text),
            text,
            version,
        }
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The version the text is at.
    pub fn version(&self) -> i32 {
        self.version
    }

    /// The index of the lines.
    pub fn lines(&self) -> &Lines {
        &self.lines
    }

    /// Replaces what `span` covers, or the whole text when there is no span, with `text`, and says what bytes that replaced.
    pub fn edit(&mut self, span: Option<Span>, text: &str) -> Change {
        let range = match span {
            Some(span) => self.lines.range_of(span, &self.text),
            None => 0..self.text.len(),
        };
        let change = Change {
            range,
            text: text.to_owned(),
        };
        change.apply(&mut self.text);
        self.lines = Lines::new(&self.text);
        change
    }

    /// Moves the buffer to `version`.
    pub fn set_version(&mut self, version: i32) {
        self.version = version;
    }

    /// The byte `offset` as a line and a UTF-16 column.
    pub fn position_of(&self, offset: usize) -> Position {
        self.lines.position_of(offset, &self.text)
    }

    /// The byte offset `position` names.
    pub fn offset_of(&self, position: Position) -> usize {
        self.lines.offset_of(position, &self.text)
    }

    /// The bytes `range` as a span of places.
    pub fn span_of(&self, range: Range<usize>) -> Span {
        self.lines.span_of(range, &self.text)
    }
}
