//! Text the reader can select with the pointer and copy: where each piece of
//! it was drawn, a point in it, and the stretch between two points.

use std::sync::Arc;

use pm_input::FocusId;
use pm_renderer::Rect;
use pm_text::{Reach, Run};

use crate::paint::{Output, Region};
use crate::retain::Retained;

/// How far apart two rows stand at least to be told apart as paragraphs, as a share of the upper row's height.
const PARAGRAPH_GAP: f32 = 0.4;

/// How far apart, in logical pixels, two pieces on one row stand at least to be copied as columns.
const COLUMN_GAP: f32 = 16.0;

/// A place in a selectable text: before byte `byte` of its piece `piece`.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TextPoint {
    /// Which piece of the text, counted in tree order from the first.
    pub piece: usize,
    /// The byte offset in that piece.
    pub byte: usize,
}

/// The stretch of a selectable text between where a drag began and where it is now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextSelection {
    /// The name the text was given with [`crate::Node::selectable`].
    pub id: FocusId,
    /// Where the drag began.
    pub anchor: TextPoint,
    /// Where it is now.
    pub head: TextPoint,
}

impl TextSelection {
    /// The two ends of the stretch, the earlier first.
    pub fn ordered(&self) -> (TextPoint, TextPoint) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }

    /// Whether the stretch holds no text.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The byte range of piece `piece`, `length` bytes long, that the stretch covers, if any.
    pub(crate) fn covering(&self, piece: usize, length: usize) -> Option<std::ops::Range<usize>> {
        let (from, to) = self.ordered();
        if piece < from.piece || piece > to.piece || self.is_empty() {
            return None;
        }
        let start = if piece == from.piece { from.byte } else { 0 };
        let end = if piece == to.piece { to.byte } else { length };
        (start < end).then_some(start..end)
    }
}

/// One line of text inside a selectable box, as it was drawn.
#[derive(Clone, Debug)]
pub struct TextPiece {
    /// The name of the selectable text the piece is part of.
    pub id: FocusId,
    /// Which piece of that text it is, counted in tree order.
    pub index: usize,
    /// What it says.
    pub content: Arc<str>,
    /// Where it was laid out, before any scrolling, which tells the lines apart.
    pub laid: Rect,
    /// Where it was seen and how it was shaped, when it was drawn on screen.
    pub drawn: Option<DrawnPiece>,
}

/// Where a piece of selectable text was drawn on screen, and its shape.
#[derive(Clone, Debug)]
pub struct DrawnPiece {
    /// The part of its box that could be seen.
    pub seen: Rect,
    /// Where the text starts along x, in logical pixels.
    pub x: f32,
    /// The line, shaped.
    pub run: Run,
}

/// One run of the lines of selectable text: a line drawn on its own, or every line of an offscreen retained subtree.
#[derive(Clone, Debug)]
enum Part {
    /// A line recorded as it was painted.
    Piece(TextPiece),
    /// The lines of a subtree that was not painted, found when asked for.
    Hidden {
        /// The subtree, whose text it keeps.
        retained: Arc<Retained>,
        /// Where its top-left corner was.
        origin: (f32, f32),
        /// The selectable text around it.
        outer: Option<FocusId>,
        /// The number of its first piece of each selectable text, when it began.
        starts: Vec<(FocusId, usize)>,
    },
}

/// The lines of selectable text of a frame, in tree order.
///
/// A subtree off screen adds one part however many lines it holds; the lines
/// themselves are put together the first time something reads them, so a frame
/// that nobody selects from costs nothing for the history.
#[derive(Clone, Debug, Default)]
pub struct Pieces {
    /// The parts, in tree order.
    parts: Vec<Part>,
    /// Every line, once asked for.
    all: std::cell::OnceCell<Vec<TextPiece>>,
}

impl Pieces {
    /// Adds a line painted on its own.
    pub(crate) fn push(&mut self, piece: TextPiece) {
        self.parts.push(Part::Piece(piece));
    }

    /// Adds the lines of an offscreen subtree.
    pub(crate) fn push_hidden(
        &mut self,
        retained: Arc<Retained>,
        origin: (f32, f32),
        outer: Option<FocusId>,
        starts: Vec<(FocusId, usize)>,
    ) {
        self.parts.push(Part::Hidden {
            retained,
            origin,
            outer,
            starts,
        });
    }

    /// The lines that were drawn on screen, which are the ones a point can land in.
    pub(crate) fn painted(&self) -> impl DoubleEndedIterator<Item = &TextPiece> {
        self.parts.iter().filter_map(|part| match part {
            Part::Piece(piece) => Some(piece),
            Part::Hidden { .. } => None,
        })
    }

    /// Every line, in tree order.
    pub(crate) fn all(&self) -> &[TextPiece] {
        self.all.get_or_init(|| {
            let mut found = Vec::new();
            for part in &self.parts {
                match part {
                    Part::Piece(piece) => found.push(piece.clone()),
                    Part::Hidden {
                        retained,
                        origin,
                        outer,
                        starts,
                    } => {
                        let Some(hidden) = &retained.hidden else {
                            continue;
                        };
                        let mut next = starts.clone();
                        for text in &hidden.texts {
                            let Some(id) = text.owner.or(*outer) else {
                                continue;
                            };
                            let Some((_, index)) = next.iter_mut().find(|(held, _)| *held == id)
                            else {
                                continue;
                            };
                            found.push(TextPiece {
                                id,
                                index: *index,
                                content: text.content.clone(),
                                laid: text.rect.offset(origin.0, origin.1),
                                drawn: None,
                            });
                            *index += 1;
                        }
                    }
                }
            }
            found
        })
    }
}

impl Output {
    /// Every line of selectable text, in tree order, those off screen included.
    pub fn pieces(&self) -> &[TextPiece] {
        self.pieces.all()
    }

    /// The selectable text under `point`, if a press there would select:
    /// its name, and the place in it nearest the point.
    pub fn text_under(&self, point: (f32, f32)) -> Option<(FocusId, TextPoint)> {
        let area: &Region = self
            .selectables
            .iter()
            .rev()
            .find(|area| area.rect.contains(point.0, point.1))?;
        Some((area.id, self.text_point(area.id, point)?))
    }

    /// The selectable box under `point`: the name of its text, and how many
    /// boxes of that text come before it in tree order.
    pub fn selectable_at(&self, point: (f32, f32)) -> Option<(FocusId, usize)> {
        let at = self
            .selectables
            .iter()
            .rposition(|area| area.rect.contains(point.0, point.1))?;
        let id = self.selectables[at].id;
        let before = self.selectables[..at]
            .iter()
            .filter(|area| area.id == id)
            .count();
        Some((id, before))
    }

    /// A selection of the whole of the selectable text `id`, if it has any text.
    pub fn select_all(&self, id: FocusId) -> Option<TextSelection> {
        let last = self.pieces().iter().rfind(|piece| piece.id == id)?;
        Some(TextSelection {
            id,
            anchor: TextPoint::default(),
            head: TextPoint {
                piece: last.index,
                byte: last.content.len(),
            },
        })
    }

    /// The place in the selectable text `id` nearest `point`, among the
    /// pieces of it that were drawn on screen.
    pub fn text_point(&self, id: FocusId, point: (f32, f32)) -> Option<TextPoint> {
        let (x, y) = point;
        let drawn = || {
            self.pieces
                .painted()
                .filter(move |piece| piece.id == id)
                .filter_map(|piece| piece.drawn.as_ref().map(|drawn| (piece, drawn)))
        };
        let (nearest, shape) =
            drawn().min_by(|a, b| distance(a.1.seen, x, y).total_cmp(&distance(b.1.seen, x, y)))?;
        let mut row = drawn().filter(|(piece, _)| same_row(piece.laid, nearest.laid));
        let point = |piece: &TextPiece, byte| TextPoint {
            piece: piece.index,
            byte,
        };
        if y < shape.seen.y {
            return row.next().map(|(piece, _)| point(piece, 0));
        }
        if y >= shape.seen.y + shape.seen.height {
            return row
                .next_back()
                .map(|(piece, _)| point(piece, piece.content.len()));
        }
        Some(point(
            nearest,
            shape.run.byte_at(x - shape.x, nearest.content.len()),
        ))
    }

    /// The stretch of the selectable text `id` that `unit` takes around
    /// `point`, the earlier end first.
    pub fn unit_around(
        &self,
        id: FocusId,
        point: TextPoint,
        unit: Reach,
    ) -> (TextPoint, TextPoint) {
        let pieces: Vec<&TextPiece> = self
            .pieces()
            .iter()
            .filter(|piece| piece.id == id)
            .collect();
        let Some(at) = pieces.iter().position(|piece| piece.index == point.piece) else {
            return (point, point);
        };
        match unit {
            Reach::Point => (point, point),
            Reach::Word => {
                let word = pm_text::word_at(&pieces[at].content, point.byte);
                (
                    TextPoint {
                        piece: point.piece,
                        byte: word.start,
                    },
                    TextPoint {
                        piece: point.piece,
                        byte: word.end,
                    },
                )
            }
            Reach::Line => {
                let runs_on = |pair: &[&TextPiece]| separator(pair[0], pair[1]) != "\n\n";
                let first = at
                    - pieces[..=at]
                        .windows(2)
                        .rev()
                        .take_while(|pair| runs_on(pair))
                        .count();
                let last = at
                    + pieces[at..]
                        .windows(2)
                        .take_while(|pair| runs_on(pair))
                        .count();
                (
                    TextPoint {
                        piece: pieces[first].index,
                        byte: 0,
                    },
                    TextPoint {
                        piece: pieces[last].index,
                        byte: pieces[last].content.len(),
                    },
                )
            }
        }
    }

    /// What `selection` covers, its lines broken where they were drawn on
    /// separate rows and a blank line left where the rows stood apart.
    pub fn selected_text(&self, selection: &TextSelection) -> String {
        let mut text = String::new();
        let mut last: Option<&TextPiece> = None;
        for piece in self
            .pieces()
            .iter()
            .filter(|piece| piece.id == selection.id)
        {
            let Some(range) = selection.covering(piece.index, piece.content.len()) else {
                continue;
            };
            if let Some(before) = last {
                text.push_str(separator(before, piece));
            }
            text.push_str(&piece.content[range]);
            last = Some(piece);
        }
        text.trim_end().to_owned()
    }
}

/// How far `rect` lies from the point `x`, `y`, rows weighing far more
/// than columns so the nearest piece is found on the nearest row first.
fn distance(rect: Rect, x: f32, y: f32) -> f32 {
    let across = (rect.x - x).max(x - (rect.x + rect.width)).max(0.0);
    let down = (rect.y - y).max(y - (rect.y + rect.height)).max(0.0);
    down * 1.0e4 + across
}

/// Whether `a` and `b` were laid out on one row: their heights overlap.
fn same_row(a: Rect, b: Rect) -> bool {
    a.y < b.y + b.height && b.y < a.y + a.height
}

/// What goes between `before` and `after` when they are copied together:
/// on one row nothing where they touch, a space where they stand apart and
/// a tab where they stand as far apart as columns do; between rows nothing
/// where a row was wrapped after a space, a line break, and a blank line
/// between rows set apart.
fn separator(before: &TextPiece, after: &TextPiece) -> &'static str {
    let bottom = before.laid.y + before.laid.height;
    if after.laid.y < bottom - 0.5 {
        let apart = after.laid.x - (before.laid.x + before.laid.width);
        return match apart {
            _ if apart > COLUMN_GAP => "\t",
            _ if apart > 1.0 => " ",
            _ => "",
        };
    }
    if after.laid.y - bottom > before.laid.height * PARAGRAPH_GAP {
        return "\n\n";
    }
    if before.content.ends_with(' ') {
        return "";
    }
    "\n"
}
