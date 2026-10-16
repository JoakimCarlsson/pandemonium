//! The items of a text an outline lists, as the outline query captures them: the whole of each `@item`, its `@name` and the `@context` words before it.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// One item of an outline: a function, a type, a section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    /// What it is called.
    pub name: String,
    /// The words that say what it is, such as `fn` or `struct`.
    pub context: String,
    /// The bytes of the text it covers.
    pub range: Range<usize>,
}

impl Syntax {
    /// The items of `text` that hold the byte `offset`, the outermost first.
    pub fn enclosing(&self, text: &str, offset: usize) -> Vec<Item> {
        let Some(query) = &self.grammar().outline else {
            return Vec::new();
        };
        let (Some(item), Some(name)) = (
            query.capture_index_for_name("item"),
            query.capture_index_for_name("name"),
        ) else {
            return Vec::new();
        };
        let context = query.capture_index_for_name("context");
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(offset..(offset + 1).min(text.len()).max(offset));
        let mut found: Vec<Item> = Vec::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            let words = |wanted: Option<u32>| {
                held.captures()
                    .iter()
                    .filter(|capture| Some(capture.index) == wanted)
                    .map(|capture| text[capture.node.byte_range()].trim())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let Some(range) = held
                .captures()
                .iter()
                .find(|capture| capture.index == item)
                .map(|capture| capture.node.byte_range())
            else {
                continue;
            };
            if !(range.start <= offset && offset <= range.end) {
                continue;
            }
            let entry = Item {
                name: words(Some(name)),
                context: words(context),
                range,
            };
            if !entry.name.is_empty() && !found.contains(&entry) {
                found.push(entry);
            }
        }
        found.sort_by(|left, right| {
            left.range
                .start
                .cmp(&right.range.start)
                .then(right.range.end.cmp(&left.range.end))
        });
        found
    }
}
