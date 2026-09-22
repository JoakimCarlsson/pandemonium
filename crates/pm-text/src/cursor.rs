//! Where a buffer is being looked at: a place in it, and a span of it.
//!
//! A position is counted in characters rather than bytes, because that is
//! what a column on the screen and a keypress both mean by "one across".
//! Everything a buffer is asked to do is asked in these terms; the byte
//! offsets a rope and a syntax tree want are the buffer's own business.

/// A place in a buffer: which line, and how far into it.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position {
    /// Which line, counted from zero.
    pub line: usize,
    /// How many characters into that line, counted from zero.
    pub column: usize,
}

impl Position {
    /// The position at `line` and `column`.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Where `text` leaves the cursor, having been put in here.
    pub fn after(self, text: &str) -> Self {
        match text.rsplit_once('\n') {
            Some((before, rest)) => Self::new(
                self.line + before.matches('\n').count() + 1,
                rest.chars().count(),
            ),
            None => Self::new(self.line, self.column + text.chars().count()),
        }
    }
}

/// What is selected: where the selection was begun, and where it is now.
///
/// The head is the end that moves, so extending a selection with the
/// keyboard and dragging one with the pointer are the same operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    /// The end the selection was begun at.
    pub anchor: Position,
    /// The end that moves, and where the cursor is drawn.
    pub head: Position,
}

impl Selection {
    /// A selection of nothing, at `position`.
    pub const fn at(position: Position) -> Self {
        Self {
            anchor: position,
            head: position,
        }
    }

    /// Whether the selection covers no text at all.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The earlier of the two ends.
    pub fn start(&self) -> Position {
        self.anchor.min(self.head)
    }

    /// The later of the two ends.
    pub fn end(&self) -> Position {
        self.anchor.max(self.head)
    }

    /// Whether `line` has any of the selection on it.
    pub fn touches(&self, line: usize) -> bool {
        (self.start().line..=self.end().line).contains(&line)
    }

    /// The lines the selection reaches, however little of them it covers.
    ///
    /// A selection that ends at the very start of a line has not reached
    /// into it, which is what makes selecting three whole lines indent
    /// three rather than four.
    pub fn lines(&self) -> std::ops::RangeInclusive<usize> {
        let (start, end) = (self.start(), self.end());
        let last = if end.line > start.line && end.column == 0 {
            end.line - 1
        } else {
            end.line
        };
        start.line..=last
    }
}

/// A way of moving the cursor that does not depend on where it is.
///
/// The buffer resolves a motion against its own text: only it knows how long
/// a line is or where the next word begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    /// One character back.
    Left,
    /// One character on.
    Right,
    /// One line up, keeping the column aimed at.
    Up,
    /// One line down, keeping the column aimed at.
    Down,
    /// To the start of the word before the cursor.
    WordLeft,
    /// To the end of the word after the cursor.
    WordRight,
    /// To the first character of the line that is not a space.
    LineStart,
    /// To the end of the line.
    LineEnd,
    /// To the start of the buffer.
    BufferStart,
    /// To the end of the buffer.
    BufferEnd,
    /// Up by as many lines as a pane holds.
    PageUp(usize),
    /// Down by as many lines as a pane holds.
    PageDown(usize),
    /// To a place named outright, which a search or a server decided on.
    To(Position),
}

impl Motion {
    /// Whether this motion aims at a column of its own.
    ///
    /// Moving up or down keeps the column the cursor was last aimed at, so
    /// that travelling past a short line does not pull the cursor left for
    /// good; every other motion decides a new one.
    pub const fn keeps_goal_column(self) -> bool {
        matches!(
            self,
            Self::Up | Self::Down | Self::PageUp(_) | Self::PageDown(_)
        )
    }
}
