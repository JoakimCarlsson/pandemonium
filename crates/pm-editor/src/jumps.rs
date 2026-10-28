//! Jumping to a place in a document.

use pm_protocol::Position;
use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// Puts the caret at `position` and returns the line it is on.
    pub fn jump(&mut self, position: Position) -> usize {
        let offset = self.line_index().offset_of(position, self.field.text());
        self.field.apply(Edit::Place(offset, false));
        self.line_of(offset)
    }
}
