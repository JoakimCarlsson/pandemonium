//! Where a line too long for its pane carries on down the next row.
//!
//! A box a reader writes a prompt or a message in has nowhere to scroll
//! sideways to: a sentence longer than the box goes on under itself, broken
//! after the last space that fits, or inside a word when the word alone is
//! wider than the box. A row is what such a pane scrolls, draws and is
//! pointed at by; a line of a document that does not wrap is always exactly
//! one of them, so a file counts rows the way it always counted lines.

use pm_text::{Buffer, Position};

/// One row of a pane: which line it belongs to, and which of that line's
/// rows it is.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Row {
    /// The line the row is part of.
    pub line: usize,
    /// How many rows of that line come before it.
    pub part: usize,
}

impl Row {
    /// The first row of `line`.
    pub const fn first_of(line: usize) -> Self {
        Self { line, part: 0 }
    }
}

/// The characters of one line that one row holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Segment {
    /// The line the characters are on.
    pub line: usize,
    /// The first character the row holds.
    pub start: usize,
    /// The character the line's next row begins at, or [`usize::MAX`] on its
    /// last row, which holds everything to the end of the line and past it.
    pub end: usize,
}

impl Segment {
    /// The one row of a line that does not wrap.
    pub const fn whole(line: usize) -> Self {
        Self {
            line,
            start: 0,
            end: usize::MAX,
        }
    }

    /// Whether this is the last row of its line.
    pub const fn is_last(&self) -> bool {
        self.end == usize::MAX
    }

    /// Whether `position` is drawn on this row.
    pub const fn holds(&self, position: Position) -> bool {
        position.line == self.line && position.column >= self.start && position.column < self.end
    }

    /// The column of the line this row begins at, which is drawn at the
    /// pane's left edge.
    pub fn indent(&self, buffer: &Buffer) -> usize {
        match self.start {
            0 => 0,
            start => buffer.display_column(Position::new(self.line, start)),
        }
    }
}

/// The rows `line` of `buffer` comes to in a pane `columns` wide.
///
/// Space is never what pushes a row over: it hangs off the end of the row
/// it follows, so the next row begins with the next word rather than with
/// the gap before it.
pub fn segments(buffer: &Buffer, line: usize, columns: usize) -> Vec<Segment> {
    let starts = breaks(buffer, line, columns.max(1));
    let ends = starts.iter().skip(1).copied().chain([usize::MAX]);
    starts
        .iter()
        .zip(ends)
        .map(|(start, end)| Segment {
            line,
            start: *start,
            end,
        })
        .collect()
}

/// The characters each row of `line` begins at, `columns` being how many
/// fit across.
fn breaks(buffer: &Buffer, line: usize, columns: usize) -> Vec<usize> {
    let tab = buffer.tab_width();
    let mut starts = vec![0];
    let mut drawn = 0;
    let mut row_left = 0;
    let mut opening = None;

    for (index, ch) in buffer.line_chars(line).enumerate() {
        let width = match ch {
            '\t' => tab - drawn % tab,
            _ => 1,
        };
        let start = starts.last().copied().unwrap_or_default();
        if !ch.is_whitespace() && drawn + width - row_left > columns && index > start {
            let (at, left) = opening
                .filter(|(at, _)| *at > start)
                .unwrap_or((index, drawn));
            starts.push(at);
            row_left = left;
            opening = None;
        }
        drawn += width;
        if ch.is_whitespace() {
            opening = Some((index + 1, drawn));
        }
    }
    starts
}
