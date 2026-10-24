//! The places of a text that can be run, as the runnables query captures them: the whole of each place as `@run.<kind>` and its `@name`.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// The words every capture of a place begins with, after which comes its kind.
const PREFIX: &str = "run.";

/// One place of a text that can be run: a `main`, a test, a module of tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Runnable {
    /// What kind of place it is, as the query names it: `main`, `test` or `module`.
    pub kind: String,
    /// What it is called in the text.
    pub name: String,
    /// The bytes of the text it covers.
    pub range: Range<usize>,
}

impl Syntax {
    /// The places of `text` that can be run, in the order they start; none when the language marks none.
    pub fn runnables(&self, text: &str) -> Vec<Runnable> {
        let Some(query) = &self.grammar().runnables else {
            return Vec::new();
        };
        let Some(name) = query.capture_index_for_name("name") else {
            return Vec::new();
        };
        let kinds: Vec<(u32, &str)> = query
            .capture_names()
            .iter()
            .enumerate()
            .filter_map(|(index, capture)| {
                capture
                    .strip_prefix(PREFIX)
                    .map(|kind| (index as u32, kind))
            })
            .collect();
        let mut cursor = QueryCursor::new();
        let mut found: Vec<Runnable> = Vec::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            let Some((place, kind)) = held.captures().iter().find_map(|capture| {
                kinds
                    .iter()
                    .find(|(index, _)| *index == capture.index)
                    .map(|(_, kind)| (capture.node, *kind))
            }) else {
                continue;
            };
            let Some(called) = held
                .captures()
                .iter()
                .find(|capture| capture.index == name)
                .map(|capture| text[capture.node.byte_range()].trim().to_owned())
            else {
                continue;
            };
            let entry = Runnable {
                kind: kind.to_owned(),
                name: called,
                range: place.byte_range(),
            };
            if !found.contains(&entry) {
                found.push(entry);
            }
        }
        found.sort_by_key(|entry| (entry.range.start, entry.range.end));
        found
    }
}
