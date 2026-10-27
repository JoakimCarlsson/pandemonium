//! More than one caret: the next place the selected text comes again selected as well, every place it comes, and a caret added on the line above or below.

use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// The word around the caret, as a byte range, when the caret touches one.
    fn word_around_caret(&self) -> Option<std::ops::Range<usize>> {
        let text = self.field.text();
        let caret = self.field.caret();
        let start = self.word_start(caret);
        let end = text[caret..]
            .char_indices()
            .find(|(_, character)| !self.is_word_character(*character))
            .map_or(text.len(), |(at, _)| caret + at);
        (start < end).then_some(start..end)
    }

    /// Whether the bytes `range` are a whole word: word characters, with none just before or after them.
    fn whole_word(&self, range: std::ops::Range<usize>) -> bool {
        let text = self.field.text();
        let inside = !range.is_empty()
            && text[range.clone()]
                .chars()
                .all(|held| self.is_word_character(held));
        let before = text[..range.start]
            .chars()
            .next_back()
            .is_some_and(|held| self.is_word_character(held));
        let after = text[range.end..]
            .chars()
            .next()
            .is_some_and(|held| self.is_word_character(held));
        inside && !before && !after
    }

    /// The edit that selects the next place the selected text comes again as well, after the last selection and going round the end of the text; with nothing selected, the word at the caret is selected first.
    pub fn add_next_occurrence_edit(&self) -> Option<Edit> {
        let Some(selected) = self.field.selection() else {
            let word = self.word_around_caret()?;
            return Some(Edit::Select(word.start, word.end));
        };
        let text = self.field.text();
        let whole = self.whole_word(selected.clone());
        let wanted = &text[selected];
        let taken = self.field.selections();
        let after = taken
            .iter()
            .map(|(anchor, caret)| (*anchor).max(*caret))
            .max()
            .unwrap_or(0);
        let is_taken = |start: usize| {
            taken
                .iter()
                .any(|(anchor, caret)| (*anchor).min(*caret) == start)
        };
        let found = text[after..]
            .match_indices(wanted)
            .map(|(at, _)| after + at)
            .chain(text[..after].match_indices(wanted).map(|(at, _)| at))
            .filter(|start| !whole || self.whole_word(*start..start + wanted.len()))
            .find(|start| !is_taken(*start))?;
        Some(Edit::AddSelection(found, found + wanted.len()))
    }

    /// The edits that select every place the selected text, or the word at the caret, comes.
    pub fn select_all_occurrences_edits(&self) -> Vec<Edit> {
        let range = match self.field.selection() {
            Some(range) => range,
            None => match self.word_around_caret() {
                Some(word) => word,
                None => return Vec::new(),
            },
        };
        let text = self.field.text();
        let whole = self.whole_word(range.clone());
        let wanted = &text[range.clone()];
        let mut edits = vec![Edit::Select(range.start, range.end)];
        edits.extend(
            text.match_indices(wanted)
                .map(|(at, _)| at)
                .filter(|at| *at != range.start)
                .filter(|at| !whole || self.whole_word(*at..at + wanted.len()))
                .map(|at| Edit::AddSelection(at, at + wanted.len())),
        );
        edits
    }

    /// The edit that adds a caret on the line below the lowest caret, or above the highest one unless `down`, as many characters in.
    pub fn add_caret_line_edit(&self, down: bool) -> Option<Edit> {
        let text = self.field.text();
        let taken = self.field.selections();
        let (_, caret) = if down {
            *taken.last()?
        } else {
            *taken.first()?
        };
        let start = text[..caret].rfind('\n').map_or(0, |at| at + 1);
        let column = text[start..caret].chars().count();
        let target = if down {
            start + text[start..].find('\n')? + 1
        } else {
            if start == 0 {
                return None;
            }
            text[..start - 1].rfind('\n').map_or(0, |at| at + 1)
        };
        let end = text[target..]
            .find('\n')
            .map_or(text.len(), |at| target + at);
        let at = text[target..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(at, _)| target + at);
        Some(Edit::AddSelection(at, at))
    }
}
