//! Formatting: the edits a language server gives, made to the text in an order that keeps them true, the caret kept where it was among them.

use pm_protocol::TextChange;
use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// The edits that make `changes`, which a language server gave to format the text as it was at `version`: the last stretch first so the others stay where they are, then the caret put back where it was among them; none when the text moved on since.
    pub fn format_edits(&self, version: i32, changes: Vec<TextChange>) -> Vec<Edit> {
        if self.version != version || changes.is_empty() {
            return Vec::new();
        }
        let caret = self.field.caret();
        let mut ranges: Vec<(std::ops::Range<usize>, String)> = changes
            .into_iter()
            .filter_map(|change| Some((self.range_of(change.span?), change.text)))
            .collect();
        ranges.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let moved: isize = ranges
            .iter()
            .filter(|(range, _)| range.end <= caret)
            .map(|(range, text)| text.len() as isize - range.len() as isize)
            .sum();
        let length: isize = self.field.text().len() as isize
            + ranges
                .iter()
                .map(|(range, text)| text.len() as isize - range.len() as isize)
                .sum::<isize>();
        let mut edits: Vec<Edit> = ranges
            .into_iter()
            .flat_map(|(range, text)| {
                [
                    Edit::Place(range.start, false),
                    Edit::Place(range.end, true),
                    Edit::Paste(text),
                ]
            })
            .collect();
        edits.push(Edit::Place(
            (caret as isize + moved).clamp(0, length.max(0)) as usize,
            false,
        ));
        edits
    }
}
