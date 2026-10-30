//! The places of a text that can be run, as the runnables query captures them: the whole of each place as `@run`, its tags from `(#set! tag …)` and every other capture not starting with an underscore as a variable.

use std::collections::BTreeMap;
use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// The capture that marks the whole of a place.
const RUN: &str = "run";

/// The property whose value tags a place; a pattern may set it more than once.
const TAG: &str = "tag";

/// One place of a text that can be run: a `main`, a test, a service of a compose file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Runnable {
    /// What the pattern that found it tags it with, in the order the pattern sets them; the tasks of a language and of a project are tied to places by these.
    pub tags: Vec<String>,
    /// What each of its other captures matched, by the capture's name; a command reads them as `$PM_CUSTOM_<name>`.
    pub variables: BTreeMap<String, String>,
    /// The bytes of the text it covers.
    pub range: Range<usize>,
}

impl Syntax {
    /// The places of `text` that can be run, in the order they start; none when the language marks none.
    pub fn runnables(&self, text: &str) -> Vec<Runnable> {
        let Some(query) = &self.grammar().runnables else {
            return Vec::new();
        };
        let Some(run) = query.capture_index_for_name(RUN) else {
            return Vec::new();
        };
        let names = query.capture_names();
        let mut cursor = QueryCursor::new();
        let mut found: Vec<Runnable> = Vec::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            let Some(place) = held
                .captures()
                .iter()
                .find(|capture| capture.index == run)
                .map(|capture| capture.node)
            else {
                continue;
            };
            let tags: Vec<String> = query
                .property_settings(held.pattern_index)
                .iter()
                .filter(|property| property.key.as_ref() == TAG)
                .filter_map(|property| property.value.as_ref().map(|value| value.to_string()))
                .collect();
            let mut variables = BTreeMap::new();
            for capture in held.captures() {
                let name = names[capture.index as usize];
                if capture.index != run && !name.starts_with('_') {
                    variables
                        .entry(name.to_owned())
                        .or_insert_with(|| text[capture.node.byte_range()].trim().to_owned());
                }
            }
            let entry = Runnable {
                tags,
                variables,
                range: place.byte_range(),
            };
            if !found.contains(&entry) {
                found.push(entry);
            }
        }
        found.sort_by_key(|entry| (entry.range.start, entry.range.end));
        found
    }

    /// The places of `text` that carry at least one of `tags`.
    pub fn runnables_tagged(&self, text: &str, tags: &[String]) -> Vec<Runnable> {
        self.runnables(text)
            .into_iter()
            .filter(|place| place.tags.iter().any(|tag| tags.contains(tag)))
            .collect()
    }
}
