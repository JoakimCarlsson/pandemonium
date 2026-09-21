//! What a language server has to say about a span of a buffer.
//!
//! A diagnostic is kept in the editor's own terms rather than the protocol's,
//! so that the screen drawing a squiggle under a line, and the bar counting
//! how many there are, never see a wire type.

use std::ops::Range;

use crate::cursor::Position;

/// How much a diagnostic matters.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    /// Something is wrong and the code will not build.
    Error,
    /// Something is suspect but the code will build.
    Warning,
    /// Something worth knowing about the code.
    Information,
    /// Something the server offers in passing.
    Hint,
}

/// One thing a language server says about one span of a buffer.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// The span it is about.
    pub range: Range<Position>,
    /// How much it matters.
    pub severity: Severity,
    /// What it says.
    pub message: String,
    /// Which tool said it, when the server names one.
    pub source: Option<String>,
}

impl Diagnostic {
    /// Whether this diagnostic covers any of `line`.
    pub fn touches(&self, line: usize) -> bool {
        (self.range.start.line..=self.range.end.line).contains(&line)
    }

    /// The columns of `line` this diagnostic covers, if it covers any.
    ///
    /// A span that ends where it starts is widened to one character: a
    /// server is entitled to point at a place rather than at a run of text,
    /// and a squiggle under nothing cannot be seen.
    pub fn columns(&self, line: usize, line_len: usize) -> Option<Range<usize>> {
        if !self.touches(line) {
            return None;
        }
        let start = if self.range.start.line == line {
            self.range.start.column
        } else {
            0
        };
        let end = if self.range.end.line == line {
            self.range.end.column.min(line_len)
        } else {
            line_len
        };
        Some(start..end.max(start + 1))
    }
}
