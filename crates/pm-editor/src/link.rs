//! Editing stretches together: while the caret is in one of several stretches the language server says belong together, such as the two names of a tag, what is typed in it is typed in the others too.

use std::ops::Range;

use pm_buffer::Change;
use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// Whether the caret is in a stretch that is edited together with others.
    pub fn is_linked(&self) -> bool {
        self.linked.is_some()
    }

    /// The edit that makes `edit` and the same change in every stretch edited together with the one it changed, as one change; `edit` itself when no such stretches are kept or it changed something else, in which case they are let go.
    pub(crate) fn mirrored(&mut self, edit: Edit) -> Edit {
        let Some(ranges) = self.linked.clone() else {
            return edit;
        };
        let before = self.field.text();
        let mut probe = self.field.clone();
        probe.apply(edit.clone());
        let Some(change) = Change::between(before, probe.text()) else {
            let caret = probe.caret();
            if !ranges
                .iter()
                .any(|range| range.start <= caret && caret <= range.end)
            {
                self.linked = None;
            }
            return edit;
        };
        match self.mirror(&ranges, &change, probe.caret()) {
            Some(mirrored) => mirrored,
            None => {
                self.linked = None;
                edit
            }
        }
    }

    /// The edit that makes `change`, which fell inside one of `ranges`, in each of them at the same place, the caret put at `caret` of the text with only `change` made, moved by what the stretches before it grew or shrank; the stretches kept move along. Nothing when `change` fell outside them all or does not fit in one of the others.
    fn mirror(&mut self, ranges: &[Range<usize>], change: &Change, caret: usize) -> Option<Edit> {
        let home = ranges
            .iter()
            .position(|range| range.start <= change.range.start && change.range.end <= range.end)?;
        let at = change.range.start - ranges[home].start;
        let length = change.range.len();
        let fits = ranges
            .iter()
            .all(|range| range.start + at + length <= range.end);
        if !fits {
            return None;
        }
        let before = self.field.text();
        let mut text = before.to_owned();
        for range in ranges.iter().rev() {
            let from = range.start + at;
            if !before.is_char_boundary(from) || !before.is_char_boundary(from + length) {
                return None;
            }
            text.replace_range(from..from + length, &change.text);
        }
        let growth = change.text.len() as isize - length as isize;
        let earlier = ranges[..home].len() as isize;
        let caret = (caret as isize + growth * earlier).clamp(0, text.len() as isize) as usize;
        self.linked = Some(
            ranges
                .iter()
                .enumerate()
                .map(|(place, range)| {
                    let start = (range.start as isize + growth * place as isize) as usize;
                    let end = (range.end as isize + growth * (place as isize + 1)) as usize;
                    start..end
                })
                .collect(),
        );
        Some(Edit::Replace(0..before.len(), text, (caret, caret)))
    }
}
