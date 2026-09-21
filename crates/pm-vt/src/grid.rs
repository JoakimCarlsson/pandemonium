//! The cell grid: the screen, the scrollback behind it and the cursor in it.
//!
//! The grid is the terminal's memory and nothing else. It knows what a line
//! feed, an erase or a scroll does to cells; it knows nothing of escape
//! sequences, which are [`crate::emulator`]'s, and nothing of pixels, which
//! are the caller's.

use std::collections::VecDeque;

use crate::cell::{Attrs, Cell};

/// Where the next character goes, and how it will be styled.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    /// Row within the screen, from the top.
    pub row: usize,
    /// Column within the row, from the left.
    pub col: usize,
    /// The style the next character is written in.
    pub attrs: Attrs,
    /// Whether the last write filled the final column and wrapping is due.
    pub wrap_pending: bool,
}

impl Cursor {
    /// A cursor at the home position with default attributes.
    fn home() -> Self {
        Self {
            row: 0,
            col: 0,
            attrs: Attrs::DEFAULT,
            wrap_pending: false,
        }
    }
}

/// One row of cells.
#[derive(Clone, Debug)]
pub struct Line {
    /// The cells, one per column.
    cells: Vec<Cell>,
}

impl Line {
    /// A blank line of `cols` cells styled with `attrs`.
    fn blank(cols: usize, attrs: Attrs) -> Self {
        Self {
            cells: vec![Cell::blank(attrs); cols],
        }
    }

    /// The cells of this row.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cell at `col`, if the row is that wide.
    pub fn cell(&self, col: usize) -> Option<&Cell> {
        self.cells.get(col)
    }

    /// Grows or shrinks this row to `cols` cells.
    fn resize(&mut self, cols: usize, attrs: Attrs) {
        self.cells.resize(cols, Cell::blank(attrs));
    }
}

/// The screen, the scrollback behind it and the cursor in it.
pub struct Grid {
    /// Columns in every row.
    cols: usize,
    /// Rows on the screen, scrollback aside.
    rows: usize,
    /// The screen itself, top row first.
    lines: VecDeque<Line>,
    /// Lines that have scrolled off the top, oldest first.
    scrollback: VecDeque<Line>,
    /// How many scrolled-off lines are kept.
    limit: usize,
    /// Where the next character goes.
    cursor: Cursor,
    /// The cursor saved by DECSC, restored by DECRC.
    saved: Cursor,
    /// First row of the scrolling region.
    top: usize,
    /// Last row of the scrolling region.
    bottom: usize,
    /// Which columns a tab stops at.
    tabs: Vec<bool>,
    /// How many lines the view is scrolled back from the screen.
    offset: usize,
}

impl Grid {
    /// A blank grid of `cols` by `rows`, keeping `limit` lines of scrollback.
    pub fn new(cols: usize, rows: usize, limit: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        Self {
            cols,
            rows,
            lines: (0..rows)
                .map(|_| Line::blank(cols, Attrs::DEFAULT))
                .collect(),
            scrollback: VecDeque::new(),
            limit,
            cursor: Cursor::home(),
            saved: Cursor::home(),
            top: 0,
            bottom: rows - 1,
            tabs: tab_stops(cols),
            offset: 0,
        }
    }

    /// Columns in every row.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Rows on the screen.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Where the next character goes.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// The style the next character is written in.
    pub fn attrs(&self) -> Attrs {
        self.cursor.attrs
    }

    /// Writes later characters in `attrs`.
    pub fn set_attrs(&mut self, attrs: Attrs) {
        self.cursor.attrs = attrs;
    }

    /// How many lines the view is scrolled back from the screen.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// How many lines the view could be scrolled back by.
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// The row shown at `index` from the top of the view.
    pub fn row(&self, index: usize) -> Option<&Line> {
        let history = self.scrollback.len();
        let first = history - self.offset;
        let absolute = first + index;
        if absolute < history {
            self.scrollback.get(absolute)
        } else {
            self.lines.get(absolute - history)
        }
    }

    /// Where the cursor is within the view, when the view is showing it.
    pub fn cursor_in_view(&self) -> Option<(usize, usize)> {
        let row = self.cursor.row + self.offset;
        (row < self.rows).then_some((row, self.cursor.col.min(self.cols - 1)))
    }

    /// Scrolls the view `lines` rows back through the scrollback.
    pub fn scroll_view(&mut self, lines: isize) {
        let offset = self.offset as isize + lines;
        self.offset = offset.clamp(0, self.scrollback.len() as isize) as usize;
    }

    /// Puts the view `lines` rows back from the screen.
    pub fn scroll_to(&mut self, lines: usize) {
        self.offset = lines.min(self.scrollback.len());
    }

    /// Brings the view back to the screen, where the cursor is.
    pub fn scroll_to_bottom(&mut self) {
        self.offset = 0;
    }

    /// Writes `ch` at the cursor, wrapping and advancing as the modes ask.
    pub fn write(&mut self, ch: char, width: usize, wrap: bool, insert: bool) {
        if self.cursor.wrap_pending && wrap {
            self.cursor.col = 0;
            self.index();
        }
        self.cursor.wrap_pending = false;

        if width == 2 && self.cursor.col + 1 >= self.cols {
            if !wrap {
                return;
            }
            self.cursor.col = 0;
            self.index();
        }

        let attrs = self.cursor.attrs;
        let (row, col) = (self.cursor.row, self.cursor.col.min(self.cols - 1));
        if insert {
            self.insert_chars(width.max(1));
        }

        let line = &mut self.lines[row];
        line.cells[col] = Cell {
            ch,
            attrs,
            width: width as u8,
        };
        if width == 2 && col + 1 < self.cols {
            line.cells[col + 1] = Cell {
                ch: ' ',
                attrs,
                width: 0,
            };
        }

        let advance = width.max(1);
        if col + advance >= self.cols {
            self.cursor.col = self.cols - 1;
            self.cursor.wrap_pending = wrap;
        } else {
            self.cursor.col = col + advance;
        }
    }

    /// Moves the cursor down one row, scrolling the region when it is at its foot.
    pub fn index(&mut self) {
        if self.cursor.row == self.bottom {
            self.scroll_up(1);
        } else if self.cursor.row + 1 < self.rows {
            self.cursor.row += 1;
        }
        self.cursor.wrap_pending = false;
    }

    /// Moves the cursor up one row, scrolling the region when it is at its head.
    pub fn reverse_index(&mut self) {
        if self.cursor.row == self.top {
            self.scroll_down(1);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
        self.cursor.wrap_pending = false;
    }

    /// Returns the cursor to the first column.
    pub fn carriage_return(&mut self) {
        self.cursor.col = 0;
        self.cursor.wrap_pending = false;
    }

    /// Moves the cursor one column back, without erasing anything.
    pub fn backspace(&mut self) {
        if self.cursor.wrap_pending {
            self.cursor.wrap_pending = false;
        } else if self.cursor.col > 0 {
            self.cursor.col -= 1;
        }
    }

    /// Moves the cursor to the next tab stop, `count` of them along.
    pub fn tab(&mut self, count: usize) {
        for _ in 0..count.max(1) {
            let mut col = self.cursor.col + 1;
            while col < self.cols && !self.tabs[col] {
                col += 1;
            }
            self.cursor.col = col.min(self.cols - 1);
        }
        self.cursor.wrap_pending = false;
    }

    /// Makes the cursor's column a tab stop.
    pub fn set_tab(&mut self) {
        let col = self.cursor.col;
        self.tabs[col] = true;
    }

    /// Clears the tab stop at the cursor, or every one of them.
    pub fn clear_tabs(&mut self, all: bool) {
        if all {
            self.tabs = vec![false; self.cols];
        } else {
            let col = self.cursor.col;
            self.tabs[col] = false;
        }
    }

    /// Puts the cursor at `row` and `col`, clamped to the screen.
    pub fn goto(&mut self, row: usize, col: usize) {
        self.cursor.row = row.min(self.rows - 1);
        self.cursor.col = col.min(self.cols - 1);
        self.cursor.wrap_pending = false;
    }

    /// Moves the cursor `rows` up, no further than the scrolling region.
    pub fn move_up(&mut self, rows: usize) {
        let limit = if self.cursor.row >= self.top {
            self.top
        } else {
            0
        };
        self.cursor.row = self.cursor.row.saturating_sub(rows.max(1)).max(limit);
        self.cursor.wrap_pending = false;
    }

    /// Moves the cursor `rows` down, no further than the scrolling region.
    pub fn move_down(&mut self, rows: usize) {
        let limit = if self.cursor.row <= self.bottom {
            self.bottom
        } else {
            self.rows - 1
        };
        self.cursor.row = (self.cursor.row + rows.max(1)).min(limit);
        self.cursor.wrap_pending = false;
    }

    /// Moves the cursor `cols` to the left.
    pub fn move_left(&mut self, cols: usize) {
        self.cursor.col = self.cursor.col.saturating_sub(cols.max(1));
        self.cursor.wrap_pending = false;
    }

    /// Moves the cursor `cols` to the right.
    pub fn move_right(&mut self, cols: usize) {
        self.cursor.col = (self.cursor.col + cols.max(1)).min(self.cols - 1);
        self.cursor.wrap_pending = false;
    }

    /// Saves the cursor and its style, for a later restore.
    pub fn save_cursor(&mut self) {
        self.saved = self.cursor;
    }

    /// Restores the cursor and style saved by [`Self::save_cursor`].
    pub fn restore_cursor(&mut self) {
        self.cursor = self.saved;
        self.cursor.row = self.cursor.row.min(self.rows - 1);
        self.cursor.col = self.cursor.col.min(self.cols - 1);
    }

    /// Confines scrolling to rows `top` through `bottom`, and homes the cursor.
    pub fn set_region(&mut self, top: usize, bottom: usize) {
        if top >= bottom || bottom >= self.rows {
            self.top = 0;
            self.bottom = self.rows - 1;
        } else {
            self.top = top;
            self.bottom = bottom;
        }
        self.goto(0, 0);
    }

    /// Erases from the cursor to the end of the line, or one of ED's other spans.
    pub fn erase_in_line(&mut self, mode: u16) {
        let attrs = self.cursor.attrs;
        let (row, col) = (self.cursor.row, self.cursor.col);
        let cols = self.cols;
        let line = &mut self.lines[row];
        let span = match mode {
            1 => 0..=col.min(cols - 1),
            2 => 0..=cols - 1,
            _ => col..=cols - 1,
        };
        for cell in span {
            line.cells[cell] = Cell::blank(attrs);
        }
        self.cursor.wrap_pending = false;
    }

    /// Erases part or all of the screen, or the scrollback behind it.
    pub fn erase_in_display(&mut self, mode: u16) {
        let attrs = self.cursor.attrs;
        let row = self.cursor.row;
        match mode {
            1 => {
                for index in 0..row {
                    self.lines[index] = Line::blank(self.cols, attrs);
                }
                self.erase_in_line(1);
            }
            2 | 3 => {
                for index in 0..self.rows {
                    self.lines[index] = Line::blank(self.cols, attrs);
                }
                if mode == 3 {
                    self.scrollback.clear();
                    self.offset = 0;
                }
            }
            _ => {
                for index in row + 1..self.rows {
                    self.lines[index] = Line::blank(self.cols, attrs);
                }
                self.erase_in_line(0);
            }
        }
    }

    /// Blanks `count` cells from the cursor, without moving anything.
    pub fn erase_chars(&mut self, count: usize) {
        let attrs = self.cursor.attrs;
        let (row, col) = (self.cursor.row, self.cursor.col);
        let end = (col + count.max(1)).min(self.cols);
        for index in col..end {
            self.lines[row].cells[index] = Cell::blank(attrs);
        }
    }

    /// Opens `count` blank cells at the cursor, pushing the rest of the row right.
    pub fn insert_chars(&mut self, count: usize) {
        let attrs = self.cursor.attrs;
        let (row, col) = (self.cursor.row, self.cursor.col);
        let count = count.max(1).min(self.cols - col);
        let line = &mut self.lines[row];
        for _ in 0..count {
            line.cells.insert(col, Cell::blank(attrs));
        }
        line.cells.truncate(self.cols);
    }

    /// Removes `count` cells at the cursor, pulling the rest of the row left.
    pub fn delete_chars(&mut self, count: usize) {
        let attrs = self.cursor.attrs;
        let (row, col) = (self.cursor.row, self.cursor.col);
        let count = count.max(1).min(self.cols - col);
        let line = &mut self.lines[row];
        for _ in 0..count {
            line.cells.remove(col);
        }
        line.cells.resize(self.cols, Cell::blank(attrs));
    }

    /// Opens `count` blank rows at the cursor, within the scrolling region.
    pub fn insert_lines(&mut self, count: usize) {
        if self.cursor.row < self.top || self.cursor.row > self.bottom {
            return;
        }
        let attrs = self.cursor.attrs;
        let count = count.max(1).min(self.bottom - self.cursor.row + 1);
        for _ in 0..count {
            self.lines.remove(self.bottom);
            self.lines
                .insert(self.cursor.row, Line::blank(self.cols, attrs));
        }
    }

    /// Removes `count` rows at the cursor, within the scrolling region.
    pub fn delete_lines(&mut self, count: usize) {
        if self.cursor.row < self.top || self.cursor.row > self.bottom {
            return;
        }
        let attrs = self.cursor.attrs;
        let count = count.max(1).min(self.bottom - self.cursor.row + 1);
        for _ in 0..count {
            self.lines.remove(self.cursor.row);
            self.lines
                .insert(self.bottom, Line::blank(self.cols, attrs));
        }
    }

    /// Scrolls the region up `count` rows, keeping what leaves the screen.
    ///
    /// Lines only reach the scrollback when the region is the whole screen:
    /// a program that has set a region is drawing inside it, and the rows it
    /// pushes out are part of that drawing, not history.
    pub fn scroll_up(&mut self, count: usize) {
        let attrs = self.cursor.attrs;
        let count = count.max(1).min(self.bottom - self.top + 1);
        let whole_screen = self.top == 0 && self.bottom == self.rows - 1;
        for _ in 0..count {
            let line = self
                .lines
                .remove(self.top)
                .expect("region is on the screen");
            if whole_screen && self.limit > 0 {
                self.scrollback.push_back(line);
                while self.scrollback.len() > self.limit {
                    self.scrollback.pop_front();
                }
            }
            self.lines
                .insert(self.bottom, Line::blank(self.cols, attrs));
        }
        if self.offset > 0 {
            self.offset = (self.offset + count).min(self.scrollback.len());
        }
    }

    /// Scrolls the region down `count` rows.
    pub fn scroll_down(&mut self, count: usize) {
        let attrs = self.cursor.attrs;
        let count = count.max(1).min(self.bottom - self.top + 1);
        for _ in 0..count {
            self.lines.remove(self.bottom);
            self.lines.insert(self.top, Line::blank(self.cols, attrs));
        }
    }

    /// Drops the scrollback and the view's offset into it.
    pub fn clear_scrollback(&mut self) {
        self.scrollback.clear();
        self.offset = 0;
    }

    /// Resizes the screen to `cols` by `rows`.
    ///
    /// Rows leaving the bottom of a shrinking screen come off the top instead,
    /// into the scrollback, so the cursor keeps the lines below it. Lines are
    /// not reflowed: a narrower screen truncates them rather than rewrapping
    /// what the program has already drawn.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return;
        }

        if cols != self.cols {
            for line in self.lines.iter_mut().chain(self.scrollback.iter_mut()) {
                line.resize(cols, Attrs::DEFAULT);
            }
            self.tabs = tab_stops(cols);
            self.cols = cols;
        }

        while self.lines.len() > rows {
            let below_cursor = self.lines.len() - 1 > self.cursor.row;
            if below_cursor {
                self.lines.pop_back();
            } else {
                let line = self.lines.pop_front().expect("screen is never empty");
                if self.limit > 0 {
                    self.scrollback.push_back(line);
                }
                self.cursor.row = self.cursor.row.saturating_sub(1);
            }
        }
        while self.lines.len() < rows {
            match self.scrollback.pop_back() {
                Some(line) => {
                    self.lines.push_front(line);
                    self.cursor.row += 1;
                }
                None => self.lines.push_back(Line::blank(cols, Attrs::DEFAULT)),
            }
        }

        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.cursor.row = self.cursor.row.min(rows - 1);
        self.cursor.col = self.cursor.col.min(cols - 1);
        self.offset = self.offset.min(self.scrollback.len());
        while self.scrollback.len() > self.limit {
            self.scrollback.pop_front();
        }
    }
}

/// A tab stop every eight columns, which is where every terminal starts.
fn tab_stops(cols: usize) -> Vec<bool> {
    (0..cols).map(|col| col % 8 == 0 && col > 0).collect()
}
