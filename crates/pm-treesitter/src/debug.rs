//! The names of a text whose value is worth showing while the program is paused, as the debugger query captures them: `@debug-variable` is a name to look up and `@debug-scope` a piece of text within which names are looked up.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

impl Syntax {
    /// The names within the bytes `window` of `text` that are variables to show the value of, each with where it is, in order; none when the language marks none.
    pub fn debug_variables(&self, text: &str, window: Range<usize>) -> Vec<(Range<usize>, String)> {
        let Some(query) = &self.grammar().debugger else {
            return Vec::new();
        };
        let Some(variable) = query.capture_index_for_name("debug-variable") else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(window.clone());
        let mut found: Vec<(Range<usize>, String)> = Vec::new();
        let mut captures = cursor.captures(query, self.tree.root_node(), text.as_bytes());
        while let Some((held, index)) = captures.next() {
            let capture = held.captures()[*index];
            let range = capture.node.byte_range();
            if capture.index == variable
                && range.start >= window.start
                && range.end <= window.end
                && !found.iter().any(|(known, _)| *known == range)
            {
                found.push((range.clone(), text[range].to_owned()));
            }
        }
        found.sort_by_key(|(range, _)| range.start);
        found
    }

    /// The bytes of the smallest piece of `text` marked as a scope of variables that holds the byte `offset`; none when the language marks none there.
    pub fn debug_scope(&self, text: &str, offset: usize) -> Option<Range<usize>> {
        let query = self.grammar().debugger.as_ref()?;
        let scope = query.capture_index_for_name("debug-scope")?;
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(offset.saturating_sub(1)..(offset + 1).min(text.len()));
        let mut best: Option<Range<usize>> = None;
        let mut captures = cursor.captures(query, self.tree.root_node(), text.as_bytes());
        while let Some((held, index)) = captures.next() {
            let capture = held.captures()[*index];
            let range = capture.node.byte_range();
            if capture.index == scope
                && range.start <= offset
                && offset <= range.end
                && best.as_ref().is_none_or(|held| range.len() < held.len())
            {
                best = Some(range);
            }
        }
        best
    }
}
