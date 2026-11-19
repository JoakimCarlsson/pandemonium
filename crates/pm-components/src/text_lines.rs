//! Where the lines of the code text were drawn, held by the state that owns
//! the text, as a Compose state holder keeps what `onTextLayout` and
//! `onGloballyPositioned` told it.
//!
//! The code editor draws its text line by line, each line, or piece of a
//! line between inlay hints, a marked text. Each piece tells a [`TextLines`]
//! its shaped layout and where it was placed, and the holder answers what the
//! app asks of the text as drawn: the byte under a point, the byte a line up
//! or down, the ends of a line as drawn, a step along a line holding
//! right-to-left text, and the caret's rectangle for the input method.
//! Places are logical pixels from the top left of the area that starts each
//! frame with [`TextLines::frame`], and bytes count into the text of the
//! field that drew the piece.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use rustle::{Affinity, Modifier, Offset, Rect, TextLayout};

use crate::focus::FocusId;

/// Where the caret goes when it is moved one place along the screen in a
/// line that holds right-to-left text: the step of a caret along the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// To this byte of the text.
    To(usize),
    /// Off the end of the line the way it was moved.
    Edge,
}

/// One line, or one piece of a line, of a text drawn line by line.
#[derive(Clone, Debug)]
pub struct DrawnLine {
    /// The field whose text it is.
    pub owner: FocusId,
    /// The byte of the field's text the piece starts at.
    pub start: usize,
    /// The piece's text, shaped as it was drawn.
    pub layout: Rc<TextLayout>,
    /// Where the piece was placed.
    pub rect: Rect,
}

impl DrawnLine {
    /// The byte one past the end of the piece.
    fn end(&self) -> usize {
        self.start + self.layout.text().len()
    }

    /// Whether the byte `caret` lies on the piece, its end included.
    fn holds(&self, caret: usize) -> bool {
        self.start <= caret && caret <= self.end()
    }

    /// The byte of the field nearest the point `x`, `y`.
    pub fn offset_at(&self, x: f32, y: f32) -> usize {
        self.start
            + self
                .layout
                .hit_test(Offset::new(x - self.rect.origin.x, y - self.rect.origin.y))
    }

    /// Where the caret at byte `caret` of the field is drawn: x, top, height.
    pub fn caret_at(&self, caret: usize) -> (f32, f32, f32) {
        let at = self
            .layout
            .caret(caret.saturating_sub(self.start), Affinity::Downstream);
        (
            self.rect.origin.x + at.x,
            self.rect.origin.y + at.y,
            at.height,
        )
    }
}

/// What a [`TextLines`] holds.
#[derive(Default)]
struct Held {
    /// The pieces drawn in the latest frame, in paint order.
    lines: RefCell<Vec<DrawnLine>>,
    /// Where the area that starts each frame was placed in its window.
    origin: Cell<Offset>,
}

/// Where the lines of a text drawn line by line were placed in the latest
/// frame, each told by the line as it is placed. Two handles are equal when
/// they are the same holder.
#[derive(Clone, Default)]
pub struct TextLines(Rc<Held>);

impl PartialEq for TextLines {
    /// Whether both handles are the same holder.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for TextLines {
    /// How many pieces were drawn.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextLines")
            .field("lines", &self.0.lines.borrow().len())
            .finish()
    }
}

impl TextLines {
    /// `modifier` starting a fresh frame of lines each time its area is
    /// placed, before the lines inside it are, and measuring every line from
    /// the area's top left.
    pub fn frame(&self, modifier: Modifier) -> Modifier {
        let held = Rc::clone(&self.0);
        modifier.on_placed(move |rect, _| {
            held.origin.set(rect.origin);
            held.lines.borrow_mut().clear();
        })
    }

    /// `modifier` telling the holder that its area, the piece of the text of
    /// `owner` starting at byte `start`, was placed with the layout `shaped`
    /// holds by then.
    pub fn line(
        &self,
        modifier: Modifier,
        owner: FocusId,
        start: usize,
        shaped: Rc<RefCell<Option<Rc<TextLayout>>>>,
    ) -> Modifier {
        let held = Rc::clone(&self.0);
        modifier.on_placed(move |rect, _| {
            let Some(layout) = shaped.borrow().clone() else {
                return;
            };
            let origin = held.origin.get();
            let rect = Rect::new(
                Offset::new(rect.origin.x - origin.x, rect.origin.y - origin.y),
                rect.size,
            );
            held.lines.borrow_mut().push(DrawnLine {
                owner,
                start,
                layout,
                rect,
            });
        })
    }

    /// The pieces of `owner` drawn in the latest frame, in paint order.
    pub fn of(&self, owner: FocusId) -> Vec<DrawnLine> {
        self.0
            .lines
            .borrow()
            .iter()
            .filter(|line| line.owner == owner)
            .cloned()
            .collect()
    }

    /// The pieces of `owner`, grouped into rows from the top, each row from the left.
    fn rows_of(&self, owner: FocusId) -> Vec<Vec<DrawnLine>> {
        let mut pieces = self.of(owner);
        pieces.sort_by(|a, b| {
            a.rect
                .origin
                .y
                .total_cmp(&b.rect.origin.y)
                .then(a.rect.origin.x.total_cmp(&b.rect.origin.x))
        });
        let mut rows: Vec<Vec<DrawnLine>> = Vec::new();
        for piece in pieces {
            match rows.last_mut() {
                Some(row) if (row[0].rect.origin.y - piece.rect.origin.y).abs() < 0.5 => {
                    row.push(piece)
                }
                _ => rows.push(vec![piece]),
            }
        }
        rows
    }

    /// The rows of `owner`, the place from the top of the one holding the
    /// byte `caret`, and the piece holding it.
    fn row_holding(
        &self,
        owner: FocusId,
        caret: usize,
    ) -> Option<(Vec<Vec<DrawnLine>>, usize, DrawnLine)> {
        let rows = self.rows_of(owner);
        let (index, piece) = rows.iter().enumerate().find_map(|(index, row)| {
            row.iter()
                .find(|piece| piece.holds(caret))
                .map(|piece| (index, piece.clone()))
        })?;
        Some((rows, index, piece))
    }

    /// The byte of `owner` nearest the point `x`, `y`: on the row the point
    /// is level with, or the last row below it.
    pub fn offset_in(&self, owner: FocusId, x: f32, y: f32) -> Option<usize> {
        let rows = self.rows_of(owner);
        let row = rows
            .iter()
            .find(|row| y < row[0].rect.origin.y + row[0].rect.size.height)
            .or(rows.last())?;
        let piece = row
            .iter()
            .find(|piece| x < piece.rect.origin.x + piece.rect.size.width)
            .or(row.last())?;
        Some(piece.offset_at(
            x,
            y.clamp(
                piece.rect.origin.y,
                piece.rect.origin.y + piece.rect.size.height - 0.5,
            ),
        ))
    }

    /// The byte `by` rows below the byte `caret` of `owner`, as its rows are
    /// drawn, keeping the caret's column; `None` past the first or the last row.
    pub fn vertical(&self, owner: FocusId, caret: usize, by: isize) -> Option<usize> {
        let (rows, index, piece) = self.row_holding(owner, caret)?;
        let (x, _, _) = piece.caret_at(caret);
        let row = rows.get(index.checked_add_signed(by)?)?;
        let target = row
            .iter()
            .find(|piece| x < piece.rect.origin.x + piece.rect.size.width)
            .or(row.last())?;
        let y = target.rect.origin.y + target.rect.size.height / 2.0;
        Some(target.offset_at(x, y))
    }

    /// The byte at the start, or the end when `end` is set, of the row the
    /// byte `caret` of `owner` is on, as drawn.
    pub fn line_edge(&self, owner: FocusId, caret: usize, end: bool) -> Option<usize> {
        let (rows, index, _) = self.row_holding(owner, caret)?;
        let row = &rows[index];
        if end {
            row.iter().map(DrawnLine::end).max()
        } else {
            row.iter().map(|piece| piece.start).min()
        }
    }

    /// Where the caret at byte `caret` of `owner` goes when moved one place
    /// right, or left, along the screen, when its row holds right-to-left
    /// text; `None` when the row runs left to right throughout, so moving by
    /// bytes is right.
    pub fn visual_step(&self, owner: FocusId, caret: usize, right: bool) -> Option<Step> {
        let (_, _, piece) = self.row_holding(owner, caret)?;
        match step_in(&piece.layout, caret - piece.start, right)? {
            Step::To(byte) => Some(Step::To(piece.start + byte)),
            Step::Edge => Some(Step::Edge),
        }
    }

    /// The caret of `owner` at byte `caret` as a rectangle one pixel wide,
    /// for the input method's candidate window.
    pub fn caret_rect(&self, owner: FocusId, caret: usize) -> Option<Rect> {
        let (x, y, height) = self.row_holding(owner, caret)?.2.caret_at(caret);
        Some(Rect::new(Offset::new(x, y), rustle::Size::new(1.0, height)))
    }
}

/// Where the caret at byte `caret` of `layout` goes when moved one place
/// right, or left, along the screen, when its line holds right-to-left
/// text; `None` when the line runs left to right throughout.
fn step_in(layout: &TextLayout, caret: usize, right: bool) -> Option<Step> {
    let line = layout
        .lines()
        .get(layout.line_of_offset(caret, Affinity::Downstream))?;
    if !line.clusters.iter().any(|cluster| cluster.rtl) {
        return None;
    }
    let text = layout.text();
    let mut stops: Vec<(f32, usize)> = (line.start..=line.end)
        .filter(|offset| text.is_char_boundary(*offset))
        .map(|offset| (layout.caret(offset, Affinity::Downstream).x, offset))
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let here_x = layout.caret(caret, Affinity::Downstream).x;
    let here = stops.iter().position(|stop| stop.1 == caret).or_else(|| {
        stops
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1.0 - here_x).abs().total_cmp(&(b.1.0 - here_x).abs()))
            .map(|(place, _)| place)
    })?;
    let next = if right {
        Some(here + 1)
    } else {
        here.checked_sub(1)
    };
    Some(
        next.and_then(|next| stops.get(next))
            .map_or(Step::Edge, |stop| Step::To(stop.1)),
    )
}
