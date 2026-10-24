//! Finding text in a document and replacing it: as typed or as a regular expression, matching case or not, whole words or anywhere, one match at a time or all at once.

use std::ops::Range;

use pm_text::Edit;
use regex::{Regex, RegexBuilder};

use crate::document::Document;

/// The most matches found in one document.
const MOST_MATCHES: usize = 10_000;

/// What is looked for in a document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Query {
    /// The text, or the regular expression.
    pub text: String,
    /// Whether upper and lower case must match.
    pub case: bool,
    /// Whether only whole words match.
    pub word: bool,
    /// Whether the text is a regular expression.
    pub regex: bool,
}

impl Query {
    /// The first stretch of `text` the query finds; nothing when it finds none or cannot be used.
    pub fn first_in(&self, text: &str) -> Option<Range<usize>> {
        let pattern = self.pattern().ok()??;
        pattern
            .find(text)
            .filter(|found| !found.is_empty())
            .map(|found| found.range())
    }

    /// The regular expression that finds what is looked for, or why it cannot be made; nothing for an empty query.
    fn pattern(&self) -> Result<Option<Regex>, String> {
        if self.text.is_empty() {
            return Ok(None);
        }
        let body = if self.regex {
            self.text.clone()
        } else {
            regex::escape(&self.text)
        };
        let body = if self.word {
            format!(r"\b(?:{body})\b")
        } else {
            body
        };
        RegexBuilder::new(&body)
            .case_insensitive(!self.case)
            .multi_line(true)
            .build()
            .map(Some)
            .map_err(|error| match error {
                regex::Error::Syntax(said) => said
                    .lines()
                    .last()
                    .unwrap_or("not a regular expression")
                    .trim()
                    .to_owned(),
                other => other.to_string(),
            })
    }
}

impl Document {
    /// Every stretch of the text `query` finds, in order, no more than a limit of them; why the query cannot be used when it cannot.
    pub fn find_all(&self, query: &Query) -> Result<Vec<Range<usize>>, String> {
        let Some(pattern) = query.pattern()? else {
            return Ok(Vec::new());
        };
        Ok(pattern
            .find_iter(self.field.text())
            .filter(|found| !found.is_empty())
            .take(MOST_MATCHES)
            .map(|found| found.range())
            .collect())
    }

    /// Which of `matches` is selected now, when one is.
    pub fn selected_match(&self, matches: &[Range<usize>]) -> Option<usize> {
        let selected = self.field.selection()?;
        matches.iter().position(|found| *found == selected)
    }

    /// The edit that selects the match after the selection or the caret among `matches`, or the one before it when `back`, going round the end of the text; nothing when there are none.
    pub fn next_match_edit(&self, matches: &[Range<usize>], back: bool) -> Option<Edit> {
        self.next_match_edit_with(matches, back, true)
    }

    /// The edit that selects the match after the selection or the caret among `matches`, or the one before it when `back`, going round the end of the text only when `wrap`.
    pub fn next_match_edit_with(
        &self,
        matches: &[Range<usize>],
        back: bool,
        wrap: bool,
    ) -> Option<Edit> {
        let caret = self.field.caret();
        let range = self.field.selection().unwrap_or(caret..caret);
        let found = if back {
            matches
                .iter()
                .rev()
                .find(|found| found.start < range.start)
                .or(matches.last().filter(|_| wrap))
        } else {
            matches
                .iter()
                .find(|found| found.start >= range.end && *found != &range)
                .or(matches.first().filter(|_| wrap))
        }?;
        Some(Edit::Select(found.start, found.end))
    }

    /// The edit that selects the first of `matches` at or after where the selection starts, for a query being typed, going round to the first.
    pub fn first_match_edit(&self, matches: &[Range<usize>]) -> Option<Edit> {
        let caret = self.field.caret();
        let start = self.field.selection().map_or(caret, |range| range.start);
        let found = matches
            .iter()
            .find(|found| found.start >= start)
            .or(matches.first())?;
        Some(Edit::Select(found.start, found.end))
    }

    /// What `replacement` writes in place of the stretch `found`: as it is, or with the groups of a regular expression filled in.
    fn replacement_for(&self, query: &Query, found: Range<usize>, replacement: &str) -> String {
        if !query.regex {
            return replacement.to_owned();
        }
        let Ok(Some(pattern)) = query.pattern() else {
            return replacement.to_owned();
        };
        let text = &self.field.text()[found];
        pattern.replace(text, replacement).into_owned()
    }

    /// The edit that replaces the selected match with `replacement`, leaving the caret after it; nothing when no match is selected.
    pub fn replace_edit(
        &self,
        query: &Query,
        matches: &[Range<usize>],
        replacement: &str,
    ) -> Option<Edit> {
        let index = self.selected_match(matches)?;
        let found = matches[index].clone();
        let written = self.replacement_for(query, found.clone(), replacement);
        let end = found.start + written.len();
        Some(Edit::Replace(found, written, (end, end)))
    }

    /// The edit that replaces every one of `matches` with `replacement` as one change; nothing when there are none.
    pub fn replace_all_edit(
        &self,
        query: &Query,
        matches: &[Range<usize>],
        replacement: &str,
    ) -> Option<Edit> {
        let first = matches.first()?.start;
        let last = matches.last()?.end;
        let text = self.field.text();
        let mut written = String::new();
        let mut at = first;
        for found in matches {
            written.push_str(&text[at..found.start]);
            written.push_str(&self.replacement_for(query, found.clone(), replacement));
            at = found.end;
        }
        written.push_str(&text[at..last]);
        let caret = first;
        Some(Edit::Replace(first..last, written, (caret, caret)))
    }
}
