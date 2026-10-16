//! Where each line of a text starts, and the conversions between byte offsets and places that the index makes quick.

use std::ops::Range;

use crate::position::{Position, Span};

/// The byte offset at which each line of a text starts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Lines {
    /// Where each line starts, the first always at nought.
    starts: Vec<usize>,
}

impl Lines {
    /// The index of the lines of `text`.
    pub fn new(text: &str) -> Self {
        Self {
            starts: std::iter::once(0)
                .chain(text.match_indices('\n').map(|(at, _)| at + 1))
                .collect(),
        }
    }

    /// How many lines there are; an empty text has one.
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// The byte offset line `index` starts at, or the end of `text` past the last line.
    pub fn start(&self, index: usize, text: &str) -> usize {
        self.starts.get(index).copied().unwrap_or(text.len())
    }

    /// Where line `index` is in `text`, without its line break.
    pub fn line(&self, index: usize, text: &str) -> Range<usize> {
        let start = self.start(index, text);
        let end = self
            .starts
            .get(index + 1)
            .map_or(text.len(), |next| next - 1);
        start..end.max(start)
    }

    /// The line the byte `offset` is on.
    pub fn line_of(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    /// The byte `offset` of `text` as a row and a byte column, the way a syntax tree counts.
    pub fn point_of(&self, offset: usize) -> (usize, usize) {
        let row = self.line_of(offset);
        (row, offset - self.starts[row])
    }

    /// The byte `offset` of `text` as a line and a column in UTF-16 code units.
    pub fn position_of(&self, offset: usize, text: &str) -> Position {
        let offset = floor_boundary(text, offset);
        let row = self.line_of(offset);
        let start = self.starts[row];
        let character = text[start..offset].encode_utf16().count();
        Position::new(row as u32, character as u32)
    }

    /// The byte offset of `text` that `position` names, kept within its line and the text.
    pub fn offset_of(&self, position: Position, text: &str) -> usize {
        let line = self.line(position.line as usize, text);
        let mut units = 0;
        for (at, character) in text[line.clone()].char_indices() {
            if units >= position.character as usize {
                return line.start + at;
            }
            units += character.len_utf16();
        }
        line.end
    }

    /// The byte range of `text` that `span` covers.
    pub fn range_of(&self, span: Span, text: &str) -> Range<usize> {
        let start = self.offset_of(span.start, text);
        start..self.offset_of(span.end, text).max(start)
    }

    /// The bytes `range` of `text` as a span of places.
    pub fn span_of(&self, range: Range<usize>, text: &str) -> Span {
        Span::new(
            self.position_of(range.start, text),
            self.position_of(range.end, text),
        )
    }
}

/// The nearest character boundary of `text` at or before `offset`.
fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
