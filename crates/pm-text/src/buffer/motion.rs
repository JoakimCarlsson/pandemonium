//! Where the cursor is, and everything that moves it.
//!
//! A motion is asked for in the reader's terms — a character, a word, a page
//! — and resolved here against the text, because only the buffer knows how
//! long a line is or where the next word begins. Nothing in this file
//! changes the text; what it changes is which part of it is being looked at.

use crate::buffer::Buffer;
use crate::cursor::{Motion, Position, Selection};

/// The kinds of character a word motion tells apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Class {
    /// A space, a tab or another blank.
    Space,
    /// A letter, a digit or an underscore.
    Word,
    /// Anything else: punctuation, operators, brackets.
    Symbol,
}

/// Which kind of character `ch` is, for a word motion.
pub(crate) fn class(ch: char) -> Class {
    match ch {
        ch if ch.is_whitespace() => Class::Space,
        ch if ch.is_alphanumeric() || ch == '_' => Class::Word,
        _ => Class::Symbol,
    }
}

impl Buffer {
    /// Puts the cursor at `position`, extending the selection when asked.
    pub fn place(&mut self, position: Position, extend: bool) {
        let head = self.clamped(position);
        self.set_selection(Selection {
            anchor: if extend {
                self.selection().anchor
            } else {
                head
            },
            head,
        });
    }

    /// Selects exactly `selection`, brought inside the text it points into.
    ///
    /// Every other way of moving the cursor ends here, so the goal column a
    /// vertical motion aims at is forgotten in one place rather than in a
    /// dozen.
    pub fn set_selection(&mut self, selection: Selection) {
        self.place_selection(selection);
        self.history.commit();
    }

    /// Selects exactly `selection` without ending the undo step being made.
    ///
    /// Moving the cursor ends a step, because a keystroke after a deliberate
    /// move is a new thing being written. An edit moves the cursor too, and
    /// that move is part of the edit rather than a break in it — which is
    /// what makes a typed word one thing to take back.
    pub(crate) fn place_selection(&mut self, selection: Selection) {
        self.selection = Selection {
            anchor: self.clamped(selection.anchor),
            head: self.clamped(selection.head),
        };
        self.goal_column = None;
    }

    /// Selects `range` and nothing else, leaving the cursor at the end of it.
    pub fn select_range(&mut self, range: std::ops::Range<Position>) {
        self.collapse_cursors();
        self.set_selection(Selection {
            anchor: range.start,
            head: range.end,
        });
    }

    /// Selects the word `position` falls in, as a double click does.
    pub fn select_word(&mut self, position: Position) {
        let position = self.clamped(position);
        let chars = self.line_chars(position.line).collect::<Vec<_>>();
        let kind = chars.get(position.column).copied().map(class);
        let matches = |index: usize| chars.get(index).copied().map(class) == kind;

        let mut start = position.column;
        while start > 0 && matches(start - 1) {
            start -= 1;
        }
        let mut end = position.column;
        while end < chars.len() && matches(end) {
            end += 1;
        }

        self.set_selection(Selection {
            anchor: Position::new(position.line, start),
            head: Position::new(position.line, end),
        });
    }

    /// Selects the whole of the line `position` falls on.
    ///
    /// The selection reaches into the line below, the way a triple click
    /// does everywhere, so that typing over it or cutting it takes the line
    /// break with it.
    pub fn select_line(&mut self, position: Position) {
        let line = position.line.min(self.line_count().saturating_sub(1));
        let end = match line + 1 < self.line_count() {
            true => Position::new(line + 1, 0),
            false => Position::new(line, self.line_len(line)),
        };
        self.set_selection(Selection {
            anchor: Position::new(line, 0),
            head: end,
        });
    }

    /// Selects the text of the line `position` falls on, and not its break.
    ///
    /// A triple click takes the line it lands on and leaves the cursor at
    /// its end: reaching into the line below would put the cursor on a line
    /// nobody clicked, and on an empty line select nothing but the break.
    pub fn select_line_text(&mut self, position: Position) {
        let line = position.line.min(self.line_count().saturating_sub(1));
        self.set_selection(Selection {
            anchor: Position::new(line, 0),
            head: Position::new(line, self.line_len(line)),
        });
    }

    /// Selects everything the buffer holds.
    pub fn select_all(&mut self) {
        self.collapse_cursors();
        let last = self.line_count().saturating_sub(1);
        self.set_selection(Selection {
            anchor: Position::default(),
            head: Position::new(last, self.line_len(last)),
        });
    }

    /// Grows the selection to the word under the cursor, or to its line.
    ///
    /// Asking twice takes the line, which is the step every editor takes
    /// after the word and before the block.
    pub fn expand_selection(&mut self) {
        let head = self.selection().head;
        if self.selection().is_empty() {
            return self.select_word(head);
        }
        self.select_line(head);
    }

    /// Moves the cursor, extending the selection when asked.
    pub fn move_cursor(&mut self, motion: Motion, extend: bool) {
        let head = self.moved(motion);
        let anchor = if extend {
            self.selection().anchor
        } else {
            head
        };
        self.selection = Selection {
            anchor: self.clamped(anchor),
            head: self.clamped(head),
        };
        if !motion.keeps_goal_column() {
            self.goal_column = None;
        }
        self.history.commit();
    }

    /// The start of the word before `position`.
    pub(crate) fn word_start(&self, position: Position) -> Position {
        if position.column == 0 {
            return match position.line {
                0 => position,
                line => Position::new(line - 1, self.line_len(line - 1)),
            };
        }
        let chars = self.line_chars(position.line).collect::<Vec<_>>();
        let mut column = position.column.min(chars.len());
        while column > 0 && class(chars[column - 1]) == Class::Space {
            column -= 1;
        }
        let kind = chars.get(column.wrapping_sub(1)).copied().map(class);
        while column > 0 && Some(class(chars[column - 1])) == kind {
            column -= 1;
        }
        Position::new(position.line, column)
    }

    /// The end of the word after `position`.
    pub(crate) fn word_end(&self, position: Position) -> Position {
        let chars = self.line_chars(position.line).collect::<Vec<_>>();
        if position.column >= chars.len() {
            return match position.line + 1 < self.line_count() {
                true => Position::new(position.line + 1, 0),
                false => position,
            };
        }
        let mut column = position.column;
        while column < chars.len() && class(chars[column]) == Class::Space {
            column += 1;
        }
        let kind = chars.get(column).copied().map(class);
        while column < chars.len() && Some(class(chars[column])) == kind {
            column += 1;
        }
        Position::new(position.line, column)
    }

    /// The span of the word at `position`, whether or not it is selected.
    pub fn word_at(&self, position: Position) -> std::ops::Range<Position> {
        let position = self.clamped(position);
        let chars = self.line_chars(position.line).collect::<Vec<_>>();
        let inside = |index: usize| chars.get(index).copied().map(class) == Some(Class::Word);

        let mut start = position.column;
        while start > 0 && inside(start - 1) {
            start -= 1;
        }
        let mut end = position.column;
        while end < chars.len() && inside(end) {
            end += 1;
        }
        Position::new(position.line, start)..Position::new(position.line, end)
    }

    /// Where `motion` takes the cursor from where it is.
    fn moved(&mut self, motion: Motion) -> Position {
        let head = self.selection().head;
        let last = self.line_count().saturating_sub(1);
        match motion {
            Motion::Left if !self.selection().is_empty() => self.selection().start(),
            Motion::Right if !self.selection().is_empty() => self.selection().end(),
            Motion::Left if head.column > 0 => Position::new(head.line, head.column - 1),
            Motion::Left if head.line > 0 => {
                Position::new(head.line - 1, self.line_len(head.line - 1))
            }
            Motion::Left => head,
            Motion::Right if head.column < self.line_len(head.line) => {
                Position::new(head.line, head.column + 1)
            }
            Motion::Right if head.line < last => Position::new(head.line + 1, 0),
            Motion::Right => head,
            Motion::Up => self.vertical(head, -1),
            Motion::Down => self.vertical(head, 1),
            Motion::PageUp(lines) => self.vertical(head, -(lines as isize)),
            Motion::PageDown(lines) => self.vertical(head, lines as isize),
            Motion::WordLeft => self.word_start(head),
            Motion::WordRight => self.word_end(head),
            Motion::LineStart => Position::new(head.line, self.indent_of(head.line, head.column)),
            Motion::LineEnd => Position::new(head.line, self.line_len(head.line)),
            Motion::BufferStart => Position::default(),
            Motion::BufferEnd => Position::new(last, self.line_len(last)),
            Motion::To(position) => self.clamped(position),
        }
    }

    /// Where the cursor lands `lines` below where it is, or above.
    fn vertical(&mut self, head: Position, lines: isize) -> Position {
        let goal = *self.goal_column.get_or_insert(head.column);
        let line = head
            .line
            .saturating_add_signed(lines)
            .min(self.line_count().saturating_sub(1));
        Position::new(line, goal.min(self.line_len(line)))
    }

    /// The column the text of `line` begins at, or zero when already there.
    ///
    /// Home goes to the first character that is not a space, and to the
    /// margin from there, so the key alternates between the two places a
    /// line can be said to start.
    fn indent_of(&self, line: usize, column: usize) -> usize {
        let indent = self
            .line_chars(line)
            .take_while(|ch| ch.is_whitespace())
            .count();
        if column == indent { 0 } else { indent }
    }
}
