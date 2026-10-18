//! Brackets coloured by how deeply they are nested: each bracket that pairs with another, with the number of pairs that enclose it.

use std::fmt;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use crate::document::Document;

/// The most bytes of a text whose brackets are worked out, beyond which none are coloured.
const MOST_BYTES: usize = 2 * 1024 * 1024;

/// The brackets of a text with their depth, in order.
type Depths = Arc<Vec<(Range<usize>, usize)>>;

/// The brackets worked out last and the parse they came from, shared by the copies of a document so a frame does not work them out again.
#[derive(Clone, Default)]
pub(crate) struct DepthCache(Arc<Mutex<(u64, Depths)>>);

impl fmt::Debug for DepthCache {
    /// Says nothing of what is held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DepthCache")
    }
}

impl PartialEq for DepthCache {
    /// What is held is worked out from the rest of the document, so it never tells two documents apart.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Document {
    /// The brackets of the whole text with their depth, worked out again only when the text was parsed again.
    fn bracket_depths(&self) -> Depths {
        let Some(syntax) = &self.syntax else {
            return Depths::default();
        };
        let Ok(mut held) = self.depths.0.lock() else {
            return Depths::default();
        };
        if held.0 != syntax.revision() {
            let text = self.field.text();
            let found = if text.len() <= MOST_BYTES {
                syntax.bracket_depths(text)
            } else {
                Vec::new()
            };
            *held = (syntax.revision(), Arc::new(found));
        }
        held.1.clone()
    }

    /// The brackets of line `row` that pair with another, as offsets into the line, each with the number of pairs that enclose it.
    pub fn brackets_on(&self, row: usize) -> Vec<(Range<usize>, usize)> {
        let line = self.line(row);
        let all = self.bracket_depths();
        let first = all.partition_point(|(range, _)| range.start < line.start);
        all[first..]
            .iter()
            .take_while(|(range, _)| range.end <= line.end)
            .map(|(range, depth)| (range.start - line.start..range.end - line.start, *depth))
            .collect()
    }
}
