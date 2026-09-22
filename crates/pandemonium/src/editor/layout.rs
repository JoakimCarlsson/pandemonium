//! Where a pane last drew a document, in the terms a pointer arrives in.
//!
//! Only the pane knows how wide a character came out, how much of it the
//! gutter took and how far along the line it had scrolled — and the window
//! is the one that hears about a click, a hover or a wheel notch. So the
//! pane writes down what it drew and the window reads places off it: one
//! measurement, agreed on by the two ends of every pointer gesture.

use pm_gfx::{Point, Rect, Size};

/// How far the gutter's numbers sit from the text.
pub const GUTTER_GAP: f32 = 16.0;

/// How far the gutter's numbers sit from the edge of the pane.
pub const GUTTER_INSET: f32 = 12.0;

/// Shortest a line number column is, in digits.
pub const GUTTER_DIGITS: usize = 2;

/// How wide the blame column is, in characters.
pub const BLAME_WIDTH: usize = 26;

/// How wide the column holding the fold markers is.
pub const FOLD_WIDTH: f32 = 14.0;

/// Where a document was drawn, and what one character of it came to.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextLayout {
    /// The pane the text was drawn in.
    pub bounds: Rect,
    /// The extent of one character.
    pub cell: Size,
    /// How much of the pane the gutter took on the left, blame included.
    pub gutter: f32,
    /// How much of that gutter the blame column took, at its right edge.
    pub blame: f32,
    /// The first line the pane showed.
    pub first: usize,
    /// The first column the pane showed, the text being scrolled left by it.
    pub column: usize,
}

impl TextLayout {
    /// The top of the `row`-th line drawn, when the pane has room for it.
    pub fn top_of(&self, row: usize) -> Option<f32> {
        let top = self.top_at(row);
        (top < self.bounds.bottom()).then_some(top)
    }

    /// The top of the `row`-th line drawn, whether or not there is room.
    pub fn top_at(&self, row: usize) -> f32 {
        self.bounds.top() + row as f32 * self.cell.height
    }

    /// The left edge of the column drawn at `column`.
    pub fn x_of(&self, column: usize) -> f32 {
        self.text_left() + (column as f32 - self.column as f32) * self.cell.width
    }

    /// Where the text begins, the gutter being to the left of it.
    pub fn text_left(&self) -> f32 {
        self.bounds.left() + self.gutter
    }

    /// The part of the pane the text itself is drawn in.
    pub fn text_area(&self) -> Rect {
        Rect::from_xywh(
            self.text_left(),
            self.bounds.top(),
            (self.bounds.size.width - self.gutter).max(0.0),
            self.bounds.size.height,
        )
    }

    /// How many whole lines the pane has room for.
    pub fn rows(&self) -> usize {
        (self.bounds.size.height / self.cell.height.max(1.0))
            .floor()
            .max(1.0) as usize
    }

    /// How many whole columns of text the pane has room for.
    pub fn columns(&self) -> usize {
        (self.text_area().size.width / self.cell.width.max(1.0))
            .floor()
            .max(1.0) as usize
    }

    /// The row `point` falls on, counted from the first one drawn.
    pub fn row_at(&self, point: Point) -> usize {
        let row = ((point.y - self.bounds.top()) / self.cell.height.max(1.0)).floor();
        row.max(0.0) as usize
    }

    /// The drawn column `point` falls on, rounded to the nearer character edge.
    pub fn column_at(&self, point: Point) -> usize {
        let offset = (point.x - self.text_left()) / self.cell.width.max(1.0);
        self.column + offset.round().max(0.0) as usize
    }

    /// The drawn column `point` is over, rather than the nearer edge of one.
    ///
    /// A caret is placed between two characters, so a press rounds; asking
    /// what the pointer is over aims at a character, and the right half of
    /// the last letter of a name is still that name.
    pub fn column_under(&self, point: Point) -> usize {
        let offset = (point.x - self.text_left()) / self.cell.width.max(1.0);
        self.column + offset.floor().max(0.0) as usize
    }

    /// Where the blame column begins, when one is being drawn.
    pub fn blame_left(&self) -> f32 {
        self.text_left() - self.blame
    }

    /// Where the column holding the fold markers begins.
    pub fn fold_left(&self) -> f32 {
        self.blame_left() - FOLD_WIDTH
    }

    /// Whether `point` is over that column rather than over the numbers.
    pub fn over_folds(&self, point: Point) -> bool {
        point.x >= self.fold_left() && point.x < self.blame_left()
    }

    /// How far the text sits from the edge when there is no gutter at all.
    pub fn plain_gutter() -> f32 {
        GUTTER_GAP
    }

    /// How wide a gutter numbering `lines` lines needs.
    pub fn gutter_for(lines: usize, cell: Size) -> f32 {
        let digits = lines.to_string().len().max(GUTTER_DIGITS);
        GUTTER_INSET * 2.0 + digits as f32 * cell.width + GUTTER_GAP + FOLD_WIDTH
    }

    /// How wide a blame column is, when one is being drawn.
    pub fn blame_for(shown: bool, cell: Size) -> f32 {
        if shown {
            BLAME_WIDTH as f32 * cell.width + GUTTER_GAP
        } else {
            0.0
        }
    }
}
