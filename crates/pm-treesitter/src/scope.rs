//! The scope of the language the caret is in: which named set of syntax nodes holds it, whether the language lists the nodes or its `overrides.scm` captures them.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::grammar::Scope;
use crate::syntax::Syntax;

impl Syntax {
    /// The scope of the innermost piece of syntax of any scope the byte `offset` of `text` is inside; none when no scope holds it.
    ///
    /// A listed node holds the offset when it is after its start and before its end. A piece the overrides query captures does too, unless the capture is named `<scope>.inclusive`, which holds it at the start and the end as well.
    pub fn scope_at(&self, text: &str, offset: usize) -> Option<&Scope> {
        let scopes = &self.grammar().editing().scopes;
        if scopes.is_empty() {
            return None;
        }
        let listed = self.listed_scope(offset);
        let captured = self.captured_scope(text, offset);
        let chosen = match (listed, captured) {
            (Some(own), Some(other)) => {
                if other.1.len() < own.1.len() {
                    other
                } else {
                    own
                }
            }
            (own, other) => own.or(other)?,
        };
        scopes.iter().find(|scope| scope.name == chosen.0)
    }

    /// The name of the innermost scope whose listed nodes hold `offset`, with the bytes of the node.
    fn listed_scope(&self, offset: usize) -> Option<(String, Range<usize>)> {
        let scopes = &self.grammar().editing().scopes;
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset, offset)?;
        loop {
            if node.start_byte() < offset && offset < node.end_byte() {
                let kind = node.kind();
                if let Some(scope) = scopes
                    .iter()
                    .find(|scope| scope.nodes.iter().any(|held| held == kind))
                {
                    return Some((scope.name.clone(), node.byte_range()));
                }
            }
            node = node.parent()?;
        }
    }

    /// The name of the innermost scope the overrides query captures around `offset` of `text`, with the bytes captured.
    fn captured_scope(&self, text: &str, offset: usize) -> Option<(String, Range<usize>)> {
        let query = self.grammar().overrides.as_ref()?;
        let names = query.capture_names();
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(offset.saturating_sub(1)..(offset + 1).min(text.len()));
        let mut best: Option<(String, Range<usize>)> = None;
        let mut captures = cursor.captures(query, self.tree.root_node(), text.as_bytes());
        while let Some((held, index)) = captures.next() {
            let capture = held.captures()[*index];
            let name = names[capture.index as usize];
            let (scope, inclusive) = match name.strip_suffix(".inclusive") {
                Some(scope) => (scope, true),
                None => (name, false),
            };
            let range = capture.node.byte_range();
            let inside = if inclusive {
                range.start <= offset && offset <= range.end
            } else {
                range.start < offset && offset < range.end
            };
            if inside
                && best
                    .as_ref()
                    .is_none_or(|(_, held)| range.len() < held.len())
            {
                best = Some((scope.to_owned(), range));
            }
        }
        best
    }
}
