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

/// One function, class or comment the text objects query found, as the bytes it covers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSpan {
    /// What kind of piece it is.
    pub object: TextObject,
    /// The whole of it.
    pub whole: Range<usize>,
    /// What is inside it, when the query says what is.
    pub inside: Option<Range<usize>>,
}

/// The smallest range that holds both `left` and `right`.
fn joined(left: Range<usize>, right: Range<usize>) -> Range<usize> {
    left.start.min(right.start)..left.end.max(right.end)
}

impl Syntax {
    /// Every function, class and comment of `text` that the text objects query finds, in the order they start; a piece the query finds more than once is listed once, with its insides joined.
    pub fn text_objects(&self, text: &str) -> Vec<ObjectSpan> {
        let Some(query) = self.grammar().textobjects.as_ref() else {
            return Vec::new();
        };
        let kinds = [TextObject::Function, TextObject::Class, TextObject::Comment].map(|object| {
            (
                object,
                query.capture_index_for_name(&format!("{}.around", object.name())),
                query.capture_index_for_name(&format!("{}.inside", object.name())),
            )
        });
        let mut found: Vec<ObjectSpan> = Vec::new();
        let mut places: std::collections::HashMap<(usize, usize, usize), usize> =
            std::collections::HashMap::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), text.as_bytes());
        while let Some(held) = matches.next() {
            for (kind, (object, around, within)) in kinds.into_iter().enumerate() {
                let ranges = |wanted: Option<u32>| {
                    held.captures()
                        .iter()
                        .filter(|capture| Some(capture.index) == wanted)
                        .map(|capture| capture.node.byte_range())
                        .reduce(joined)
                };
                let Some(whole) = ranges(around) else {
                    continue;
                };
                let inside = ranges(within);
                match places.get(&(kind, whole.start, whole.end)) {
                    Some(&index) => {
                        let span = &mut found[index];
                        span.inside = match (span.inside.take(), inside) {
                            (Some(left), Some(right)) => Some(joined(left, right)),
                            (left, right) => left.or(right),
                        };
                    }
                    None => {
                        places.insert((kind, whole.start, whole.end), found.len());
                        found.push(ObjectSpan {
                            object,
                            whole,
                            inside,
                        });
                    }
                }
            }
        }
        found.sort_by(|left, right| {
            left.whole
                .start
                .cmp(&right.whole.start)
                .then(right.whole.end.cmp(&left.whole.end))
        });
        found
    }

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
