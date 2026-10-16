//! How deep a new line is indented, as the indents query says: a node captured `@indent` indents the lines inside it, and a token captured `@outdent` closes one.

use pm_buffer::Lines;
use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// What breaking a line at a place calls for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Indent {
    /// Whether the new line goes one level deeper than the line broken, because a block opened on that line is still open at the break.
    pub deeper: bool,
    /// Whether what follows the break closes that block, so it goes on a line of its own at the level of the line broken.
    pub closing: bool,
}

impl Syntax {
    /// What breaking the line of `text`, whose lines `lines` indexes, at the byte `offset` calls for.
    pub fn indent_at(&self, text: &str, lines: &Lines, offset: usize) -> Indent {
        let Some(query) = &self.grammar().indents else {
            return Indent::default();
        };
        let indent = query.capture_index_for_name("indent");
        let outdent = query.capture_index_for_name("outdent");
        let row = lines.line_of(offset);
        let line = lines.line(row, text);
        let rest = text[offset..line.end].trim_start();
        let after = line.end - rest.len();
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(line.start..line.end.max(offset + 1).min(text.len()));
        let mut found = Indent::default();
        let mut captures = cursor.captures(query, self.tree.root_node(), text.as_bytes());
        while let Some((found_match, index)) = captures.next() {
            let capture = found_match.captures()[*index];
            let node = capture.node;
            if Some(capture.index) == indent
                && node.start_position().row == row
                && node.start_byte() < offset
                && node.end_byte() > offset
            {
                found.deeper = true;
            }
            if Some(capture.index) == outdent && !rest.is_empty() && node.start_byte() == after {
                found.closing = true;
            }
        }
        found.closing &= found.deeper;
        found
    }

    /// Whether the token of `text` that starts at byte `offset` closes a block, as the indents query captures it `@outdent`.
    pub fn closes_at(&self, text: &str, offset: usize) -> bool {
        let Some(query) = &self.grammar().indents else {
            return false;
        };
        let outdent = query.capture_index_for_name("outdent");
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(offset..(offset + 1).min(text.len()));
        let mut captures = cursor.captures(query, self.tree.root_node(), text.as_bytes());
        while let Some((found_match, index)) = captures.next() {
            let capture = found_match.captures()[*index];
            if Some(capture.index) == outdent && capture.node.start_byte() == offset {
                return true;
            }
        }
        false
    }
}
