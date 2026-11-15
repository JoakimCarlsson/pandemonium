//! The rows a code editor draws its lines in: one per line that shows, past
//! the blocks folded away, or several per line when long lines wrap at a
//! width, each holding a stretch of its line.

use std::ops::Range;

use pm_editor::{Document, Fold};

use crate::code_text::TAB_COLUMNS;

/// Which line and which stretch of it each row of an editor draws.
#[derive(Clone, Debug, Default)]
pub(crate) struct RowMap {
    /// Each row as its line and the bytes of the line it holds, or nothing
    /// while every line shows whole in a row of its own.
    rows: Option<Vec<(usize, Range<usize>)>>,
    /// What the rows were worked out from: the text's revision, the folds
    /// and the columns a row holds, so they are worked out again only when
    /// one of them changes.
    from: Option<(u64, Vec<Fold>, Option<usize>)>,
}

impl RowMap {
    /// Works the rows out again for `document` with a row holding `columns`
    /// characters, or whole lines when nothing, unless they already are.
    pub(crate) fn update(&mut self, document: &Document, columns: Option<usize>) {
        let from = (document.field.revision(), document.folds.clone(), columns);
        if self.from.as_ref() == Some(&from) {
            return;
        }
        self.rows = if document.folds.is_empty() && columns.is_none() {
            None
        } else {
            Some(rows_of(document, columns))
        };
        self.from = Some(from);
    }

    /// Whether some line takes more than one row.
    pub(crate) fn wraps(&self) -> bool {
        self.from
            .as_ref()
            .is_some_and(|(_, _, columns)| columns.is_some())
    }

    /// How many rows there are for `document`.
    pub(crate) fn len(&self, document: &Document) -> usize {
        match &self.rows {
            Some(rows) => rows.len().max(1),
            None => document.lines().max(1),
        }
    }

    /// The line row `row` of `document` draws and the bytes of the line it
    /// holds, kept inside the text.
    pub(crate) fn at(&self, document: &Document, row: usize) -> (usize, Range<usize>) {
        let whole = |line: usize| {
            let range = document.line(line);
            (line, 0..range.len())
        };
        match &self.rows {
            Some(rows) => rows
                .get(row)
                .or(rows.last())
                .cloned()
                .unwrap_or_else(|| whole(0)),
            None => whole(row.min(document.lines().saturating_sub(1))),
        }
    }

    /// The row the byte `offset` of the text of `document` is drawn in; a
    /// byte folded away is drawn in the row of the line its fold hangs from.
    pub(crate) fn row_of(&self, document: &Document, offset: usize) -> usize {
        let line = document.line_of(offset);
        let Some(rows) = &self.rows else {
            return line;
        };
        let within = offset - document.line(line).start;
        rows.partition_point(|(held, segment)| (*held, segment.start) <= (line, within))
            .saturating_sub(1)
    }

    /// The first row of the line at `index` of `document`.
    pub(crate) fn row_of_line(&self, document: &Document, index: usize) -> usize {
        self.row_of(document, document.line(index).start)
    }
}

/// Each row of `document` as its line and the bytes of the line it holds,
/// with a row holding `columns` characters, or whole lines when nothing.
fn rows_of(document: &Document, columns: Option<usize>) -> Vec<(usize, Range<usize>)> {
    let text = document.field.text();
    let mut rows = Vec::new();
    for line in document.shown_lines(0, document.lines()) {
        let range = document.line(line);
        let held = &text[range.clone()];
        match columns {
            Some(columns) => rows.extend(
                pm_editor::segments(held, columns, TAB_COLUMNS)
                    .into_iter()
                    .map(|segment| (line, segment)),
            ),
            None => rows.push((line, 0..held.len())),
        }
    }
    rows
}
