//! What the syntax tree says about where functions, types and comments
//! are, for the motions and objects that go by them.
//!
//! Grammars name their nodes differently — `function_item`,
//! `function_definition`, `method_declaration` — so a node is taken by what
//! its name says it is rather than by a list per language. A buffer with no
//! grammar has no nodes, and the section motions fall back on the braces in
//! the first column that vim itself goes by.

use pm_text::{Buffer, Position, SyntaxNode};

use crate::text;

/// Whether a node called `kind` is a function or a method.
pub(crate) fn is_function(kind: &str) -> bool {
    let named = kind.contains("function") || kind.contains("method") || kind.contains("lambda");
    named && !kind.contains("call") && !kind.contains("type") && !kind.contains("signature")
}

/// Whether a node called `kind` is a class, a struct or something like one.
pub(crate) fn is_class(kind: &str) -> bool {
    let named = [
        "class",
        "struct",
        "interface",
        "trait",
        "enum",
        "impl_item",
        "object_declaration",
    ]
    .iter()
    .any(|word| kind.contains(word));
    named && !kind.ends_with("body") && !kind.contains("field") && !kind.contains("specifier")
}

/// Whether a node called `kind` is a comment.
pub(crate) fn is_comment(kind: &str) -> bool {
    kind.contains("comment")
}

/// The innermost node around `at` that `keep` accepts.
pub(crate) fn enclosing(
    buffer: &Buffer,
    at: Position,
    keep: &dyn Fn(&str) -> bool,
) -> Option<SyntaxNode> {
    buffer
        .syntax_nodes(keep)
        .into_iter()
        .filter(|node| node.range.start <= at && at < node.range.end)
        .min_by_key(|node| buffer.char_of(node.range.end) - buffer.char_of(node.range.start))
}

/// Where each function starts, or ends when `end`, in the order they come.
pub(crate) fn functions(buffer: &Buffer, end: bool) -> Vec<Position> {
    places(buffer, buffer.syntax_nodes(&is_function), end)
}

/// Where each comment starts, in the order they come.
pub(crate) fn comments(buffer: &Buffer) -> Vec<Position> {
    places(buffer, buffer.syntax_nodes(&is_comment), false)
}

/// Where each top-level function or type starts, or ends when `end`.
///
/// Without a tree, a section starts at a `{` in the first column and ends
/// at a `}` there, which is what vim's own `]]` looks for.
pub(crate) fn sections(buffer: &Buffer, end: bool) -> Vec<Position> {
    let nodes = buffer.syntax_nodes(&|kind| is_function(kind) || is_class(kind));
    if nodes.is_empty() {
        let brace = if end { '}' } else { '{' };
        return (0..buffer.line_count())
            .filter(|line| buffer.char_at(Position::new(*line, 0)) == Some(brace))
            .map(|line| Position::new(line, 0))
            .collect();
    }
    let outermost = nodes
        .iter()
        .filter(|node| {
            !nodes.iter().any(|other| {
                other != *node
                    && other.range.start <= node.range.start
                    && node.range.end <= other.range.end
            })
        })
        .cloned()
        .collect();
    places(buffer, outermost, end)
}

/// Where each of `nodes` starts, or ends on its last character, sorted.
fn places(buffer: &Buffer, nodes: Vec<SyntaxNode>, end: bool) -> Vec<Position> {
    let mut places = nodes
        .into_iter()
        .map(|node| match end {
            true => text::before(buffer, node.range.end),
            false => node.range.start,
        })
        .collect::<Vec<_>>();
    places.sort();
    places.dedup();
    places
}
