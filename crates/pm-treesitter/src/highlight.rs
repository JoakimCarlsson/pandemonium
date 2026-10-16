//! What colour each stretch of a window of a text is, the stretches written in another language included.

use std::collections::BTreeMap;
use std::ops::Range;
use tree_sitter::{QueryCursor, StreamingIterator, Tree};

use crate::grammar::Grammar;
use crate::kind::Kind;
use crate::registry;
use crate::syntax::{Syntax, parse_text};

/// How many languages deep a stretch inside a stretch is still highlighted.
const DEPTH: usize = 3;

/// A stretch of a text and what it is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Highlight {
    /// The bytes of the text.
    pub range: Range<usize>,
    /// What they are.
    pub kind: Kind,
}

/// The kind of each byte of a window of a text, painted capture by capture.
struct Canvas {
    /// The byte of the text the window starts at.
    start: usize,
    /// The kind of each byte of the window, or nothing for plain text.
    kinds: Vec<Option<Kind>>,
}

/// The name of the language a stretch is written in, as a code fence's info string or a capture gives it: its first word.
fn language_name(raw: &str) -> String {
    raw.split(|c: char| c.is_whitespace() || c == ',' || c == '{')
        .next()
        .unwrap_or_default()
        .to_owned()
}

impl Canvas {
    /// A window over the bytes `window` of a text, all plain.
    fn new(window: &Range<usize>) -> Self {
        Self {
            start: window.start,
            kinds: vec![None; window.len()],
        }
    }

    /// The bytes of the text the window covers.
    fn window(&self) -> Range<usize> {
        self.start..self.start + self.kinds.len()
    }

    /// Paints the bytes `range` of the text as `kind`, as far as they fall in the window.
    fn paint(&mut self, range: Range<usize>, kind: Kind) {
        let low = range.start.max(self.start) - self.start;
        let high = range
            .end
            .min(self.start + self.kinds.len())
            .saturating_sub(self.start);
        for held in self.kinds.iter_mut().take(high).skip(low) {
            *held = Some(kind);
        }
    }

    /// Paints what the highlights query of `grammar` captures in `tree`, a tree of `text`: wider captures first so narrower ones show over them, and of two captures of one stretch the one whose pattern comes first in the query last, so it shows.
    fn paint_tree(&mut self, grammar: &Grammar, tree: &Tree, text: &str, depth: usize) {
        if let Some(query) = &grammar.highlights {
            let mut cursor = QueryCursor::new();
            cursor.set_byte_range(self.window());
            let mut found: Vec<(Range<usize>, Kind, usize)> = Vec::new();
            let mut captures = cursor.captures(query, tree.root_node(), text.as_bytes());
            while let Some((found_match, index)) = captures.next() {
                let capture = found_match.captures()[*index];
                if let Some(kind) = grammar.kinds.get(capture.index as usize).copied().flatten() {
                    found.push((capture.node.byte_range(), kind, found_match.pattern_index));
                }
            }
            found
                .sort_by(|left, right| right.0.len().cmp(&left.0.len()).then(right.2.cmp(&left.2)));
            for (range, kind, _) in found {
                self.paint(range, kind);
            }
        }
        if depth < DEPTH {
            self.paint_injections(grammar, tree, text, depth);
        }
    }

    /// Paints the stretches of `tree`, a tree of `text` parsed with `grammar`, that its injections query says are written in another language, in that language's colours, when its grammar is loaded.
    fn paint_injections(&mut self, grammar: &Grammar, tree: &Tree, text: &str, depth: usize) {
        let Some(query) = &grammar.injections else {
            return;
        };
        let content = query.capture_index_for_name("injection.content");
        let language = query.capture_index_for_name("injection.language");
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(self.window());
        let mut stretches: Vec<(String, Vec<tree_sitter::Range>)> = Vec::new();
        let mut combined: BTreeMap<String, Vec<tree_sitter::Range>> = BTreeMap::new();
        let mut matches = cursor.matches(query, tree.root_node(), text.as_bytes());
        while let Some(found) = matches.next() {
            let settings = query.property_settings(found.pattern_index);
            let setting = |key: &str| {
                settings
                    .iter()
                    .find(|property| &*property.key == key)
                    .map(|property| property.value.as_deref().unwrap_or_default().to_owned())
            };
            let mut name = setting("injection.language");
            let mut ranges = Vec::new();
            for capture in found.captures() {
                if Some(capture.index) == language {
                    name = Some(text[capture.node.byte_range()].to_owned());
                } else if Some(capture.index) == content {
                    ranges.push(capture.node.range());
                }
            }
            let Some(name) = name.map(|name| language_name(&name)) else {
                continue;
            };
            if ranges.is_empty() {
                continue;
            }
            if setting("injection.combined").is_some() {
                combined.entry(name).or_default().extend(ranges);
            } else {
                stretches.push((name, ranges));
            }
        }
        stretches.extend(combined);
        for (name, mut ranges) in stretches {
            let Some(inner) = registry::find(&name) else {
                continue;
            };
            ranges.sort_by_key(|range| range.start_byte);
            ranges.dedup_by_key(|range| range.start_byte);
            if let Some(tree) = parse_text(&inner, text, &ranges, None) {
                self.paint_tree(&inner, &tree, text, depth + 1);
            }
        }
    }

    /// The painted stretches, each run of one kind as one highlight.
    fn highlights(&self) -> Vec<Highlight> {
        let mut found: Vec<Highlight> = Vec::new();
        for (index, kind) in self.kinds.iter().enumerate() {
            let Some(kind) = *kind else {
                continue;
            };
            let at = self.start + index;
            match found.last_mut() {
                Some(last) if last.kind == kind && last.range.end == at => last.range.end = at + 1,
                _ => found.push(Highlight {
                    range: at..at + 1,
                    kind,
                }),
            }
        }
        found
    }
}

impl Syntax {
    /// What colour each stretch of the bytes `window` of `text` is, in order; bytes of plain text are left out.
    pub fn highlights(&self, text: &str, window: Range<usize>) -> Vec<Highlight> {
        let window = window.start.min(text.len())..window.end.min(text.len());
        let mut canvas = Canvas::new(&window);
        canvas.paint_tree(self.grammar(), &self.tree, text, 0);
        canvas.highlights()
    }
}
