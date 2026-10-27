//! The completions open at the caret: narrowed to the word typed, and the edits that write the one picked.

use pm_protocol::Completion;
use pm_text::Edit;

use crate::document::Document;

/// The positions in `items` of the completions that match `typed`, best first.
fn matching(items: &[Completion], typed: &str) -> Vec<usize> {
    let mut scored: Vec<(u8, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(place, item)| pm_lsp::score(&item.filter, typed).map(|held| (held, place)))
        .collect();
    scored.sort();
    scored.into_iter().map(|(_, place)| place).collect()
}

impl Document {
    /// Narrows the completions open to the word typed before the caret, and closes the list when the caret left the word or nothing matches; says whether a fresh list should be asked for in place of none, as when the reader began a word, unless a trigger character asked already, `triggered`.
    pub(crate) fn refilter(&mut self, triggered: bool) -> bool {
        let caret = self.field.caret();
        let extra = self.editing.word_characters.clone();
        let is_word = |held: char| held.is_alphanumeric() || held == '_' || extra.contains(held);
        let text = self.field.text();
        let Some(completion) = &mut self.completion else {
            let typed_word = text[..caret].chars().next_back().is_some_and(is_word);
            return !triggered && typed_word && caret - self.word_start(caret) == 1 && self.synced;
        };
        let in_word = caret >= completion.start
            && text
                .get(completion.start..caret)
                .is_some_and(|typed| typed.chars().all(is_word));
        if !in_word {
            self.completion = None;
            return false;
        }
        if completion.loading {
            return false;
        }
        let typed = &text[completion.start..caret];
        completion.shown = matching(&completion.items, typed);
        completion.picked = completion
            .picked
            .min(completion.shown.len().saturating_sub(1));
        if completion.shown.is_empty() {
            self.completion = None;
        }
        false
    }

    /// Takes in what the language server offered for the completions asked at `version`, when they are still the ones open.
    pub fn take_completions(&mut self, version: i32, items: Vec<Completion>) {
        if let Some(completion) = &mut self.completion
            && completion.asked == version
        {
            completion.items = items;
            completion.loading = false;
            self.refilter(false);
        }
    }

    /// Moves the pick of the completions open `delta` rows down, upwards when negative, wrapping around.
    pub fn move_completion(&mut self, delta: isize) {
        if let Some(completion) = &mut self.completion
            && !completion.shown.is_empty()
        {
            let count = completion.shown.len() as isize;
            completion.picked = (completion.picked as isize + delta).rem_euclid(count) as usize;
        }
    }

    /// The edits that write the completion picked, or the one at `row` of the list when given, in place of the word it completes; nothing when there is none.
    pub fn completion_edits(&self, row: Option<usize>) -> Option<Vec<Edit>> {
        let completion = self.completion.as_ref()?;
        let item = completion
            .shown
            .get(row.unwrap_or(completion.picked))
            .and_then(|place| completion.items.get(*place))?;
        let caret = self.field.caret();
        let (start, end) = match item.span {
            Some(span) => {
                let range = self.range_of(span);
                (range.start.min(caret), range.end.max(caret))
            }
            None => (completion.start.min(caret), caret),
        };
        Some(vec![
            Edit::Place(start, false),
            Edit::Place(end, true),
            Edit::Paste(item.text.clone()),
        ])
    }
}
