//! The dependencies a manifest lists, as the dependencies query captures them: the name of each as `@name` and its version as `@version`.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// The capture that holds the name of a dependency.
const NAME: &str = "name";

/// The capture that holds the version of a dependency, without the quotes around it.
const VERSION: &str = "version";

/// One dependency a manifest lists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    /// What it is called.
    pub name: String,
    /// The version the manifest asks for, as written.
    pub version: String,
    /// The bytes of the text the version is written in.
    pub range: Range<usize>,
}

/// `range` of `text` without the quotes around it, when it is written inside a pair of them.
fn unquoted(text: &str, range: Range<usize>) -> Range<usize> {
    let written = &text[range.clone()];
    let quoted = written.len() >= 2
        && ["\"", "'"]
            .iter()
            .any(|quote| written.starts_with(quote) && written.ends_with(quote));
    if quoted {
        range.start + 1..range.end - 1
    } else {
        range
    }
}

impl Syntax {
    /// The dependencies `text` lists, in the order they are written; none when the language finds none.
    pub fn dependencies(&self, text: &str) -> Vec<Dependency> {
        let Some(query) = &self.grammar().dependencies else {
            return Vec::new();
        };
        let (Some(name), Some(version)) = (
            query.capture_index_for_name(NAME),
            query.capture_index_for_name(VERSION),
        ) else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        let mut found: Vec<Dependency> = Vec::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            let node = |index| {
                held.captures()
                    .iter()
                    .find(|capture| capture.index == index)
                    .map(|capture| capture.node)
            };
            let (Some(name), Some(version)) = (node(name), node(version)) else {
                continue;
            };
            let range = unquoted(text, version.byte_range());
            let entry = Dependency {
                name: text[name.byte_range()].trim().to_owned(),
                version: text[range.clone()].trim().to_owned(),
                range,
            };
            if !found.contains(&entry) {
                found.push(entry);
            }
        }
        found.sort_by_key(|entry| entry.range.start);
        found
    }
}
