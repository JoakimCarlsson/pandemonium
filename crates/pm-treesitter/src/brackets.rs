//! Brackets that pair up, as the brackets query captures them `@open` and `@close` in one match.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

impl Syntax {
    /// The bracket of `text` the caret at byte `caret` touches and the one it pairs with, the opening one first.
    pub fn bracket_pair(&self, text: &str, caret: usize) -> Option<(Range<usize>, Range<usize>)> {
        let query = self.grammar().brackets.as_ref()?;
        let open = query.capture_index_for_name("open")?;
        let close = query.capture_index_for_name("close")?;
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(caret.saturating_sub(1)..(caret + 1).min(text.len()));
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        let mut best: Option<(Range<usize>, Range<usize>)> = None;
        while let Some(found) = matches.next() {
            let range_of = |wanted: u32| {
                found
                    .captures()
                    .iter()
                    .find(|capture| capture.index == wanted)
                    .map(|capture| capture.node.byte_range())
            };
            let (Some(opening), Some(closing)) = (range_of(open), range_of(close)) else {
                continue;
            };
            let touches = |range: &Range<usize>| range.start == caret || range.end == caret;
            if !touches(&opening) && !touches(&closing) {
                continue;
            }
            let tighter = best
                .as_ref()
                .is_none_or(|held| closing.end - opening.start < held.1.end - held.0.start);
            if tighter {
                best = Some((opening, closing));
            }
        }
        best
    }

    /// Every bracket of `text` that pairs with another, in order, with how many pairs enclose it; the two of a pair stand at the same depth.
    pub fn bracket_depths(&self, text: &str) -> Vec<(Range<usize>, usize)> {
        let Some(query) = self.grammar().brackets.as_ref() else {
            return Vec::new();
        };
        let (Some(open), Some(close)) = (
            query.capture_index_for_name("open"),
            query.capture_index_for_name("close"),
        ) else {
            return Vec::new();
        };
        let mut found: Vec<(Range<usize>, bool)> = Vec::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(pair) = matches.next() {
            for capture in pair.captures() {
                if capture.index == open {
                    found.push((capture.node.byte_range(), true));
                } else if capture.index == close {
                    found.push((capture.node.byte_range(), false));
                }
            }
        }
        found.sort_by_key(|(range, _)| range.start);
        found.dedup_by(|later, earlier| later.0 == earlier.0);
        let mut depth = 0usize;
        found
            .into_iter()
            .map(|(range, opens)| {
                if opens {
                    depth += 1;
                    (range, depth - 1)
                } else {
                    depth = depth.saturating_sub(1);
                    (range, depth)
                }
            })
            .collect()
    }
}
