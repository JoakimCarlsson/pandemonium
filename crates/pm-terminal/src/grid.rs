//! The grid of cells a screen is made of, and the edits a terminal makes to it.

use std::collections::VecDeque;

use crate::cell::Cell;

/// How many lines scrolled off the top are kept, until the grid is told otherwise.
const HISTORY: usize = 5_000;

/// Where the next character goes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Cursor {
    /// The line, from the top of the grid.
    pub(crate) row: usize,
    /// The column, from the left.
    pub(crate) column: usize,
    /// Whether the last character filled the line, so the next one starts a new line.
    pub(crate) pending_wrap: bool,
}

/// A screenful of lines, the lines scrolled off above it and the region that scrolls.
#[derive(Clone, Debug)]
pub(crate) struct Grid {
    /// How many columns each line has.
    pub(crate) columns: usize,
    /// How many lines the grid has.
    pub(crate) rows: usize,
    /// The lines, top to bottom.
    pub(crate) lines: Vec<Vec<Cell>>,
    /// Where the next character goes.
    pub(crate) cursor: Cursor,
    /// The cursor as it was last saved.
    pub(crate) saved: Cursor,
    /// The first line of the scrolling region.
    pub(crate) top: usize,
    /// The last line of the scrolling region.
    pub(crate) bottom: usize,
    /// The lines scrolled off the top, oldest first.
    pub(crate) history: VecDeque<Vec<Cell>>,
    /// Whether lines scrolled off the top are kept.
    keeps_history: bool,
    /// How many of them are kept at most.
    most_history: usize,
}

impl Grid {
    /// An empty grid of `columns` by `rows`, which keeps what scrolls off its top when `keeps_history`.
    pub(crate) fn new(columns: usize, rows: usize, keeps_history: bool) -> Self {
        Self {
            columns,
            rows,
            lines: vec![vec![Cell::default(); columns]; rows],
            cursor: Cursor::default(),
            saved: Cursor::default(),
            top: 0,
            bottom: rows - 1,
            history: VecDeque::new(),
            keeps_history,
            most_history: HISTORY,
        }
    }

    /// Keeps at most `lines` of what scrolled off the top, the oldest going first.
    pub(crate) fn set_history(&mut self, lines: usize) {
        self.most_history = lines;
        while self.history.len() > lines {
            self.history.pop_front();
        }
    }

    /// A line of `fill` cells.
    fn blank(&self, fill: Cell) -> Vec<Cell> {
        vec![fill; self.columns]
    }

    /// Puts `cell` under the cursor and moves it right, wrapping to the next line first when the last character filled this one and `wrap` is set.
    pub(crate) fn put(&mut self, cell: Cell, wrap: bool, fill: Cell) {
        if self.cursor.pending_wrap {
            if wrap {
                self.cursor.column = 0;
                self.line_feed(fill);
            }
            self.cursor.pending_wrap = false;
        }
        let (row, column) = (self.cursor.row, self.cursor.column);
        self.lines[row][column] = cell;
        if column + 1 >= self.columns {
            self.cursor.pending_wrap = true;
        } else {
            self.cursor.column += 1;
        }
    }

    /// Moves the cursor down a line, scrolling the region when it is on the last line of it.
    pub(crate) fn line_feed(&mut self, fill: Cell) {
        if self.cursor.row == self.bottom {
            self.scroll_up(1, fill);
        } else if self.cursor.row + 1 < self.rows {
            self.cursor.row += 1;
        }
    }

    /// Moves the cursor up a line, scrolling the region down when it is on the first line of it.
    pub(crate) fn reverse_line_feed(&mut self, fill: Cell) {
        if self.cursor.row == self.top {
            self.scroll_down(1, fill);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
    }

    /// Scrolls the region up `count` lines, keeping what leaves the top of the screen.
    pub(crate) fn scroll_up(&mut self, count: usize, fill: Cell) {
        let count = count.min(self.bottom + 1 - self.top);
        for _ in 0..count {
            let line = self.lines.remove(self.top);
            if self.keeps_history && self.top == 0 {
                self.history.push_back(line);
                if self.history.len() > self.most_history {
                    self.history.pop_front();
                }
            }
            let blank = self.blank(fill);
            self.lines.insert(self.bottom, blank);
        }
    }

    /// Scrolls the region down `count` lines.
    pub(crate) fn scroll_down(&mut self, count: usize, fill: Cell) {
        let count = count.min(self.bottom + 1 - self.top);
        for _ in 0..count {
            self.lines.remove(self.bottom);
            let blank = self.blank(fill);
            self.lines.insert(self.top, blank);
        }
    }

    /// Erases the cells of line `row` from `from` up to but not including `to`.
    pub(crate) fn erase(&mut self, row: usize, from: usize, to: usize, fill: Cell) {
        let to = to.min(self.columns);
        if from < to {
            self.lines[row][from..to].fill(fill);
        }
    }

    /// Erases whole lines `from` up to but not including `to`.
    pub(crate) fn erase_lines(&mut self, from: usize, to: usize, fill: Cell) {
        for row in from..to.min(self.rows) {
            self.lines[row].fill(fill);
        }
    }

    /// Inserts `count` blank lines at the cursor, pushing the ones below it down within the region.
    pub(crate) fn insert_lines(&mut self, count: usize, fill: Cell) {
        if self.cursor.row < self.top || self.cursor.row > self.bottom {
            return;
        }
        let count = count.min(self.bottom + 1 - self.cursor.row);
        for _ in 0..count {
            self.lines.remove(self.bottom);
            let blank = self.blank(fill);
            self.lines.insert(self.cursor.row, blank);
        }
        self.cursor.column = 0;
        self.cursor.pending_wrap = false;
    }

    /// Deletes `count` lines at the cursor, pulling the ones below it up within the region.
    pub(crate) fn delete_lines(&mut self, count: usize, fill: Cell) {
        if self.cursor.row < self.top || self.cursor.row > self.bottom {
            return;
        }
        let count = count.min(self.bottom + 1 - self.cursor.row);
        for _ in 0..count {
            self.lines.remove(self.cursor.row);
            let blank = self.blank(fill);
            self.lines.insert(self.bottom, blank);
        }
        self.cursor.column = 0;
        self.cursor.pending_wrap = false;
    }

    /// Inserts `count` blank cells at the cursor, pushing the rest of the line right.
    pub(crate) fn insert_cells(&mut self, count: usize, fill: Cell) {
        let (row, column) = (self.cursor.row, self.cursor.column);
        let count = count.min(self.columns - column);
        let line = &mut self.lines[row];
        line.truncate(self.columns - count);
        for _ in 0..count {
            line.insert(column, fill);
        }
        self.cursor.pending_wrap = false;
    }

    /// Deletes `count` cells at the cursor, pulling the rest of the line left.
    pub(crate) fn delete_cells(&mut self, count: usize, fill: Cell) {
        let (row, column) = (self.cursor.row, self.cursor.column);
        let count = count.min(self.columns - column);
        let line = &mut self.lines[row];
        line.drain(column..column + count);
        line.resize(self.columns, fill);
        self.cursor.pending_wrap = false;
    }

    /// Changes the grid to `columns` by `rows`: lines are cut or padded, and a taller grid takes lines back from the history while a shorter one pushes its top lines into it.
    pub(crate) fn resize(&mut self, columns: usize, rows: usize) {
        for line in self.lines.iter_mut().chain(self.history.iter_mut()) {
            line.resize(columns, Cell::default());
        }
        self.columns = columns;
        if rows < self.rows {
            let mut excess = self.rows - rows;
            while excess > 0 && self.cursor.row + 1 < self.rows {
                self.lines.pop();
                self.rows -= 1;
                excess -= 1;
            }
            for _ in 0..excess {
                let line = self.lines.remove(0);
                if self.keeps_history {
                    self.history.push_back(line);
                }
                self.cursor.row = self.cursor.row.saturating_sub(1);
                self.rows -= 1;
            }
        } else {
            for _ in self.rows..rows {
                match self.history.pop_back().filter(|_| self.keeps_history) {
                    Some(line) => {
                        self.lines.insert(0, line);
                        self.cursor.row += 1;
                    }
                    None => self.lines.push(vec![Cell::default(); columns]),
                }
            }
        }
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.cursor.row = self.cursor.row.min(rows - 1);
        self.cursor.column = self.cursor.column.min(columns - 1);
        self.cursor.pending_wrap = false;
        self.saved.row = self.saved.row.min(rows - 1);
        self.saved.column = self.saved.column.min(columns - 1);
    }
}
