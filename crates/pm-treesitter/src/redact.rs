//! The stretches of a text that hold private values, as the redactions query captures them `@redact`.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

impl Syntax {
    /// The bytes of `text` that hold private values, in order and not overlapping; none when the language marks none.
    pub fn redactions(&self, text: &str) -> Vec<Range<usize>> {
        let Some(query) = &self.grammar().redactions else {
            return Vec::new();
        };
        let Some(redact) = query.capture_index_for_name("redact") else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        let mut found: Vec<Range<usize>> = Vec::new();
        let mut captures = cursor.captures(query, self.tree.root_node(), text.as_bytes());
        while let Some((held, index)) = captures.next() {
            let capture = held.captures()[*index];
            if capture.index == redact {
                found.push(capture.node.byte_range());
            }
        }
        found.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in found {
            match merged.last_mut() {
                Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        merged
    }
}
