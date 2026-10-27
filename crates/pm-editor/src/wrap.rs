//! Soft wrap: a line longer than a row holds broken onto the rows under it, at a space where there is one, and the rows the lines that show then take.

use std::ops::Range;

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

impl Document {
    /// The bytes of the text each row of the line at `index` holds, from the start of the line, when a row has room for `columns` characters, or the whole line when lines do not wrap.
    pub fn line_segments(&self, index: usize, columns: Option<usize>) -> Vec<Range<usize>> {
        let line = self.line(index);
        let text = &self.field.text()[line.clone()];
        match columns {
            Some(columns) => segments(text, columns, self.editing.indent),
            None => std::iter::once(0..text.len()).collect(),
        }
    }

    /// How many rows the line at `index` takes when a row has room for `columns` characters, the row its lenses stand in above it included.
    pub fn line_rows(&self, index: usize, columns: Option<usize>) -> usize {
        let text = match columns {
            Some(_) => self.line_segments(index, columns).len(),
            None => 1,
        };
        text + usize::from(self.has_lens_row(index))
    }

    /// The lines that show, by index, in order.
    fn every_shown_line(&self) -> Vec<usize> {
        self.shown_lines(0, self.lines())
    }

    /// How many rows the lines that show take when a row has room for `columns` characters.
    pub fn wrapped_rows(&self, columns: Option<usize>) -> usize {
        if columns.is_none() && !self.shows_lens_rows() {
            return self.rows();
        }
        self.every_shown_line()
            .into_iter()
            .map(|index| self.line_rows(index, columns))
            .sum()
    }

    /// The first row the line at `index` is drawn in when a row has room for `columns` characters; a hidden line is drawn in the rows of the line that stands for it.
    pub fn wrapped_row_of_line(&self, index: usize, columns: Option<usize>) -> usize {
        if columns.is_none() && !self.shows_lens_rows() {
            return self.row_of_line(index);
        }
        let standing = self.standing_for(index);
        self.every_shown_line()
            .into_iter()
            .take_while(|shown| *shown < standing)
            .map(|shown| self.line_rows(shown, columns))
            .sum()
    }

    /// The line that shows standing for the line at `index`: itself when it shows, or the line its fold hangs from.
    fn standing_for(&self, index: usize) -> usize {
        self.hidden_lines()
            .into_iter()
            .find(|lines| lines.contains(&index))
            .map_or(index, |lines| lines.start.saturating_sub(1))
    }

    /// The row the caret is drawn in when a row has room for `columns` characters.
    pub fn caret_row(&self, columns: Option<usize>) -> usize {
        let caret = self.field.caret();
        let index = self.line_of(caret);
        let within = caret - self.line(index).start;
        let segments = self.line_segments(index, columns);
        let last = segments.len().saturating_sub(1);
        let segment = segments
            .iter()
            .position(|segment| within < segment.end)
            .unwrap_or(last);
        self.wrapped_row_of_line(index, columns) + usize::from(self.has_lens_row(index)) + segment
    }

    /// The lines that show from the one drawn in row `first` until the rows they take reach `count` past it, when a row has room for `columns` characters, with the row the first of them starts in.
    pub fn wrapped_lines(
        &self,
        first: usize,
        count: usize,
        columns: Option<usize>,
    ) -> (Vec<usize>, usize) {
        if columns.is_none() && !self.shows_lens_rows() {
            return (self.shown_lines(first, count), first.min(self.rows()));
        }
        let mut row = 0;
        let mut start = None;
        let mut lines = Vec::new();
        for index in self.every_shown_line() {
            let rows = self.line_rows(index, columns);
            if row + rows > first {
                start.get_or_insert(row);
                lines.push(index);
            }
            row += rows;
            if row >= first + count {
                break;
            }
        }
        (lines, start.unwrap_or(row))
    }
}
