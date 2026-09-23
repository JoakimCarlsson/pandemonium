//! Finding text: what `/`, `?`, `n`, `*`, `:s` and `:g` look for, and where
//! it is.
//!
//! Patterns are written the way vim writes them — `\(` groups, `\<` starts a
//! word, `\v` makes every symbol special — and turned into a regular
//! expression here, once. A pattern with no capital letter matches without
//! regard to case unless it says `\C`; one with a capital, or `\c`, says
//! otherwise.

use std::ops::Range;

use pm_text::{Buffer, Position};
use regex::{Regex, RegexBuilder};

/// What a search looks for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pattern {
    /// The pattern as it was typed, in vim's syntax.
    pub source: String,
    /// Whether it only counts where it stands as a whole word.
    pub whole_word: bool,
}

/// The last search made, for `n` and `N`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LastSearch {
    /// What was looked for.
    pub pattern: Pattern,
    /// Whether it looked forward.
    pub forward: bool,
}

impl Pattern {
    /// A pattern typed as `source`.
    pub(crate) fn typed(source: &str) -> Self {
        Self {
            source: source.to_owned(),
            whole_word: false,
        }
    }

    /// The pattern that finds `word` and nothing it is part of.
    pub(crate) fn word(word: &str) -> Self {
        Self {
            source: word.to_owned(),
            whole_word: true,
        }
    }

    /// The regular expression the pattern stands for, if it makes one.
    pub(crate) fn regex(&self) -> Option<Regex> {
        if self.source.is_empty() {
            return None;
        }
        let (body, case) = match self.whole_word {
            true => (format!(r"\b{}\b", regex::escape(&self.source)), None),
            false => translate(&self.source),
        };
        let insensitive = case.unwrap_or_else(|| !has_capital(&self.source));
        RegexBuilder::new(&body)
            .case_insensitive(insensitive)
            .multi_line(true)
            .build()
            .or_else(|_| {
                RegexBuilder::new(&regex::escape(&self.source))
                    .case_insensitive(insensitive)
                    .build()
            })
            .ok()
    }

    /// The offset of the next match after `from`, or before it when not
    /// `forward`, wrapping round the end of the buffer.
    pub(crate) fn find(&self, buffer: &Buffer, from: usize, forward: bool) -> Option<usize> {
        let regex = self.regex()?;
        let contents = buffer.contents();
        let starts = regex
            .find_iter(&contents)
            .map(|found| contents[..found.start()].chars().count())
            .collect::<Vec<_>>();
        match forward {
            true => starts
                .iter()
                .find(|start| **start > from)
                .or(starts.first())
                .copied(),
            false => starts
                .iter()
                .rev()
                .find(|start| **start < from)
                .or(starts.last())
                .copied(),
        }
    }

    /// Every match on `lines`, as the spans of the text they cover.
    pub(crate) fn matches_on(&self, buffer: &Buffer, lines: Range<usize>) -> Vec<Range<Position>> {
        let Some(regex) = self.regex() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for line in lines.start..lines.end.min(buffer.line_count()) {
            let text = buffer.line_text(line);
            for matched in regex.find_iter(&text).filter(|matched| !matched.is_empty()) {
                let start = text[..matched.start()].chars().count();
                let end = start + matched.as_str().chars().count();
                found.push(Position::new(line, start)..Position::new(line, end));
            }
        }
        found
    }
}

/// `source` in vim's pattern syntax as a regular expression, with the case
/// it asks for with `\c` or `\C`.
///
/// Without `\v` the symbols that group, alternate and repeat are literal
/// until a backslash makes them special, the way vim's default `magic`
/// reads them; with it, they are special as they stand.
pub(crate) fn translate(source: &str) -> (String, Option<bool>) {
    let mut out = String::new();
    let mut case = None;
    let mut very_magic = false;
    let mut in_brace = false;
    let mut in_class = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_class {
            out.push(ch);
            in_class = ch != ']';
            continue;
        }
        if ch == '\\' {
            let Some(next) = chars.next() else {
                out.push_str(r"\\");
                break;
            };
            match next {
                'v' => very_magic = true,
                'c' => case = Some(true),
                'C' => case = Some(false),
                '<' | '>' => out.push_str(r"\b"),
                '(' | ')' | '|' | '+' | '?' if !very_magic => out.push(next),
                '=' if !very_magic => out.push('?'),
                '{' if !very_magic => {
                    out.push('{');
                    in_brace = true;
                }
                'n' => out.push('\n'),
                't' => out.push('\t'),
                's' | 'S' | 'd' | 'D' | 'w' | 'W' => {
                    out.push('\\');
                    out.push(next);
                }
                'a' => out.push_str("[a-zA-Z]"),
                'l' => out.push_str("[a-z]"),
                'u' => out.push_str("[A-Z]"),
                'x' => out.push_str("[0-9a-fA-F]"),
                other => out.push_str(&regex::escape(&other.to_string())),
            }
            continue;
        }
        match ch {
            '[' => {
                out.push('[');
                in_class = true;
                if chars.peek() == Some(&'^') {
                    out.push('^');
                    chars.next();
                }
                if chars.peek() == Some(&']') {
                    out.push_str(r"\]");
                    chars.next();
                }
            }
            '.' | '*' | '^' | '$' => out.push(ch),
            '}' if in_brace => {
                out.push('}');
                in_brace = false;
            }
            '<' | '>' if very_magic => out.push_str(r"\b"),
            '=' if very_magic => out.push('?'),
            '(' | ')' | '|' | '+' | '?' | '{' | '}' if very_magic => out.push(ch),
            other => out.push_str(&regex::escape(&other.to_string())),
        }
    }
    (out, case)
}

/// Whether `source` holds a capital letter of its own, not one that names
/// a class after a backslash.
fn has_capital(source: &str) -> bool {
    let mut escaped = false;
    source.chars().any(|ch| {
        let capital = !escaped && ch.is_uppercase();
        escaped = !escaped && ch == '\\';
        capital
    })
}

/// A replacement written the way `:s` writes it, as the regex crate reads
/// one: `\1` and `&` name what was matched, `\r` and `\n` break the line.
pub(crate) fn replacement(written: &str) -> String {
    let mut out = String::new();
    let mut chars = written.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some(digit @ '0'..='9') => out.push_str(&format!("${{{digit}}}")),
                Some('r' | 'n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('&') => out.push('&'),
                Some('$') => out.push_str("$$"),
                Some(other) => out.push(other),
                None => out.push('\\'),
            },
            '&' => out.push_str("${0}"),
            '$' => out.push_str("$$"),
            other => out.push(other),
        }
    }
    out
}
