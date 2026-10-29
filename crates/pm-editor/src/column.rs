//! Selecting a rectangle of text: a caret or a selection on each line from one column to another, grown with the keys or dragged out with the pointer.

use pm_text::Edit;

use crate::document::Document;

/// The rectangle being selected by the keys: where it started and where it reaches, as a line and a column of characters each, and the selections it made, which tell whether it is still the one on screen.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColumnBox {
    /// The line and column the rectangle started at.
    anchor: (usize, usize),
    /// The line and column the rectangle reaches, which may be past the end of a short line.
    active: (usize, usize),
    /// The selections the rectangle made, in the order they come in the text.
    made: Vec<(usize, usize)>,
}

impl Document {
    /// The line and the column, counted in characters, of the byte `offset`.
    fn column_of(&self, offset: usize) -> (usize, usize) {
        let line = self.line_of(offset);
        let start = self.line(line).start;
        (line, self.field.text()[start..offset].chars().count())
    }

    /// The byte of line `line` that is `column` characters in, or its end when it is shorter.
    fn offset_at(&self, line: usize, column: usize) -> usize {
        let range = self.line(line);
        self.field.text()[range.clone()]
            .char_indices()
            .nth(column)
            .map_or(range.end, |(at, _)| range.start + at)
    }

    /// The edits that select the rectangle between the places `anchor` and `active`, a selection on each line that is as wide as the line allows, and the selections they make.
    fn rectangle(
        &self,
        anchor: (usize, usize),
        active: (usize, usize),
    ) -> (Vec<Edit>, Vec<(usize, usize)>) {
        let (left, right) = (anchor.1.min(active.1), anchor.1.max(active.1));
        let lines: Vec<usize> = if anchor.0 <= active.0 {
            (anchor.0..=active.0).collect()
        } else {
            (active.0..=anchor.0).rev().collect()
        };
        let selections: Vec<(usize, usize)> = lines
            .into_iter()
            .map(|line| {
                let (start, end) = (self.offset_at(line, left), self.offset_at(line, right));
                if active.1 >= anchor.1 {
                    (start, end)
                } else {
                    (end, start)
                }
            })
            .collect();
        let edits = selections
            .iter()
            .enumerate()
            .map(|(at, (from, to))| {
                if at == 0 {
                    Edit::Select(*from, *to)
                } else {
                    Edit::AddSelection(*from, *to)
                }
            })
            .collect();
        let mut made = selections;
        made.sort_by_key(|(from, to)| (*from).min(*to));
        (edits, made)
    }

    /// The edits that grow the rectangle being selected by `down` lines and `right` columns, started from the selection the caret is in when none is being grown; a rectangle does not grow past the widest of its lines to the right.
    pub fn column_select_edits(&mut self, down: isize, right: isize) -> Vec<Edit> {
        let current = self.field.selections();
        let (anchor, mut active) = match &self.column {
            Some(held) if held.made == current => (held.anchor, held.active),
            _ => (
                self.column_of(self.field.anchor()),
                self.column_of(self.field.caret()),
            ),
        };
        let last = self.lines().saturating_sub(1);
        active.0 = active.0.saturating_add_signed(down).min(last);
        let moved = active.1.saturating_add_signed(right);
        active.1 = if right > 0 {
            let widest = (anchor.0.min(active.0)..=anchor.0.max(active.0))
                .map(|line| self.field.text()[self.line(line)].chars().count())
                .max()
                .unwrap_or(0);
            moved.min(widest.max(active.1))
        } else {
            moved
        };
        let (edits, made) = self.rectangle(anchor, active);
        self.column = Some(ColumnBox {
            anchor,
            active,
            made,
        });
        edits
    }

    /// The edits that select the rectangle with corners at the bytes `from` and `to`, as a drag of the pointer makes it.
    pub fn column_drag_edits(&self, from: usize, to: usize) -> Vec<Edit> {
        self.rectangle(self.column_of(from), self.column_of(to)).0
    }
}
