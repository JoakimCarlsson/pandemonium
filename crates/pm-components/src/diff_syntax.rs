//! The syntax colours of the code in a diff: each side read as one text by
//! the grammar of the file's language, so what spans lines is coloured on
//! every line it covers, with the colours worked out once per diff and kept.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use pm_treesitter::{Grammar, Kind, Syntax};

use crate::diff_view::DiffRow;

/// How many diffs keep their colours before all are worked out afresh.
const KEPT: usize = 64;

/// The highlighted stretches of a line, as byte ranges into it.
pub(crate) type LineSpans = Vec<(Range<usize>, Kind)>;

/// The highlighted stretches of each row's old and new side.
pub(crate) type RowSpans = Rc<Vec<(LineSpans, LineSpans)>>;

thread_local! {
    /// The colours of the diffs coloured lately, by a fingerprint of the grammar and the rows.
    static PAINTED: RefCell<HashMap<u64, RowSpans>> = RefCell::new(HashMap::new());
}

/// The syntax colours of each of `rows`, read with `grammar`; every row plain when there is none.
pub(crate) fn colour_rows(rows: &[DiffRow], grammar: Option<&Arc<Grammar>>) -> RowSpans {
    let Some(grammar) = grammar else {
        return Rc::new(vec![Default::default(); rows.len()]);
    };
    let key = fingerprint(grammar.name(), rows);
    if let Some(painted) = PAINTED.with(|painted| painted.borrow().get(&key).cloned()) {
        return painted;
    }
    let left = side_spans(rows, grammar, |row| row.left.as_ref());
    let right = side_spans(rows, grammar, |row| row.right.as_ref());
    let painted: RowSpans = Rc::new(left.into_iter().zip(right).collect());
    PAINTED.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= KEPT {
            kept.clear();
        }
        kept.insert(key, painted.clone());
    });
    painted
}

/// The highlighted stretches of the side of each of `rows` that `side` picks, read as one text with `grammar`.
fn side_spans(
    rows: &[DiffRow],
    grammar: &Arc<Grammar>,
    side: impl Fn(&DiffRow) -> Option<&(usize, String)>,
) -> Vec<LineSpans> {
    let mut text = String::new();
    let mut starts: Vec<Option<Range<usize>>> = Vec::with_capacity(rows.len());
    for row in rows {
        match side(row) {
            Some((_, line)) => {
                let start = text.len();
                text.push_str(line);
                starts.push(Some(start..text.len()));
                text.push('\n');
            }
            None => starts.push(None),
        }
    }
    let mut spans = vec![LineSpans::new(); rows.len()];
    let Some(syntax) = Syntax::parse(grammar.clone(), &text) else {
        return spans;
    };
    let lines: Vec<(usize, Range<usize>)> = starts
        .into_iter()
        .enumerate()
        .filter_map(|(row, range)| range.map(|range| (row, range)))
        .collect();
    for highlight in syntax.highlights(&text, 0..text.len()) {
        let first = lines.partition_point(|(_, range)| range.end <= highlight.range.start);
        for (row, range) in lines.iter().skip(first) {
            if range.start >= highlight.range.end {
                break;
            }
            let start = highlight.range.start.max(range.start) - range.start;
            let end = highlight.range.end.min(range.end) - range.start;
            if start < end {
                spans[*row].push((start..end, highlight.kind));
            }
        }
    }
    spans
}

/// A fingerprint of the grammar called `name` and the text of `rows`.
fn fingerprint(name: &str, rows: &[DiffRow]) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    for row in rows {
        row.left.hash(&mut hasher);
        row.right.hash(&mut hasher);
    }
    hasher.finish()
}
