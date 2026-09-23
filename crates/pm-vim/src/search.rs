//! Finding text: what `/`, `?`, `n` and `*` look for, and where it is.
//!
//! The pattern is literal text rather than a regular expression, matched
//! without regard to case unless it holds a capital letter, which is how
//! most readers set vim up anyway.

use pm_text::Buffer;

use crate::text::{Class, class};

/// What a search looks for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pattern {
    /// The text to find.
    pub text: String,
    /// Whether it only counts where it stands as a whole word.
    pub whole_word: bool,
}

/// The last search made, for `n` and `N` to repeat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LastSearch {
    /// What was looked for.
    pub pattern: Pattern,
    /// Whether it looked forward.
    pub forward: bool,
}

impl Pattern {
    /// The offset of the next match after `from`, or before it when not
    /// `forward`, wrapping round the end of the buffer.
    pub(crate) fn find(&self, buffer: &Buffer, from: usize, forward: bool) -> Option<usize> {
        let needle = self.text.chars().collect::<Vec<_>>();
        if needle.is_empty() {
            return None;
        }
        let haystack = buffer.contents().chars().collect::<Vec<_>>();
        let exact = needle.iter().any(|ch| ch.is_uppercase());
        let matches = |at: usize| self.matches_at(&haystack, &needle, at, exact);

        let len = haystack.len();
        match forward {
            true => (from + 1..len)
                .chain(0..=from.min(len))
                .find(|at| matches(*at)),
            false => (0..from.min(len))
                .rev()
                .chain((from..len).rev())
                .find(|at| matches(*at)),
        }
    }

    /// Whether `needle` is found in `haystack` starting at `at`.
    fn matches_at(&self, haystack: &[char], needle: &[char], at: usize, exact: bool) -> bool {
        let Some(found) = haystack.get(at..at + needle.len()) else {
            return false;
        };
        let same = found.iter().zip(needle).all(|(left, right)| match exact {
            true => left == right,
            false => left.to_lowercase().eq(right.to_lowercase()),
        });
        if !same || !self.whole_word {
            return same;
        }
        let is_word =
            |index: Option<&char>| index.is_some_and(|ch| class(*ch, false) == Class::Word);
        let before = at.checked_sub(1).and_then(|index| haystack.get(index));
        !is_word(before) && !is_word(haystack.get(at + needle.len()))
    }
}
