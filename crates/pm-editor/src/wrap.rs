//! Soft wrap: a line longer than a row holds broken onto the rows under it, at a space where there is one, and the rows the lines that show then take.

use std::ops::Range;
use std::sync::Arc;

use crate::document::Document;

/// The bytes of `text`, a line, that each row it is broken into holds when a row has room for `columns` characters, a tab taking `tab` of them; a break falls after the last space that fits, or where the row is full when none does.
pub fn segments(text: &str, columns: usize, tab: usize) -> Vec<Range<usize>> {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    let mut start = 0;
    let mut used = 0;
    let mut space: Option<usize> = None;
    for (at, character) in text.char_indices() {
        let width = if character == '\t' { tab.max(1) } else { 1 };
        if used + width > columns && at > start {
            let end = space.filter(|space| *space > start).unwrap_or(at);
            rows.push(start..end);
            used = text[end..at]
                .chars()
                .map(|held| if held == '\t' { tab.max(1) } else { 1 })
                .sum();
            start = end;
            space = None;
        }
        used += width;
        if character == ' ' || character == '\t' {
            space = Some(at + character.len_utf8());
        }
    }
    rows.push(start..text.len());
    rows
}

/// How many characters a row holds when a file whose lines do not wrap still has rows above some of its lines, which the rows of wrapped lines are counted for.
const UNWRAPPED: usize = usize::MAX / 4;

/// The rows the lines of a text take as they wrap, kept from one call to the next: how many each line's text breaks into, and how many rows come before each line once the folded lines are left out and the rows of links above lines added.
#[derive(Debug, Default)]
pub(crate) struct WrapIndex {
    /// The revision of the text the rows are of.
    revision: u64,
    /// How many characters a row holds.
    columns: usize,
    /// How many of them a tab takes.
    tab: usize,
    /// How many rows each line's text breaks into.
    counts: Vec<u32>,
    /// The lines folded away the rows before each line leave out.
    hidden: Arc<Vec<Range<usize>>>,
    /// The lines with a row of links above them, in order.
    lenses: Vec<usize>,
    /// How many rows come before each line, with one more for the end of the text.
    before: Arc<Vec<usize>>,
}

/// How many rows `text`, a line, breaks into when a row holds `columns` characters, a tab taking `tab` of them; a line always takes one.
fn count_rows(text: &str, columns: usize, tab: usize) -> u32 {
    if columns >= UNWRAPPED || (text.len() <= columns.max(1) && !text.contains('\t')) {
        return 1;
    }
    segments(text, columns, tab).len() as u32
}

impl Document {
    /// The lines that have a row of links above them, in order.
    fn lens_lines(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self
            .annotations
            .lenses
            .items
            .iter()
            .filter(|lens| lens.offset <= self.field.text().len())
            .map(|lens| self.line_of(lens.offset))
            .collect();
        if !self.editing.run_lenses.is_empty() {
            lines.extend(
                self.runnables()
                    .iter()
                    .filter(|(_, place)| self.is_lens_place(place))
                    .map(|(line, _)| *line),
            );
        }
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// How many rows the text of each line breaks into when a row holds `columns` characters, taken from `kept` for the lines the last edit did not touch when `kept` holds them for the text before it.
    fn row_counts(&self, kept: Option<WrapIndex>, columns: usize, tab: usize) -> Vec<u32> {
        let text = self.field.text();
        let count = |index: usize| count_rows(&text[self.line(index)], columns, tab);
        let revision = self.field.revision();
        match kept {
            Some(kept)
                if kept.columns == columns && kept.tab == tab && kept.revision == revision =>
            {
                kept.counts
            }
            Some(mut kept)
                if kept.columns == columns
                    && kept.tab == tab
                    && self.last_edit.as_ref().is_some_and(|edit| {
                        edit.from == kept.revision
                            && edit.to == revision
                            && edit.old_rows.end <= kept.counts.len()
                    }) =>
            {
                let Some(edit) = &self.last_edit else {
                    return kept.counts;
                };
                kept.counts
                    .splice(edit.old_rows.clone(), edit.new_rows.clone().map(count));
                kept.counts
            }
            _ => (0..self.lines()).map(count).collect(),
        }
    }

    /// How many rows come before each line, with one more for the end of the text, when a row holds `columns` characters, worked out again only for what changed since it was last asked for.
    fn rows_before(&self, columns: usize) -> Arc<Vec<usize>> {
        let tab = self.editing.indent;
        let hidden = self.hidden();
        let lenses = self.lens_lines();
        let mut kept = self
            .wraps
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let revision = self.field.revision();
        if let Some(held) = kept.as_ref()
            && held.revision == revision
            && held.columns == columns
            && held.tab == tab
            && held.hidden == hidden
            && held.lenses == lenses
        {
            return held.before.clone();
        }
        let counts = self.row_counts(kept.take(), columns, tab);
        let mut before = Vec::with_capacity(counts.len() + 1);
        let mut row = 0;
        let mut gaps = hidden.iter().peekable();
        let mut marked = lenses.iter().peekable();
        for (index, rows) in counts.iter().enumerate() {
            before.push(row);
            while gaps.peek().is_some_and(|gap| gap.end <= index) {
                gaps.next();
            }
            if gaps.peek().is_some_and(|gap| gap.start <= index) {
                continue;
            }
            while marked.peek().is_some_and(|line| **line < index) {
                marked.next();
            }
            let lens = usize::from(marked.peek().is_some_and(|line| **line == index));
            row += *rows as usize + lens;
        }
        before.push(row);
        let before = Arc::new(before);
        *kept = Some(WrapIndex {
            revision,
            columns,
            tab,
            counts,
            hidden,
            lenses,
            before: before.clone(),
        });
        before
    }

    /// `columns`, or room for any line when lines do not wrap but the language puts links on rows of their own above some lines.
    fn with_lens_rows(&self, columns: Option<usize>) -> Option<usize> {
        match columns {
            None if self.has_lens_rows() => Some(UNWRAPPED),
            other => other,
        }
    }

    /// The bytes of the text each row of the line at `index` holds, from the start of the line, when a row has room for `columns` characters, or the whole line when lines do not wrap.
    ///
    /// A line that has links above it starts with an empty row of its own, which holds them.
    pub fn line_segments(&self, index: usize, columns: Option<usize>) -> Vec<Range<usize>> {
        let columns = self.with_lens_rows(columns);
        let line = self.line(index);
        let text = &self.field.text()[line.clone()];
        let mut rows: Vec<Range<usize>> = match columns {
            Some(columns) => segments(text, columns, self.editing.indent),
            None => std::iter::once(0..text.len()).collect(),
        };
        if self.lens_row_at(index) {
            rows.insert(0, 0..0);
        }
        rows
    }

    /// How many rows the line at `index` takes when a row has room for `columns` characters.
    pub fn line_rows(&self, index: usize, columns: Option<usize>) -> usize {
        let columns = self.with_lens_rows(columns);
        match columns {
            Some(_) => self.line_segments(index, columns).len(),
            None => 1,
        }
    }

    /// How many rows the lines that show take when a row has room for `columns` characters.
    pub fn wrapped_rows(&self, columns: Option<usize>) -> usize {
        let columns = self.with_lens_rows(columns);
        match columns {
            Some(columns) => self.rows_before(columns).last().copied().unwrap_or(0),
            None => self.rows(),
        }
    }

    /// How many rows come before each line, with one more for the end of the text, when a row has room for `columns` characters or some lines have a row of links above them, a hidden line taking none; `None` when every line that shows takes one row, so that a line's row is its place among those that show.
    pub fn row_starts(&self, columns: Option<usize>) -> Option<Arc<Vec<usize>>> {
        self.with_lens_rows(columns)
            .map(|columns| self.rows_before(columns))
    }

    /// The first row the line at `index` is drawn in when a row has room for `columns` characters; a hidden line is drawn in the rows of the line that stands for it.
    pub fn wrapped_row_of_line(&self, index: usize, columns: Option<usize>) -> usize {
        let Some(columns) = self.with_lens_rows(columns) else {
            return self.row_of_line(index);
        };
        let standing = self.standing_for(index);
        let before = self.rows_before(columns);
        before.get(standing).or(before.last()).copied().unwrap_or(0)
    }

    /// The line that shows standing for the line at `index`: itself when it shows, or the line its fold hangs from.
    fn standing_for(&self, index: usize) -> usize {
        self.hidden()
            .iter()
            .find(|lines| lines.contains(&index))
            .map_or(index, |lines| lines.start.saturating_sub(1))
    }

    /// The row the caret is drawn in when a row has room for `columns` characters.
    pub fn caret_row(&self, columns: Option<usize>) -> usize {
        let columns = self.with_lens_rows(columns);
        let caret = self.field.caret();
        let index = self.line_of(caret);
        let within = caret - self.line(index).start;
        let segments = self.line_segments(index, columns);
        let last = segments.len().saturating_sub(1);
        let segment = segments
            .iter()
            .position(|segment| within < segment.end)
            .unwrap_or(last);
        self.wrapped_row_of_line(index, columns) + segment
    }

    /// The lines that show from the one drawn in row `first` until the rows they take reach `count` past it, when a row has room for `columns` characters, with the row the first of them starts in.
    pub fn wrapped_lines(
        &self,
        first: usize,
        count: usize,
        columns: Option<usize>,
    ) -> (Vec<usize>, usize) {
        let Some(columns) = self.with_lens_rows(columns) else {
            return (self.shown_lines(first, count), first.min(self.rows()));
        };
        let before = self.rows_before(columns);
        let total = self.lines();
        let mut index = before[1..=total].partition_point(|end| *end <= first);
        let mut row = before[index.min(total)];
        let start = row;
        let mut lines = Vec::new();
        while index < total {
            let rows = before[index + 1] - before[index];
            index += 1;
            if rows == 0 {
                continue;
            }
            lines.push(index - 1);
            row += rows;
            if row >= first + count {
                break;
            }
        }
        let start = if lines.is_empty() { row } else { start };
        (lines, start)
    }
}
