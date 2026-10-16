//! One text parsed with a grammar, kept up to date edit by edit.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use pm_buffer::{Change, Lines};
use tree_sitter::{InputEdit, Point, Range, Tree};

use crate::engine::with_parser;
use crate::grammar::Grammar;

/// The number the next tree parsed gets, so that two parses are never mistaken for one.
static REVISIONS: AtomicU64 = AtomicU64::new(1);

/// A text parsed with a grammar.
#[derive(Clone)]
pub struct Syntax {
    /// The grammar it was parsed with.
    grammar: Arc<Grammar>,
    /// The tree of the text as it is now.
    pub(crate) tree: Tree,
    /// Which parse the tree came from.
    revision: u64,
}

impl fmt::Debug for Syntax {
    /// Names the grammar and the parse.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Syntax")
            .field("grammar", &self.grammar.name())
            .field("revision", &self.revision)
            .finish()
    }
}

impl PartialEq for Syntax {
    /// Two syntaxes are the same when they came from the same parse.
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision
    }
}

/// The byte `offset` as a point of the tree, by the index `lines` of the text it is in.
fn point(lines: &Lines, offset: usize) -> Point {
    let (row, column) = lines.point_of(offset);
    Point::new(row, column)
}

/// Parses `text` with `grammar`, only the stretches `ranges` of it when there are any, starting from `old`, the tree before the last edit.
pub(crate) fn parse_text(
    grammar: &Grammar,
    text: &str,
    ranges: &[Range],
    old: Option<&Tree>,
) -> Option<Tree> {
    with_parser(|parser| {
        parser.set_language(&grammar.language).ok()?;
        parser.set_included_ranges(ranges).ok()?;
        let tree = parser.parse(text, old);
        let _ = parser.set_included_ranges(&[]);
        tree
    })
    .ok()
    .flatten()
}

impl Syntax {
    /// `text` parsed with `grammar`, or nothing when the parser gave up on it.
    pub fn parse(grammar: Arc<Grammar>, text: &str) -> Option<Self> {
        let tree = parse_text(&grammar, text, &[], None)?;
        Some(Self {
            grammar,
            tree,
            revision: REVISIONS.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// The grammar the text was parsed with.
    pub fn grammar(&self) -> &Arc<Grammar> {
        &self.grammar
    }

    /// Which parse the tree came from; it changes with every edit.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Takes in `change`, which turned a text whose lines `before` indexes into `after`, whose lines `after_lines` indexes, and parses again from the tree as it was.
    pub fn edit(&mut self, change: &Change, before: &Lines, after: &str, after_lines: &Lines) {
        self.tree.edit(&InputEdit {
            start_byte: change.range.start,
            old_end_byte: change.range.end,
            new_end_byte: change.new_end(),
            start_position: point(before, change.range.start),
            old_end_position: point(before, change.range.end),
            new_end_position: point(after_lines, change.new_end()),
        });
        let tree = parse_text(&self.grammar, after, &[], Some(&self.tree))
            .or_else(|| parse_text(&self.grammar, after, &[], None));
        if let Some(tree) = tree {
            self.tree = tree;
        }
        self.revision = REVISIONS.fetch_add(1, Ordering::Relaxed);
    }
}
