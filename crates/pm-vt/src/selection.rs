//! What part of the screen and its scrollback the reader has picked out.
//!
//! A selection is held in [`Place`]s, which count lines from the first one
//! the terminal ever kept rather than from the top of the view, so text that
//! scrolls away keeps its selection on it rather than leaving it behind on
//! whatever scrolled into its rows.

use crate::grid::Grid;

/// Characters that end a word, besides whitespace.
///
/// A path, a flag and an address are each one word to a double click, so the
/// slash, the dot, the dash and the colon are not among them.
const DELIMITERS: &[char] = &[
    '"', '\'', '`', '(', ')', '[', ']', '{', '}', '<', '>', '|', ',', ';', '│',
];

/// One cell, by the line it is on and its column along that line.
///
/// Lines are counted from the first one the terminal kept, scrollback
/// included, so a place names the same cell however far the view scrolls.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Place {
    /// The line, counted from the first one kept.
    pub line: usize,
    /// The column along it.
    pub col: usize,
}

impl Place {
    /// The cell at `col` of `line`.
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// What a selection grows by as the pointer drags it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Unit {
    /// A cell at a time, from a single press.
    #[default]
    Cell,
    /// A word at a time, from a double press.
    Word,
    /// A line at a time, from a triple press.
    Line,
}

/// The cells between where a drag began and where it has reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection {
    /// Where the drag began.
    pub anchor: Place,
    /// Where it has reached.
    pub head: Place,
    /// What it grows by.
    pub unit: Unit,
}

impl Selection {
    /// The first and last cells it covers in `grid`, grown to whole units.
    pub fn span(&self, grid: &Grid) -> (Place, Place) {
        let (start, end) = if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        match self.unit {
            Unit::Cell => (start, end),
            Unit::Word => (word_start(grid, start), word_end(grid, end)),
            Unit::Line => (line_start(grid, start), line_end(grid, end)),
        }
    }

    /// The text it covers in `grid`, a line break where a line really ends.
    ///
    /// A line the terminal wrapped runs straight on into the next, and the
    /// blanks after the end of a line are not part of what was written there.
    pub fn text(&self, grid: &Grid) -> String {
        let (start, end) = self.span(grid);
        let mut text = String::new();
        for number in start.line..=end.line {
            let Some(line) = grid.line(number) else {
                continue;
            };
            let first = if number == start.line { start.col } else { 0 };
            let last = if number == end.line {
                end.col.min(grid.cols() - 1)
            } else {
                grid.cols() - 1
            };
            let written: String = line
                .cells()
                .iter()
                .take(last + 1)
                .skip(first)
                .filter(|cell| !cell.is_spacer())
                .map(|cell| cell.ch)
                .collect();
            let continues = line.is_wrapped() && number != end.line;
            if continues {
                text.push_str(&written);
            } else {
                text.push_str(written.trim_end());
                if number != end.line {
                    text.push('\n');
                }
            }
        }
        text.trim_end_matches('\n').to_owned()
    }
}

/// Which kind of character the cell at `place` holds, for telling words apart.
///
/// The trailing half of a wide character belongs to the character, and every
/// delimiter is a kind of its own so that a double click on one takes only it.
fn class(grid: &Grid, place: Place) -> Option<u32> {
    let cell = grid.line(place.line)?.cell(place.col)?;
    Some(match cell.ch {
        _ if cell.is_spacer() => 1,
        ch if ch.is_whitespace() || ch == '\0' => 0,
        ch if DELIMITERS.contains(&ch) => 2 + ch as u32,
        _ => 1,
    })
}

/// Whether a run of `class` grows past a single cell.
fn runs(class: u32) -> bool {
    class < 2
}

/// The first cell of the word `place` is in.
fn word_start(grid: &Grid, place: Place) -> Place {
    let Some(kind) = class(grid, place).filter(|kind| runs(*kind)) else {
        return place;
    };
    let mut start = place;
    while let Some(before) = grid
        .step_back(start)
        .filter(|before| class(grid, *before) == Some(kind))
    {
        start = before;
    }
    start
}

/// The last cell of the word `place` is in.
fn word_end(grid: &Grid, place: Place) -> Place {
    let Some(kind) = class(grid, place).filter(|kind| runs(*kind)) else {
        return place;
    };
    let mut end = place;
    while let Some(after) = grid
        .step_forward(end)
        .filter(|after| class(grid, *after) == Some(kind))
    {
        end = after;
    }
    end
}

/// The first cell of the wrapped line `place` is on.
fn line_start(grid: &Grid, place: Place) -> Place {
    let mut line = place.line;
    while line > 0 && grid.line(line - 1).is_some_and(|above| above.is_wrapped()) {
        line -= 1;
    }
    Place::new(line, 0)
}

/// The last cell of the wrapped line `place` is on.
fn line_end(grid: &Grid, place: Place) -> Place {
    let mut line = place.line;
    while grid.line(line).is_some_and(|row| row.is_wrapped()) && grid.line(line + 1).is_some() {
        line += 1;
    }
    Place::new(line, grid.cols() - 1)
}
