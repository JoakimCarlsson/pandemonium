//! The completions open at the caret: narrowed to the word typed, and the edits that write the one picked.

use pm_protocol::Completion;
use pm_text::Edit;

use crate::document::Document;

/// Whether `character` is part of a word a completion finishes.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

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

    /// The completion picked, as its language server wrote it, when the server has more to say about it that was not asked yet; it is marked as asked.
    pub fn completion_to_resolve(&mut self) -> Option<String> {
        let completion = self.completion.as_mut()?;
        let raw = completion
            .shown
            .get(completion.picked)
            .and_then(|place| completion.items.get(*place))?
            .resolve
            .clone()?;
        if completion.resolving.as_deref() == Some(raw.as_str()) {
            return None;
        }
        completion.resolving = Some(raw.clone());
        Some(raw)
    }

    /// Takes in what the language server added to the completion `resolved` stands for, when it is still in the list: its documentation, detail and the edits choosing it makes besides writing it.
    pub fn take_resolved(&mut self, resolved: Completion) {
        let (Some(completion), Some(raw)) = (&mut self.completion, resolved.resolve) else {
            return;
        };
        if let Some(item) = completion
            .items
            .iter_mut()
            .find(|item| item.resolve.as_deref() == Some(raw.as_str()))
        {
            item.additional_edits = resolved.additional_edits;
            item.documentation = resolved.documentation.or(item.documentation.take());
            item.detail = resolved.detail.or(item.detail.take());
            item.resolve = None;
        }
    }

    /// The edits that write the completion picked, or the one at `row` of the list when given, in place of the word it completes, and make the other edits choosing it calls for, such as the line that imports it; nothing when there is none.
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
        Some(self.replacement_edits(start, end, &item.text, &item.additional_edits))
    }

    /// The edits that write `text` in place of the bytes `start` to `end`, the caret after it, and make the changes `extras` call for, such as the line that imports what is written, in an order that keeps every stretch right.
    pub fn replacement_edits(
        &self,
        start: usize,
        end: usize,
        text: &str,
        extras: &[pm_protocol::TextChange],
    ) -> Vec<Edit> {
        let caret = self.field.caret();
        let mut before: Vec<(std::ops::Range<usize>, &str)> = Vec::new();
        let mut after: Vec<(std::ops::Range<usize>, &str)> = Vec::new();
        for change in extras {
            let range = change
                .span
                .map_or(0..self.field.text().len(), |span| self.range_of(span));
            if range.end <= start {
                before.push((range, &change.text));
            } else if range.start >= end {
                after.push((range, &change.text));
            }
        }
        before.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        after.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        let mut edits: Vec<Edit> = after
            .into_iter()
            .map(|(range, text)| Edit::Replace(range, text.to_owned(), (caret, caret)))
            .collect();
        edits.extend([
            Edit::Place(start, false),
            Edit::Place(end, true),
            Edit::Paste(text.to_owned()),
        ]);
        let mut at = start + text.len();
        for (range, text) in before {
            at = (at + text.len()).saturating_sub(range.len());
            edits.push(Edit::Replace(range, text.to_owned(), (at, at)));
        }
        edits
    }
}
