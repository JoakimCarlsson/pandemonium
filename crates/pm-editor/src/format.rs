//! Formatting and edits across a project: the changes a language server gives, made to the text as one change, the caret kept where it was among them.

use pm_protocol::TextChange;
use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// The edits that make `changes`, which a language server gave to format the text as it was at `version`: the last stretch first so the others stay where they are, then the caret put back where it was among them; none when the text moved on since.
    pub fn format_edits(&self, version: i32, changes: Vec<TextChange>) -> Vec<Edit> {
        if self.version != version {
            return Vec::new();
        }
        self.change_edits(changes)
    }

    /// The edit that makes `changes`, each a stretch of the text as it is now and what replaces it, as one change that undoes at once: the caret is kept where it was among them.
    pub fn change_edits(&self, changes: Vec<TextChange>) -> Vec<Edit> {
        if changes.is_empty() {
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
        let mut text = self.field.text().to_owned();
        let mut floor = text.len();
        for (range, replacement) in ranges {
            if range.end > floor {
                continue;
            }
            floor = range.start;
            text.replace_range(range, &replacement);
        }
        let caret = (caret as isize + moved).clamp(0, text.len() as isize) as usize;
        let whole = self.field.text().len();
        vec![Edit::Replace(0..whole, text, (caret, caret))]
    }
}
