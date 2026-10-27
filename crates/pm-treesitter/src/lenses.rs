//! The places of a text that carry a lens, as the lenses query captures them: the whole of each place as `@lens.<kind>` and, where the lens speaks of it, its `@name`.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// The words every capture of a place begins with, after which comes its kind.
const PREFIX: &str = "lens.";

/// One place of a text that carries a lens: a service of a Compose file, a script of a `package.json`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LensPlace {
    /// What kind of place it is, as the query names it.
    pub kind: String,
    /// What it is called in the text, when the query captures a name.
    pub name: Option<String>,
    /// The bytes of the text it covers.
    pub range: Range<usize>,
}

impl Syntax {
    /// The places of `text` that carry a lens, in the order they start; none when the language declares none.
    pub fn lens_places(&self, text: &str) -> Vec<LensPlace> {
        let Some(query) = &self.grammar().lenses else {
            return Vec::new();
        };
        let name = query.capture_index_for_name("name");
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
        let mut found: Vec<LensPlace> = Vec::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            for capture in held.captures() {
                let Some((_, kind)) = kinds.iter().find(|(index, _)| *index == capture.index)
                else {
                    continue;
                };
                let called = name.and_then(|name| {
                    held.captures()
                        .iter()
                        .find(|capture| capture.index == name)
                        .map(|capture| text[capture.node.byte_range()].trim().to_owned())
                });
                let entry = LensPlace {
                    kind: (*kind).to_owned(),
                    name: called,
                    range: capture.node.byte_range(),
                };
                if !found.contains(&entry) {
                    found.push(entry);
                }
            }
        }
        found.sort_by_key(|entry| (entry.range.start, entry.range.end));
        found
    }
}
