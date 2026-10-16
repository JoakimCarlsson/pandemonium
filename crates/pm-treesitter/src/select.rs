//! Growing a selection along the syntax tree, from a stretch of the text to the smallest piece of syntax that holds more of it, and the piece of syntax around a place that can be folded away.

use std::ops::Range;

use crate::syntax::Syntax;

impl Syntax {
    /// The smallest piece of syntax that holds the bytes `range` and more besides, or nothing when the whole text is already held.
    pub fn larger_node(&self, range: Range<usize>) -> Option<Range<usize>> {
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(range.start, range.end)?;
        loop {
            let held = node.byte_range();
            if held.start <= range.start && held.end >= range.end && held != range {
                return Some(held);
            }
            node = node.parent()?;
        }
    }

    /// Whether the byte `offset` is inside a string or a comment, where brackets and quotes typed are text rather than code.
    pub fn in_text(&self, offset: usize) -> bool {
        let Some(mut node) = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset, offset)
        else {
            return false;
        };
        loop {
            let kind = node.kind();
            if kind.contains("string") || kind.contains("comment") {
                return node.start_byte() < offset;
            }
            match node.parent() {
                Some(parent) => node = parent,
                None => return false,
            }
        }
    }

    /// The bytes of the smallest piece of syntax around the byte `offset` that spans at least three lines, so that the lines between its first and its last can be folded away.
    pub fn fold_range(&self, offset: usize) -> Option<Range<usize>> {
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset, offset)?;
        loop {
            if node.end_position().row > node.start_position().row + 1 {
                return Some(node.byte_range());
            }
            node = node.parent()?;
        }
    }
}
