//! Functions, classes and comments as wholes, as the text objects query captures them: `@<object>.around` is the whole of one and `@<object>.inside` what is within it.

use std::ops::Range;

use tree_sitter::{QueryCursor, StreamingIterator};

use crate::syntax::Syntax;

/// A kind of piece of a text that the text objects query names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextObject {
    /// A function or a method.
    Function,
    /// A class, a struct or another type with a body.
    Class,
    /// A comment, or the comments that follow one another.
    Comment,
}

impl TextObject {
    /// What its captures are called before `.around` and `.inside`.
    fn name(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Comment => "comment",
        }
    }
}

impl Syntax {
    /// The bytes of the smallest `object` around the byte `offset` of `text`: the whole of it, or what is inside it when `inside` and the query says what is; none when the language has no such object there.
    pub fn text_object(
        &self,
        text: &str,
        object: TextObject,
        inside: bool,
        offset: usize,
    ) -> Option<Range<usize>> {
        let query = self.grammar().textobjects.as_ref()?;
        let around = query.capture_index_for_name(&format!("{}.around", object.name()));
        let within = query.capture_index_for_name(&format!("{}.inside", object.name()));
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(offset.saturating_sub(1)..(offset + 1).min(text.len()));
        let mut best: Option<(Range<usize>, Option<Range<usize>>)> = None;
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            let ranges = |wanted: Option<u32>| -> Vec<Range<usize>> {
                held.captures()
                    .iter()
                    .filter(|capture| Some(capture.index) == wanted)
                    .map(|capture| capture.node.byte_range())
                    .collect()
            };
            let (wholes, parts) = (ranges(around), ranges(within));
            let Some(whole) = wholes
                .into_iter()
                .reduce(|left, right| left.start.min(right.start)..left.end.max(right.end))
            else {
                continue;
            };
            if !(whole.start <= offset && offset <= whole.end) {
                continue;
            }
            let part = parts
                .into_iter()
                .reduce(|left, right| left.start.min(right.start)..left.end.max(right.end));
            match best.as_mut() {
                Some((held, held_part)) if *held == whole => {
                    *held_part = match (held_part.take(), part) {
                        (Some(left), Some(right)) => {
                            Some(left.start.min(right.start)..left.end.max(right.end))
                        }
                        (left, right) => left.or(right),
                    };
                }
                Some((held, _)) if whole.len() >= held.len() => {}
                _ => best = Some((whole, part)),
            }
        }
        best.map(|(whole, part)| if inside { part.unwrap_or(whole) } else { whole })
    }
}
