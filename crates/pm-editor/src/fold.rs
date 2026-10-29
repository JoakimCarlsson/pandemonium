//! Folding: the lines between the first and the last of a block hidden away, the first line standing for them, and the rows the lines that show are drawn in.

use std::ops::Range;

use pm_buffer::Change;

use crate::document::Document;

/// A folded block, as the byte where its first line ends and the byte where its last line starts; the lines between are hidden.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fold {
    /// The end of the first line, which stays in view.
    pub head: usize,
    /// The start of the last line, which stays in view.
    pub tail: usize,
}

/// Where the byte `offset` of the text before `change` is after it, or nothing when the change replaced it.
fn shifted(offset: usize, change: &Change) -> Option<usize> {
    if offset < change.range.start {
        Some(offset)
    } else if offset >= change.range.end {
        Some(offset - change.range.len() + change.text.len())
    } else {
        None
    }
}

impl Document {
    /// The lines each fold hides, by index, in order and not overlapping.
    pub fn hidden_lines(&self) -> Vec<Range<usize>> {
        let mut hidden: Vec<Range<usize>> = self
            .folds
            .iter()
            .map(|fold| self.line_of(fold.head) + 1..self.line_of(fold.tail))
            .filter(|lines| !lines.is_empty())
            .collect();
        hidden.sort_by_key(|lines| lines.start);
        let mut joined: Vec<Range<usize>> = Vec::new();
        for lines in hidden {
            match joined.last_mut() {
                Some(last) if lines.start <= last.end => last.end = last.end.max(lines.end),
                _ => joined.push(lines),
            }
        }
        joined
    }

    /// How many rows the lines that show take.
    pub fn rows(&self) -> usize {
        self.lines()
            - self
                .hidden_lines()
                .iter()
                .map(|lines| lines.len())
                .sum::<usize>()
    }

    /// The row the line at `index` is drawn in; a hidden line is drawn in the row of the line that stands for it.
    pub fn row_of_line(&self, index: usize) -> usize {
        let mut row = index;
        for lines in self.hidden_lines() {
            if index >= lines.end {
                row -= lines.len();
            } else if index >= lines.start {
                return lines.start - 1 - (index - row);
            }
        }
        row
    }

    /// The lines that show, by index, from the one in row `first` until `count` of them.
    pub fn shown_lines(&self, first: usize, count: usize) -> Vec<usize> {
        let hidden = self.hidden_lines();
        let mut shown = Vec::with_capacity(count);
        let mut row = 0;
        let mut index = 0;
        let mut gaps = hidden.iter().peekable();
        while index < self.lines() && shown.len() < count {
            if let Some(gap) = gaps.peek()
                && index == gap.start
            {
                index = gap.end;
                gaps.next();
                continue;
            }
            if row >= first {
                shown.push(index);
            }
            row += 1;
            index += 1;
        }
        shown
    }

    /// Whether the line at `index` stands for lines folded away after it.
    pub fn is_folded_at(&self, index: usize) -> bool {
        self.folds
            .iter()
            .any(|fold| self.line_of(fold.head) == index)
    }

    /// The fold of the smallest block of three lines or more that holds the line at `index`, when there is one.
    fn fold_around_line(&self, index: usize) -> Option<(usize, usize, Fold)> {
        let text = self.field.text();
        let line = self.line(index);
        let leading = text[line.clone()].len() - text[line.clone()].trim_start().len();
        let from = (line.start + leading).min(text.len());
        let range = self.server_fold(index).or_else(|| match &self.syntax {
            Some(syntax) => syntax.fold_range(from),
            None => self.indented_block(index),
        })?;
        let first = self.line_of(range.start);
        let last = self.line_of(range.end.saturating_sub(1).max(range.start));
        (last > first + 1).then(|| {
            (
                first,
                last,
                Fold {
                    head: self.line(first).end,
                    tail: self.line(last).start,
                },
            )
        })
    }

    /// Folds away every block of three lines or more that is not inside another, so that what stays in view is the outline of the file.
    pub fn fold_all(&mut self) {
        let mut index = 0;
        while index < self.lines() {
            match self.fold_around_line(index) {
                Some((first, last, fold)) if first == index => {
                    if !self.folds.contains(&fold) {
                        self.folds.push(fold);
                    }
                    index = last + 1;
                }
                _ => index += 1,
            }
        }
        let caret = self.field.caret();
        if let Some(fold) = self
            .folds
            .iter()
            .find(|fold| (fold.head..fold.tail).contains(&caret))
            .copied()
        {
            self.field.apply(pm_text::Edit::Place(fold.head, false));
        }
    }

    /// Folds away the block around the caret: the lines between its first and its last; nothing happens when no block of three lines or more holds the caret.
    pub fn fold_at_caret(&mut self) -> bool {
        let caret = self.field.caret();
        let Some((_, _, fold)) = self.fold_around_line(self.line_of(caret)) else {
            return false;
        };
        if !self.folds.contains(&fold) {
            self.folds.push(fold);
        }
        if (fold.head..fold.tail).contains(&self.field.caret()) {
            self.field.apply(pm_text::Edit::Place(fold.head, false));
        }
        true
    }

    /// The bytes of the smallest block of three lines or more the language server said can be folded that holds the line at `index`, while what it said is about the text as it is.
    fn server_fold(&self, index: usize) -> Option<Range<usize>> {
        if !self.has_server_folds() {
            return None;
        }
        let last = self.lines().checked_sub(1)?;
        self.server_folds
            .1
            .iter()
            .map(|range| (range.start as usize, (range.end as usize).min(last)))
            .filter(|(first, end)| *first <= index && index <= *end && *end > *first + 1)
            .min_by_key(|(first, end)| end - first)
            .map(|(first, end)| self.line(first).start..self.line(end).end)
    }

    /// The lines after the line at `index` that are indented deeper than it, with the line after them, as a byte range, for a file without a syntax tree.
    fn indented_block(&self, index: usize) -> Option<Range<usize>> {
        let text = self.field.text();
        let depth = |row: usize| {
            let held = &text[self.line(row)];
            (!held.trim().is_empty()).then(|| held.len() - held.trim_start().len())
        };
        let own = depth(index)?;
        let mut last = index;
        for row in index + 1..self.lines() {
            match depth(row) {
                Some(deeper) if deeper > own => last = row,
                Some(_) => break,
                None => {}
            }
        }
        let closing = (last + 1).min(self.lines() - 1);
        (closing > index + 1).then(|| self.line(index).start..self.line(closing).end)
    }

    /// Unfolds the blocks the line of the caret stands for or is in; says whether there were any.
    pub fn unfold_at_caret(&mut self) -> bool {
        let line = self.line_of(self.field.caret());
        self.unfold_line(line)
    }

    /// Unfolds the blocks the line at `index` stands for or is hidden in; says whether there were any.
    pub fn unfold_line(&mut self, index: usize) -> bool {
        let before = self.folds.len();
        let lines: Vec<(usize, usize)> = self
            .folds
            .iter()
            .map(|fold| (self.line_of(fold.head), self.line_of(fold.tail)))
            .collect();
        let mut kept = lines.iter();
        self.folds.retain(|_| {
            let (head, tail) = kept.next().copied().unwrap_or_default();
            !(head <= index && index < tail)
        });
        self.folds.len() != before
    }

    /// Unfolds every block.
    pub fn unfold_all(&mut self) {
        self.folds.clear();
    }

    /// Carries the folds along `change`, dropping those whose first or last line it touched, and unfolds the block the caret went into.
    pub(crate) fn carry_folds(&mut self, change: Option<&Change>) {
        if let Some(change) = change {
            self.folds = self
                .folds
                .iter()
                .filter_map(|fold| {
                    Some(Fold {
                        head: shifted(fold.head, change)?,
                        tail: shifted(fold.tail, change)?,
                    })
                })
                .filter(|fold| fold.head < fold.tail)
                .collect();
        }
        let caret = self.field.caret();
        self.folds
            .retain(|fold| !(fold.head < caret && caret < fold.tail));
    }
}
