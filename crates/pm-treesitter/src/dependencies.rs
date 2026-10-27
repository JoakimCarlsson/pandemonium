//! The dependencies a manifest names, as the dependencies query captures them: `@name` the dependency's name and `@version` the version it asks for; a place captured as `@skip` takes the dependencies inside it out, such as one that comes from a git repository.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// One dependency a manifest names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyPlace {
    /// What the registry knows it by.
    pub name: String,
    /// The version or range the manifest asks for, as written.
    pub requirement: String,
    /// The bytes of the text the requirement stands in, without the quotes around it.
    pub version: Range<usize>,
}

/// The bytes of `range` in `text` without a pair of quotes around them, when it has one.
fn unquoted(text: &str, range: Range<usize>) -> Range<usize> {
    let held = &text[range.clone()];
    let quoted = held.len() >= 2
        && ["\"", "'"]
            .iter()
            .any(|quote| held.starts_with(quote) && held.ends_with(quote));
    if quoted {
        range.start + 1..range.end - 1
    } else {
        range
    }
}

impl Syntax {
    /// The dependencies `text` names, in the order they are written; none when the language declares no query for them.
    pub fn dependency_places(&self, text: &str) -> Vec<DependencyPlace> {
        let Some(query) = &self.grammar().dependencies else {
            return Vec::new();
        };
        let (name, version, skip) = (
            query.capture_index_for_name("name"),
            query.capture_index_for_name("version"),
            query.capture_index_for_name("skip"),
        );
        let (Some(name), Some(version)) = (name, version) else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        let mut found: Vec<DependencyPlace> = Vec::new();
        let mut skipped: Vec<Range<usize>> = Vec::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            let node_of = |index: u32| {
                held.captures()
                    .iter()
                    .find(|capture| capture.index == index)
                    .map(|capture| capture.node)
            };
            if let Some(node) = skip.and_then(node_of) {
                skipped.push(node.byte_range());
            }
            let (Some(called), Some(asked)) = (node_of(name), node_of(version)) else {
                continue;
            };
            let range = unquoted(text, asked.byte_range());
            let called = unquoted(text, called.byte_range());
            let entry = DependencyPlace {
                name: text[called].trim().to_owned(),
                requirement: text[range.clone()].trim().to_owned(),
                version: range,
            };
            if !found.contains(&entry) {
                found.push(entry);
            }
        }
        found.retain(|place| {
            !skipped
                .iter()
                .any(|gone| gone.start <= place.version.start && place.version.end <= gone.end)
        });
        found.sort_by_key(|place| place.version.start);
        found
    }
}
