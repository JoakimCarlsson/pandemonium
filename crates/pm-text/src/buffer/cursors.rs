//! More than one cursor in one buffer, and doing one thing at all of them.
//!
//! A buffer has one selection it always has — the primary — and any number
//! of others beside it. Everything the buffer can be asked to do is written
//! against the primary alone; [`Buffer::at_each`] is what makes that one
//! thing happen at every cursor, by putting each of them in the primary's
//! place in turn and carrying the rest along past whatever the text did.

use std::ops::Range;

use crate::buffer::Buffer;
use crate::cursor::{Position, Selection};

impl Buffer {
    /// Every cursor, in the order they appear in the text.
    pub fn selections(&self) -> Vec<Selection> {
        let mut all = self.extra.clone();
        all.push(self.selection);
        all.sort_by_key(Selection::start);
        all
    }

    /// Whether there is more than one cursor.
    pub fn has_many_cursors(&self) -> bool {
        !self.extra.is_empty()
    }

    /// Selects exactly `selections`, the last of them being the primary.
    ///
    /// Cursors that have run into each other are merged: an edit that brings
    /// two of them to the same place has left one cursor, not two drawn on
    /// top of each other.
    pub fn set_selections(&mut self, selections: Vec<Selection>) {
        let mut kept: Vec<Selection> = Vec::with_capacity(selections.len());
        for selection in selections {
            let selection = Selection {
                anchor: self.clamped(selection.anchor),
                head: self.clamped(selection.head),
            };
            if kept.iter().any(|held| overlaps(*held, selection)) {
                continue;
            }
            kept.push(selection);
        }

        self.selection = kept.pop().unwrap_or_default();
        self.extra = kept;
        self.goal_column = None;
        self.history.commit();
    }

    /// Adds another cursor at `selection`.
    pub fn add_cursor(&mut self, selection: Selection) {
        let mut all = self.selections();
        all.push(selection);
        self.set_selections(all);
    }

    /// Leaves one cursor, saying whether there had been others.
    pub fn collapse_cursors(&mut self) -> bool {
        let many = self.has_many_cursors();
        self.extra.clear();
        many
    }

    /// Adds a cursor on the line above the topmost one, or below the lowest.
    pub fn add_cursor_vertically(&mut self, down: bool) {
        let all = self.selections();
        let edge = if down {
            all.last().copied()
        } else {
            all.first().copied()
        };
        let Some(edge) = edge else {
            return;
        };
        let line = match down {
            true => edge.head.line + 1,
            false => match edge.head.line.checked_sub(1) {
                Some(line) => line,
                None => return,
            },
        };
        if line >= self.line_count() {
            return;
        }

        let column = edge.head.column.min(self.line_len(line));
        self.add_cursor(Selection::at(Position::new(line, column)));
    }

    /// Selects the word under the cursor, or adds the next place it appears.
    ///
    /// The first press turns a cursor into a selection of the word it is in;
    /// every press after that finds the same text again and puts a cursor
    /// there too, which is how one name is renamed by typing over it.
    pub fn add_next_match(&mut self) {
        if self.selection.is_empty() {
            let word = self.word_at(self.selection.head);
            if word.start == word.end {
                return;
            }
            return self.select_range(word);
        }

        let needle = self.selected_text();
        if needle.contains('\n') {
            return;
        }
        let taken = self.selections();
        let from = taken
            .iter()
            .map(Selection::end)
            .max()
            .unwrap_or(self.selection.end());

        let Some(found) = self
            .occurrences(&needle)
            .into_iter()
            .find(|found| found.start >= from)
            .or_else(|| self.occurrences(&needle).into_iter().next())
        else {
            return;
        };
        self.add_cursor(Selection {
            anchor: found.start,
            head: found.end,
        });
    }

    /// Puts a cursor at every place the selected text appears.
    pub fn select_all_matches(&mut self) {
        if self.selection.is_empty() {
            let word = self.word_at(self.selection.head);
            if word.start == word.end {
                return;
            }
            self.select_range(word);
        }
        let needle = self.selected_text();
        if needle.is_empty() || needle.contains('\n') {
            return;
        }

        let all = self
            .occurrences(&needle)
            .into_iter()
            .map(|found| Selection {
                anchor: found.start,
                head: found.end,
            })
            .collect();
        self.set_selections(all);
    }

    /// One cursor per line, all covering the same columns of each.
    ///
    /// This is what dragging a box over the text comes to: the two corners
    /// are given in the columns they are drawn at, because a box is a shape
    /// on the screen, and a line too short to reach the box gets a cursor at
    /// its end rather than none at all.
    pub fn box_selection(&mut self, anchor: Position, head: Position) {
        let (top, bottom) = (anchor.line.min(head.line), anchor.line.max(head.line));
        let left = self.display_column(anchor).min(self.display_column(head));
        let right = self.display_column(anchor).max(self.display_column(head));

        let selections = (top..=bottom)
            .filter(|line| *line < self.line_count())
            .map(|line| Selection {
                anchor: self.position_at_display(line, left),
                head: self.position_at_display(line, right),
            })
            .collect();
        self.set_selections(selections);
    }

    /// Does `change` at every cursor, carrying the others past what it did.
    ///
    /// The cursors are worked through from the end of the text backwards, so
    /// an edit never moves the text a later one was measured against; the
    /// ones already done are carried along by however many characters each
    /// edit put in or took out. The whole of it is one undo step, because it
    /// is one thing the reader asked for.
    pub fn at_each(&mut self, change: impl FnMut(&mut Self)) {
        self.at(self.selections(), change);
    }

    /// Does `change` once per line the cursors are on.
    ///
    /// A command that rewrites whole lines — commenting them, moving them,
    /// making another copy of them — is asked for once per line however many
    /// cursors are sitting on it, because the line is what it acts on.
    pub fn on_each_line(&mut self, change: impl FnMut(&mut Self)) {
        let mut taken: Vec<std::ops::RangeInclusive<usize>> = Vec::new();
        let kept = self
            .selections()
            .into_iter()
            .filter(|selection| {
                let lines = selection.lines();
                let clash = taken
                    .iter()
                    .any(|held| held.start() <= lines.end() && lines.start() <= held.end());
                if !clash {
                    taken.push(lines);
                }
                !clash
            })
            .collect();
        self.at(kept, change);
    }

    /// Does `change` at each of `selections`, carrying the rest past it.
    fn at(&mut self, selections: Vec<Selection>, mut change: impl FnMut(&mut Self)) {
        if self.extra.is_empty() {
            return change(self);
        }

        let mut offsets = selections
            .iter()
            .map(|selection| (self.char_of(selection.anchor), self.char_of(selection.head)))
            .collect::<Vec<_>>();

        self.history.begin();
        for index in (0..offsets.len()).rev() {
            let (anchor, head) = offsets[index];
            self.selection = Selection {
                anchor: self.position_of(anchor),
                head: self.position_of(head),
            };
            let before = self.len_chars();
            change(self);
            let moved = self.len_chars() as isize - before as isize;

            offsets[index] = (
                self.char_of(self.selection.anchor),
                self.char_of(self.selection.head),
            );
            for later in offsets.iter_mut().skip(index + 1) {
                later.0 = later.0.saturating_add_signed(moved);
                later.1 = later.1.saturating_add_signed(moved);
            }
        }
        self.history.end();

        let selections = offsets
            .iter()
            .map(|(anchor, head)| Selection {
                anchor: self.position_of(*anchor),
                head: self.position_of(*head),
            })
            .collect();
        self.set_selections(selections);
    }

    /// Every place `needle` appears, in the order they appear.
    fn occurrences(&self, needle: &str) -> Vec<Range<Position>> {
        let width = needle.chars().count();
        let mut found = Vec::new();

        for line in 0..self.line_count() {
            let text = self.line_text(line);
            for (byte, _) in text.match_indices(needle) {
                let column = text[..byte].chars().count();
                found.push(Position::new(line, column)..Position::new(line, column + width));
            }
        }
        found
    }
}

/// Whether two cursors have run into each other.
fn overlaps(left: Selection, right: Selection) -> bool {
    left.start().max(right.start()) <= left.end().min(right.end())
}
