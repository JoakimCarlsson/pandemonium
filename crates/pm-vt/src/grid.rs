//! The cell grid: the screen, the scrollback behind it and the cursor in it.
//!
//! The grid is the terminal's memory and nothing else. It knows what a line
//! feed, an erase or a scroll does to cells; it knows nothing of escape
//! sequences, which are [`crate::emulator`]'s, and nothing of pixels, which
//! are the caller's.

use std::collections::VecDeque;

use crate::cell::{Attrs, Cell};
use crate::link::LinkId;
use crate::selection::{Place, Selection};

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
    /// The link the next character is part of, while a program has one open.
    pub link: Option<LinkId>,
}

impl Cursor {
    /// A cursor at the home position with default attributes.
    fn home() -> Self {
        Self {
            row: 0,
            col: 0,
            attrs: Attrs::DEFAULT,
            wrap_pending: false,
            link: None,
        }
    }
}

/// One row of cells.
#[derive(Clone, Debug)]
pub struct Line {
    /// The cells, one per column.
    cells: Vec<Cell>,
    /// Whether the text runs on into the next line rather than ending here.
    wrapped: bool,
}

impl Line {
    /// A blank line of `cols` cells styled with `attrs`.
    fn blank(cols: usize, attrs: Attrs) -> Self {
        Self {
            cells: vec![Cell::blank(attrs); cols],
            wrapped: false,
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

    /// Whether the text runs on into the next line rather than ending here.
    pub fn is_wrapped(&self) -> bool {
        self.wrapped
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
    /// How many lines have fallen off the far end of the scrollback.
    ///
    /// A [`Place`] counts from the first line ever kept, so this is what a
    /// place is measured against once the oldest lines are gone.
    dropped: usize,
    /// What the reader has picked out, if anything.
    selection: Option<Selection>,
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
            dropped: 0,
            selection: None,
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

    /// Opens `link` for the characters written from now on, or closes it.
    pub fn set_link(&mut self, link: Option<LinkId>) {
        self.cursor.link = link;
    }

    /// The place of the cell at `row` and `col` of the view.
    pub fn place_at(&self, row: usize, col: usize) -> Place {
        let first = self.dropped + self.scrollback.len() - self.offset;
        Place::new(first + row, col.min(self.cols - 1))
    }

    /// The row of the view `line` is shown on, if the view is showing it.
    pub fn view_row(&self, line: usize) -> Option<usize> {
        let first = self.dropped + self.scrollback.len() - self.offset;
        line.checked_sub(first).filter(|row| *row < self.rows)
    }

    /// The line `line` counts to, while it is still kept.
    pub fn line(&self, line: usize) -> Option<&Line> {
        let index = line.checked_sub(self.dropped)?;
        let history = self.scrollback.len();
        if index < history {
            self.scrollback.get(index)
        } else {
            self.lines.get(index - history)
        }
    }

    /// The first place kept and the last, scrollback and screen together.
    pub fn extent(&self) -> (Place, Place) {
        let last = self.dropped + self.scrollback.len() + self.rows - 1;
        (Place::new(self.dropped, 0), Place::new(last, self.cols - 1))
    }

    /// The cell before `place`, stepping back onto a line that wrapped into it.
    pub fn step_back(&self, place: Place) -> Option<Place> {
        if place.col > 0 {
            return Some(Place::new(place.line, place.col - 1));
        }
        let above = place.line.checked_sub(1)?;
        self.line(above)
            .filter(|line| line.is_wrapped())
            .map(|_| Place::new(above, self.cols - 1))
    }

    /// The cell after `place`, stepping on to the line this one wraps into.
    pub fn step_forward(&self, place: Place) -> Option<Place> {
        if place.col + 1 < self.cols {
            return Some(Place::new(place.line, place.col + 1));
        }
        let wraps = self.line(place.line)?.is_wrapped();
        let below = place.line + 1;
        (wraps && self.line(below).is_some()).then_some(Place::new(below, 0))
    }

    /// Every character of the wrapped line `line` is part of, with its place.
    ///
    /// The trailing halves of wide characters are left out, so the text reads
    /// as it was written while each character still knows where it is drawn.
    pub fn logical_line(&self, line: usize) -> Vec<(char, Place)> {
        let mut first = line;
        while first > 0 && self.line(first - 1).is_some_and(Line::is_wrapped) {
            first -= 1;
        }
        let mut text = Vec::new();
        let mut number = first;
        while let Some(row) = self.line(number) {
            text.extend(
                row.cells()
                    .iter()
                    .enumerate()
                    .filter(|(_, cell)| !cell.is_spacer())
                    .map(|(col, cell)| (cell.ch, Place::new(number, col))),
            );
            if !row.is_wrapped() {
                break;
            }
            number += 1;
        }
        text
    }

    /// What the reader has picked out, if anything.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Picks out `selection`, or lets go of what was picked out.
    pub fn set_selection(&mut self, selection: Option<Selection>) {
        self.selection = selection;
    }

    /// Writes `ch` at the cursor, wrapping and advancing as the modes ask.
    pub fn write(&mut self, ch: char, width: usize, wrap: bool, insert: bool) {
        if self.cursor.wrap_pending && wrap {
            self.wrap_line();
        }
        self.cursor.wrap_pending = false;

        if width == 2 && self.cursor.col + 1 >= self.cols {
            if !wrap {
                return;
            }
            let (row, last) = (self.cursor.row, self.cols - 1);
            self.lines[row].cells[last] = Cell::carried(self.cursor.attrs);
            self.wrap_line();
        }

        let (attrs, link) = (self.cursor.attrs, self.cursor.link);
        let (row, col) = (self.cursor.row, self.cursor.col.min(self.cols - 1));
        if insert {
            self.insert_chars(width.max(1));
        }

        let line = &mut self.lines[row];
        line.cells[col] = Cell {
            ch,
            attrs,
            width: width as u8,
            link,
        };
        if width == 2 && col + 1 < self.cols {
            line.cells[col + 1] = Cell {
                ch: ' ',
                attrs,
                width: 0,
                link,
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

    /// Carries the cursor on to the start of the next row, marking this one as
    /// running on into it.
    fn wrap_line(&mut self) {
        self.lines[self.cursor.row].wrapped = true;
        self.cursor.col = 0;
        self.index();
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
        if mode != 1 {
            line.wrapped = false;
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
                    self.clear_scrollback();
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
                self.trim_scrollback();
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
        self.dropped += self.scrollback.len();
        self.scrollback.clear();
        self.offset = 0;
    }

    /// Resizes the screen to `cols` by `rows`.
    ///
    /// Lines are rewrapped to the new width, scrollback and screen together,
    /// so narrowing the screen and widening it again gives back what was
    /// drawn rather than what survived the narrowest moment. The screen keeps
    /// its top line where it can; rows leaving the bottom of a shrinking
    /// screen come off the top instead, into the scrollback, so the cursor
    /// keeps the lines below it, and a growing screen draws its new rows back
    /// down out of the scrollback.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return;
        }

        let history = self.scrollback.len();
        let marks = [
            Mark::at(
                history + self.cursor.row,
                self.cursor.col,
                self.cursor.wrap_pending,
            ),
            Mark::at(
                history + self.saved.row,
                self.saved.col,
                self.saved.wrap_pending,
            ),
            Mark::at(history, 0, false),
        ];
        let lines: Vec<Line> = self
            .scrollback
            .drain(..)
            .chain(self.lines.drain(..))
            .collect();
        let (mut rewrapped, [cursor, saved, top]) = reflow(lines, cols, marks);

        let start = top
            .line
            .max((cursor.line + 1).saturating_sub(rows))
            .min(cursor.line)
            .min(rewrapped.len().saturating_sub(rows));
        if rewrapped.len() > start + rows {
            rewrapped.truncate(start + rows);
            if let Some(last) = rewrapped.last_mut() {
                last.wrapped = false;
            }
        }
        let mut screen = rewrapped.split_off(start);
        screen.resize_with(rows, || Line::blank(cols, Attrs::DEFAULT));
        self.scrollback = rewrapped.into();
        self.lines = screen.into();

        self.cols = cols;
        self.rows = rows;
        self.tabs = tab_stops(cols);
        self.top = 0;
        self.bottom = rows - 1;
        cursor.place(&mut self.cursor, start, cols, rows);
        saved.place(&mut self.saved, start, cols, rows);
        self.trim_scrollback();
        self.offset = self.offset.min(self.scrollback.len());
        self.selection = None;
    }

    /// Keeps `limit` lines of scrollback from now on, dropping the oldest of
    /// what it already has when that is more.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.trim_scrollback();
        self.offset = self.offset.min(self.scrollback.len());
    }

    /// Drops the oldest lines of the scrollback until it is within its limit.
    fn trim_scrollback(&mut self) {
        while self.scrollback.len() > self.limit {
            self.scrollback.pop_front();
            self.dropped += 1;
        }
    }
}

/// A tab stop every eight columns, which is where every terminal starts.
fn tab_stops(cols: usize) -> Vec<bool> {
    (0..cols).map(|col| col % 8 == 0 && col > 0).collect()
}

/// A place in the grid followed through a reflow: the cursor, the saved
/// cursor, and the first line of the screen.
#[derive(Clone, Copy, Debug)]
struct Mark {
    /// Line counted from the oldest line of the scrollback.
    line: usize,
    /// Column within that line; one past the last column while a wrap is due.
    col: usize,
}

impl Mark {
    /// The mark on `line` at `col`, one column further on while a wrap is due.
    fn at(line: usize, col: usize, wrap_pending: bool) -> Self {
        Self {
            line,
            col: col + usize::from(wrap_pending),
        }
    }

    /// Moves `cursor` to this mark, on a screen whose first line is `start`
    /// and which is `cols` by `rows`.
    fn place(self, cursor: &mut Cursor, start: usize, cols: usize, rows: usize) {
        cursor.row = self.line.saturating_sub(start).min(rows - 1);
        cursor.wrap_pending = self.col >= cols;
        cursor.col = self.col.min(cols - 1);
    }
}

/// Rewraps `lines` to `cols` columns, and follows `marks` to where they land.
///
/// Each run of wrapped lines is joined into the text it was written as,
/// stripped of the blank cells trailing it, and cut again at the new width.
/// A wide character that would straddle the last column moves to the next
/// row whole. Cells a mark sits on are never stripped, so the cursor keeps
/// the spaces it was typed after.
fn reflow<const N: usize>(
    lines: Vec<Line>,
    cols: usize,
    marks: [Mark; N],
) -> (Vec<Line>, [Mark; N]) {
    let blank = Cell::default();
    let mut rewrapped = Vec::with_capacity(lines.len());
    let mut placed = marks;
    let mut lines = lines.into_iter().enumerate();
    while let Some((first, mut line)) = lines.next() {
        if !line.wrapped && fits(&line, first, cols, &marks, &blank) {
            for (mark, target) in placed.iter_mut().zip(&marks) {
                if target.line == first {
                    *mark = Mark {
                        line: rewrapped.len(),
                        col: target.col,
                    };
                }
            }
            line.resize(cols, Attrs::DEFAULT);
            rewrapped.push(line);
            continue;
        }
        let mut run = vec![line];
        while run.last().is_some_and(Line::is_wrapped) {
            match lines.next() {
                Some((_, line)) => run.push(line),
                None => break,
            }
        }
        let last = first + run.len() - 1;

        let written = written_lengths(&run);
        let mut offsets = [None; N];
        for (offset, mark) in offsets.iter_mut().zip(&marks) {
            if (first..=last).contains(&mark.line) {
                let before: usize = written[..mark.line - first].iter().sum();
                *offset = Some(before + mark.col);
            }
        }
        let cells: Vec<Cell> = match run.len() {
            1 => run.pop().expect("a run holds a line").cells,
            _ => join(&run, &written),
        };
        let content = cells
            .iter()
            .rposition(|cell| *cell != blank)
            .map_or(0, |index| index + 1);
        let kept = offsets
            .iter()
            .flatten()
            .map(|offset| offset + 1)
            .fold(content, usize::max)
            .min(cells.len());

        let mut start = 0;
        loop {
            let mut end = (start + cols).min(kept);
            let carried = end - start == cols && cols > 1 && cells[end - 1].width == 2;
            if carried {
                end -= 1;
            }
            let last = end >= kept;
            for (mark, target) in placed.iter_mut().zip(&offsets) {
                if target.is_some_and(|offset| offset >= start && (offset < end || last)) {
                    *mark = Mark {
                        line: rewrapped.len(),
                        col: target.unwrap_or(start) - start,
                    };
                }
            }
            let mut row = wrapped_row(cells[start..end].to_vec(), cols);
            if carried {
                row.cells[cols - 1] = Cell::carried(Attrs::DEFAULT);
            }
            row.wrapped = !last;
            rewrapped.push(row);
            if last {
                break;
            }
            start = end;
        }
    }
    (rewrapped, placed)
}

/// How many cells of each line in `run` belong to the text written there:
/// all of them but the blank a wide character left behind when it was
/// carried to the next row whole.
fn written_lengths(run: &[Line]) -> Vec<usize> {
    run.iter()
        .map(|line| {
            let carried = match line.cells.as_slice() {
                [.., before, last] => last.is_spacer() && before.width != 2,
                [last] => last.is_spacer(),
                [] => false,
            };
            line.cells.len() - usize::from(carried)
        })
        .collect()
}

/// The cells of a run of wrapped lines, as the one line they were written
/// as, each line cut to its `written` length.
fn join(run: &[Line], written: &[usize]) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(written.iter().sum());
    for (line, length) in run.iter().zip(written) {
        cells.extend_from_slice(&line.cells[..*length]);
    }
    cells
}

/// Whether `line`, numbered `number`, keeps its cells as they are at `cols`
/// columns: nothing past the width but blanks, and no mark past it either.
///
/// Most of the scrollback is such lines, and cutting or padding one in
/// place is what keeps a resize from copying every cell it holds.
fn fits(line: &Line, number: usize, cols: usize, marks: &[Mark], blank: &Cell) -> bool {
    let marked = marks
        .iter()
        .filter(|mark| mark.line == number)
        .all(|mark| mark.col < cols);
    marked
        && line
            .cells
            .get(cols..)
            .is_none_or(|rest| rest.iter().all(|cell| cell == blank))
}

/// A row of `cells` padded out to `cols`, running on into the next.
fn wrapped_row(cells: Vec<Cell>, cols: usize) -> Line {
    let mut line = Line {
        cells,
        wrapped: true,
    };
    line.resize(cols, Attrs::DEFAULT);
    line
}
