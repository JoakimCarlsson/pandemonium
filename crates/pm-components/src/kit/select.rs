//! Text that can be selected with the pointer across many boxes, as one text.
//!
//! Every [`selectable`] line given one id is a piece of one text, counted in
//! paint order, so a selection runs from one line into the next. The host
//! keeps the [`TextSelection`] (pressing, dragging, double and triple clicks
//! by [`Reach`](pm_field::Reach)) and hands it back in the
//! [`Frame`](crate::kit::Frame); each piece highlights what it covers. The
//! frame's [`Report`](crate::kit::Report) finds places in the text and copies
//! what a selection covers.

use std::cell::RefCell;
use std::rc::Rc;

use rustle::{Modifier, Offset, Rect, TextLayout};

use pm_field::Reach;

use crate::kit::focus::FocusId;
use crate::kit::frame;
use crate::kit::report;
use crate::kit::text::Type;
use crate::kit::theme::{self, Role, Tone};

/// How far apart, in logical pixels, two pieces on one row stand to be copied with a tab between them.
const COLUMN_GAP: f32 = 24.0;

/// How far apart two rows stand, as a share of a row's height, to be copied with a blank line between them.
const PARAGRAPH_GAP: f32 = 0.5;

/// A place in a selectable text: a piece, and a byte offset in its content.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TextPoint {
    /// Which piece, counted in paint order among the pieces of its text.
    pub piece: usize,
    /// The byte offset in the piece's content.
    pub byte: usize,
}

/// A stretch of a selectable text, from where a press began to where the pointer is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextSelection {
    /// The text.
    pub id: FocusId,
    /// Where the press began.
    pub anchor: TextPoint,
    /// Where the selection ends now.
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
    pub fn covering(&self, piece: usize, length: usize) -> Option<std::ops::Range<usize>> {
        let (from, to) = self.ordered();
        if piece < from.piece || piece > to.piece || self.is_empty() {
            return None;
        }
        let start = if piece == from.piece { from.byte } else { 0 };
        let end = if piece == to.piece { to.byte } else { length };
        (start < end).then_some(start..end)
    }
}

/// One line of a selectable text as it was painted.
#[derive(Clone, Debug)]
pub struct Piece {
    /// The text it is part of.
    pub id: FocusId,
    /// Which piece of that text it is, in paint order.
    pub index: usize,
    /// What it says.
    pub content: Rc<str>,
    /// Where it was painted in the window.
    pub rect: Rect,
    /// Its shaped text.
    pub layout: Rc<TextLayout>,
}

/// The selectable text the views being composed belong to, if any.
static SCOPE: rustle::CompositionLocal<Option<FocusId>> = rustle::CompositionLocal::new(|| None);

/// Composes `content` with every line of text it emits a piece of the
/// selectable text `id`, in the order they are painted: a box made
/// `.selectable(id)`, whose text lines, from labels, paragraphs and marked
/// lines alike, a selection runs through.
#[track_caller]
pub fn selectable_scope(id: FocusId, content: impl FnOnce()) {
    SCOPE.provide(Some(id), content);
}

/// The selectable text the line being composed belongs to, if it is inside
/// a [`selectable_scope`].
pub(crate) fn scope() -> Option<FocusId> {
    if rustle::is_composing() {
        SCOPE.current()
    } else {
        None
    }
}

/// `modifier` recording the text it holds, `content`, whose shaped layout
/// `shaped` holds once laid out, as a piece of the selectable text `id`, and
/// highlighting behind it the part of it the frame's selection covers.
pub(crate) fn piece(
    modifier: Modifier,
    id: FocusId,
    content: Rc<str>,
    shaped: Rc<RefCell<Option<Rc<TextLayout>>>>,
) -> Modifier {
    let index: Rc<RefCell<usize>> = Rc::default();
    let highlight = theme::color(Tone::from(Role::Accent).alpha(0.3));
    let (placed, numbered) = (shaped.clone(), index.clone());
    modifier
        .on_placed(move |rect, _| {
            let Some(layout) = placed.borrow().clone() else {
                return;
            };
            report::record(|report| {
                let next = report.pieces.iter().filter(|piece| piece.id == id).count();
                *numbered.borrow_mut() = next;
                report.pieces.push(Piece {
                    id,
                    index: next,
                    content: content.clone(),
                    rect,
                    layout,
                });
            });
        })
        .draw_behind(move |canvas| {
            let Some(layout) = shaped.borrow().clone() else {
                return;
            };
            let Some(selection) = frame::current().selection.filter(|held| held.id == id) else {
                return;
            };
            let length = layout.text().len();
            if let Some(range) = selection.covering(*index.borrow(), length) {
                canvas.selection(0.0, 0.0, &layout, range.start, range.end, highlight);
            }
        })
}

/// Emits one line of `content` set in `kind` as a piece of the selectable
/// text `id`, highlighting the part of it the frame's selection covers:
/// `text(content).styles([...])` inside a box made `.selectable(id)`.
#[track_caller]
pub fn selectable(modifier: Modifier, id: FocusId, content: impl Into<String>, kind: Type) {
    let content: String = content.into();
    let shaped: Rc<RefCell<Option<Rc<TextLayout>>>> = Rc::default();
    let laid = shaped.clone();
    let modifier = piece(
        modifier.on_text_layout(move |layout| *laid.borrow_mut() = Some(layout)),
        id,
        Rc::from(content.as_str()),
        shaped,
    );
    rustle::text_styled(modifier, content, kind.style());
}

/// The place in the selectable text `id` nearest the window point `at`, among its painted pieces.
pub(crate) fn point_in(pieces: &[Piece], id: FocusId, at: (f32, f32)) -> Option<TextPoint> {
    let (x, y) = at;
    let mine = || pieces.iter().filter(move |piece| piece.id == id);
    let nearest =
        mine().min_by(|a, b| distance(a.rect, x, y).total_cmp(&distance(b.rect, x, y)))?;
    let mut row = mine().filter(|piece| same_row(piece.rect, nearest.rect));
    let point = |piece: &Piece, byte| TextPoint {
        piece: piece.index,
        byte,
    };
    if y < nearest.rect.origin.y {
        return row.next().map(|piece| point(piece, 0));
    }
    if y >= nearest.rect.origin.y + nearest.rect.size.height {
        return row
            .next_back()
            .map(|piece| point(piece, piece.content.len()));
    }
    let byte = nearest.layout.hit_test(Offset::new(
        x - nearest.rect.origin.x,
        y - nearest.rect.origin.y,
    ));
    Some(point(nearest, byte))
}

/// A selection of every painted piece of the selectable text `id`.
pub(crate) fn all_of(pieces: &[Piece], id: FocusId) -> Option<TextSelection> {
    let last = pieces.iter().rfind(|piece| piece.id == id)?;
    Some(TextSelection {
        id,
        anchor: TextPoint::default(),
        head: TextPoint {
            piece: last.index,
            byte: last.content.len(),
        },
    })
}

/// The word, or the run of rows up to a blank line, of the selectable text
/// `id` around `point`, as `reach` says, among its painted `pieces`.
pub(crate) fn unit_around(
    pieces: &[Piece],
    id: FocusId,
    point: TextPoint,
    reach: Reach,
) -> (TextPoint, TextPoint) {
    let mine: Vec<&Piece> = pieces.iter().filter(|piece| piece.id == id).collect();
    let Some(at) = mine.iter().position(|piece| piece.index == point.piece) else {
        return (point, point);
    };
    let place = |piece: &Piece, byte| TextPoint {
        piece: piece.index,
        byte,
    };
    match reach {
        Reach::Point => (point, point),
        Reach::Word => {
            let word = pm_field::word_at(&mine[at].content, point.byte);
            (place(mine[at], word.start), place(mine[at], word.end))
        }
        Reach::Line => {
            let runs_on = |pair: &[&Piece]| separator(pair[0], pair[1]) != "\n\n";
            let first = at
                - mine[..=at]
                    .windows(2)
                    .rev()
                    .take_while(|pair| runs_on(pair))
                    .count();
            let last = at
                + mine[at..]
                    .windows(2)
                    .take_while(|pair| runs_on(pair))
                    .count();
            (
                place(mine[first], 0),
                place(mine[last], mine[last].content.len()),
            )
        }
    }
}

/// What `selection` covers of `pieces`, broken where the pieces were drawn on separate rows.
pub(crate) fn text_of(pieces: &[Piece], selection: &TextSelection) -> String {
    let mut text = String::new();
    let mut last: Option<&Piece> = None;
    for piece in pieces.iter().filter(|piece| piece.id == selection.id) {
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

/// How far `rect` lies from the point `x`, `y`, rows weighing far more than
/// columns so the nearest piece is found on the nearest row first.
fn distance(rect: Rect, x: f32, y: f32) -> f32 {
    let across = (rect.origin.x - x)
        .max(x - (rect.origin.x + rect.size.width))
        .max(0.0);
    let down = (rect.origin.y - y)
        .max(y - (rect.origin.y + rect.size.height))
        .max(0.0);
    down * 1.0e4 + across
}

/// Whether `a` and `b` were laid out on one row: their heights overlap.
fn same_row(a: Rect, b: Rect) -> bool {
    a.origin.y < b.origin.y + b.size.height && b.origin.y < a.origin.y + a.size.height
}

/// What goes between `before` and `after` when they are copied together:
/// on one row nothing where they touch, a space or a tab where they stand
/// apart; between rows a line break, or a blank line between rows set apart.
fn separator(before: &Piece, after: &Piece) -> &'static str {
    let bottom = before.rect.origin.y + before.rect.size.height;
    if after.rect.origin.y < bottom - 0.5 {
        let apart = after.rect.origin.x - (before.rect.origin.x + before.rect.size.width);
        return match apart {
            _ if apart > COLUMN_GAP => "\t",
            _ if apart > 1.0 => " ",
            _ => "",
        };
    }
    if after.rect.origin.y - bottom > before.rect.size.height * PARAGRAPH_GAP {
        return "\n\n";
    }
    if before.content.ends_with(' ') {
        return "";
    }
    "\n"
}
