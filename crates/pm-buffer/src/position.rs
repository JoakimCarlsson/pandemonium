//! Places in a text as a language server names them.

use serde::{Deserialize, Serialize};

/// A place in a text: the line, counted from nought, and how many UTF-16 code units into the line it is.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct Position {
    /// The line, counted from nought.
    pub line: u32,
    /// How far into the line, in UTF-16 code units.
    pub character: u32,
}

impl Position {
    /// The place `character` code units into line `line`.
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A stretch of a text between two places, the end left out.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Span {
    /// Where it starts.
    pub start: Position,
    /// Where it ends.
    pub end: Position,
}

impl Span {
    /// The stretch from `start` to `end`.
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}
