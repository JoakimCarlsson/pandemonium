//! The stretch of the text that finding is limited to, kept in step with the edits made to the text around and inside it.

use std::ops::Range;

use pm_buffer::Change;

use crate::document::Document;

/// Where the byte `offset` of the text before `change` is after it, for an edge of a stretch: an edge the change replaced lands after what replaced it when it `starts` the stretch, and before it when it ends the stretch.
fn carried(offset: usize, change: &Change, starts: bool) -> usize {
    if offset <= change.range.start {
        offset
    } else if offset >= change.range.end {
        offset - change.range.len() + change.text.len()
    } else if starts {
        change.range.start + change.text.len()
    } else {
        change.range.start
    }
}

impl Document {
    /// The stretch finding is limited to, when it is.
    pub fn find_scope(&self) -> Option<Range<usize>> {
        self.find_scope.clone()
    }

    /// Limits finding to `scope`, or to the whole text when nothing; a stretch that holds nothing limits nothing.
    pub fn set_find_scope(&mut self, scope: Option<Range<usize>>) {
        let length = self.field.text().len();
        self.find_scope = scope
            .map(|scope| scope.start.min(length)..scope.end.min(length))
            .filter(|scope| scope.start < scope.end);
    }

    /// The stretch the selection covers as the limit of a search: the selected text, less the line break at its end when it ends at the start of a line, or nothing when no text is selected.
    pub fn selection_scope(&self) -> Option<Range<usize>> {
        let selected = self.field.selection()?;
        let text = self.field.text();
        let end = if selected.end > selected.start && text[..selected.end].ends_with('\n') {
            selected.end - 1
        } else {
            selected.end
        };
        (selected.start < end).then_some(selected.start..end)
    }

    /// Carries the limit of finding along `change`.
    pub(crate) fn carry_find_scope(&mut self, change: &Change) {
        if let Some(scope) = self.find_scope.take() {
            let start = carried(scope.start, change, true);
            let end = carried(scope.end, change, false);
            self.find_scope = (start < end).then_some(start..end);
        }
    }
}
