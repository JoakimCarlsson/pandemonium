//! Matching and replacing text one line at a time.

use std::ops::Range;

use regex::{Regex, RegexBuilder};

/// The options shared by file and worktree search.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Query {
    /// The pattern to find.
    pub text: String,
    /// Whether the pattern uses regular expression syntax.
    pub regex: bool,
    /// Whether letter case must match.
    pub case_sensitive: bool,
    /// Whether matches must have word boundaries.
    pub whole_word: bool,
}

/// A compiled query for matching individual lines.
pub struct Finder {
    /// The compiled pattern.
    pattern: Regex,
    /// Whether replacement text expands capture groups.
    regex: bool,
}

impl Finder {
    /// Compiles a query or returns the regular expression error.
    pub fn new(query: &Query) -> Result<Self, String> {
        let body = if query.regex {
            query.text.clone()
        } else {
            regex::escape(&query.text)
        };
        let pattern = if query.whole_word {
            format!(r"\b(?:{body})\b")
        } else {
            body
        };
        let pattern = RegexBuilder::new(&pattern)
            .case_insensitive(!query.case_sensitive)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            pattern,
            regex: query.regex,
        })
    }

    /// Returns the matches in one line with character column offsets.
    pub fn line(&self, text: &str) -> Vec<Range<usize>> {
        self.pattern
            .find_iter(text)
            .map(|found| text[..found.start()].chars().count()..text[..found.end()].chars().count())
            .collect()
    }

    /// Expands capture groups for the match, or returns literal replacement text.
    pub fn replacement(&self, line: &str, found: Range<usize>, with: &str) -> String {
        if !self.regex {
            return with.to_owned();
        }
        let start = line
            .char_indices()
            .nth(found.start)
            .map_or(line.len(), |(byte, _)| byte);
        let end = line
            .char_indices()
            .nth(found.end)
            .map_or(line.len(), |(byte, _)| byte);
        let Some(captures) = self.pattern.captures_at(line, start) else {
            return with.to_owned();
        };
        if !captures
            .get(0)
            .is_some_and(|matched| matched.start() == start && matched.end() == end)
        {
            return with.to_owned();
        }
        let mut expanded = String::new();
        captures.expand(with, &mut expanded);
        expanded
    }
}
