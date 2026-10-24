//! Jumping to a place in a document, and the places jumped from, to go back to and forward again.

use pm_protocol::Position;
use pm_text::Edit;

use crate::document::Document;

/// A place the caret was in a file of a session.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    /// The session, by position.
    pub session: usize,
    /// The file.
    pub path: String,
    /// The caret.
    pub position: Position,
}

/// Where the caret was before each jump, and where it was before each step back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Jumps {
    /// The places to go back to, the latest last.
    pub back: Vec<Spot>,
    /// The places to go forward to, the latest last.
    pub forward: Vec<Spot>,
}

/// How many places each way are remembered.
const REMEMBERED: usize = 100;

impl Jumps {
    /// Remembers `spot` as where the caret was before a jump, and forgets the places gone back from.
    pub fn leave(&mut self, spot: Spot) {
        self.back.push(spot);
        if self.back.len() > REMEMBERED {
            self.back.remove(0);
        }
        self.forward.clear();
    }
}

impl Document {
    /// Puts the caret at `position` and returns the line it is on.
    pub fn jump(&mut self, position: Position) -> usize {
        let offset = self.line_index().offset_of(position, self.field.text());
        self.field.apply(Edit::Place(offset, false));
        self.line_of(offset)
    }
}
