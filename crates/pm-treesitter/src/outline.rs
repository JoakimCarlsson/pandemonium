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
    /// Where its name starts.
    pub name_start: usize,
    /// How many items hold it.
    pub depth: usize,
}

impl Syntax {
    /// The items of `text` that touch the bytes `within`, outermost and earliest first, each with how many of them hold it.
    fn items(&self, text: &str, within: Range<usize>) -> Vec<Item> {
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
        cursor.set_byte_range(within);
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
            let name_start = held
                .captures()
                .iter()
                .find(|capture| capture.index == name)
                .map_or(range.start, |capture| capture.node.start_byte());
            let entry = Item {
                name: words(Some(name)),
                context: words(context),
                range,
                name_start,
                depth: 0,
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
        let mut open: Vec<Range<usize>> = Vec::new();
        for entry in &mut found {
            open.retain(|outer| outer.start <= entry.range.start && entry.range.end <= outer.end);
            entry.depth = open.len();
            open.push(entry.range.clone());
        }
        found
    }

    /// The items of `text` that hold the byte `offset`, the outermost first.
    pub fn enclosing(&self, text: &str, offset: usize) -> Vec<Item> {
        self.items(text, offset..(offset + 1).min(text.len()).max(offset))
            .into_iter()
            .filter(|entry| entry.range.start <= offset && offset <= entry.range.end)
            .collect()
    }

    /// Every item of `text`, in the order they start, each with how many items hold it.
    pub fn outline(&self, text: &str) -> Vec<Item> {
        self.items(text, 0..text.len())
    }
}
