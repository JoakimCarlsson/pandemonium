//! Text that can be selected with the pointer across many boxes, as one text.
//!
//! Every [`selectable`] line given one id is a piece of one text, counted in
//! paint order, so a selection runs from one line into the next. The pieces
//! register themselves, with their shaped text and where they were placed, in
//! the [`Selectables`] of the state that owns the screen, provided to the views
//! with [`Selectables::provide`]: Compose's `SelectionRegistrar`, which a
//! `SelectionContainer` provides to the text inside it. The app keeps the
//! [`TextSelection`] (pressing, dragging, double and triple clicks by
//! [`Reach`](pm_field::Reach)) and hands it to the same holder; each piece
//! highlights what it covers, and the holder finds places in the text and
//! copies what a selection covers.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle::{Modifier, Offset, Rect, TextLayout, TextStyle};

use pm_field::Reach;

use crate::focus::FocusId;
use crate::theme::{self, Role, Tone};

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

/// The selectable texts of a screen as they were last placed, and the stretch of them
/// selected: Compose's `SelectionRegistrar` held by the state that owns the screen.
///
/// The area that starts each frame with [`frame`](Self::frame) forgets the pieces placed
/// before; every piece composed inside [`provide`](Self::provide) registers itself as it
/// is placed, in paint order. Places are logical pixels from the top left of that area.
#[derive(Clone, Default)]
pub struct Selectables {
    /// The pieces placed in the last frame, in paint order.
    pieces: Rc<RefCell<Vec<Piece>>>,
    /// The boxes of the selectable texts placed in the last frame, by the text they are
    /// part of, in paint order.
    areas: Rc<RefCell<Vec<(FocusId, Rect)>>>,
    /// Where the area that starts each frame was placed in the window.
    origin: Rc<Cell<Offset>>,
    /// The stretch selected, which the pieces highlight.
    selection: Rc<Cell<Option<TextSelection>>>,
}

impl PartialEq for Selectables {
    /// Whether both are the same holder.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.pieces, &other.pieces)
    }
}

impl std::fmt::Debug for Selectables {
    /// How many pieces were placed and what is selected.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Selectables")
            .field("pieces", &self.pieces.borrow().len())
            .field("selection", &self.selection.get())
            .finish()
    }
}

/// The holder the pieces composed below register in.
static REGISTRAR: rustle::CompositionLocal<Option<Selectables>> =
    rustle::CompositionLocal::new(|| None);

impl Selectables {
    /// `modifier` on the area that holds the selectable texts: each time it is placed, the
    /// pieces of the frame before are forgotten and places are measured from its corner.
    pub fn frame(&self, modifier: Modifier) -> Modifier {
        let (pieces, areas, origin) = (
            Rc::clone(&self.pieces),
            Rc::clone(&self.areas),
            Rc::clone(&self.origin),
        );
        modifier.on_placed(move |rect, _| {
            pieces.borrow_mut().clear();
            areas.borrow_mut().clear();
            origin.set(rect.origin);
        })
    }

    /// Composes `content` with its selectable text registering in this holder.
    pub fn provide<R>(&self, content: impl FnOnce() -> R) -> R {
        REGISTRAR.provide(Some(self.clone()), content)
    }

    /// Selects `selection`, or nothing; the pieces highlight it when next painted.
    pub fn select(&self, selection: Option<TextSelection>) {
        self.selection.set(selection);
    }

    /// `rect`, placed in the window, measured from the corner of the frame's area.
    fn local(&self, rect: Rect) -> Rect {
        let origin = self.origin.get();
        Rect::new(
            Offset::new(rect.origin.x - origin.x, rect.origin.y - origin.y),
            rect.size,
        )
    }

    /// Records a piece placed at `rect` in the window, and returns its index among the
    /// pieces of its text.
    fn register(&self, id: FocusId, content: Rc<str>, rect: Rect, layout: Rc<TextLayout>) -> usize {
        let rect = self.local(rect);
        let mut pieces = self.pieces.borrow_mut();
        let index = pieces.iter().filter(|piece| piece.id == id).count();
        pieces.push(Piece {
            id,
            index,
            content,
            rect,
            layout,
        });
        index
    }

    /// Records a box of the selectable text `id` placed at `rect` in the window.
    fn register_area(&self, id: FocusId, rect: Rect) {
        let rect = self.local(rect);
        self.areas.borrow_mut().push((id, rect));
    }

    /// The box of a selectable text under the point `at`: the name of its text, and how
    /// many boxes of that text were placed before it.
    pub fn selectable_at(&self, at: (f32, f32)) -> Option<(FocusId, usize)> {
        let areas = self.areas.borrow();
        let index = areas.iter().rposition(|(_, rect)| contains(*rect, at))?;
        let id = areas[index].0;
        let before = areas[..index]
            .iter()
            .filter(|(held, _)| *held == id)
            .count();
        Some((id, before))
    }

    /// The first box of the selectable text `id` that was placed.
    pub fn area(&self, id: FocusId) -> Option<Rect> {
        self.areas
            .borrow()
            .iter()
            .find(|(held, _)| *held == id)
            .map(|(_, rect)| *rect)
    }

    /// The word, or the run of lines up to a blank line, of the selectable text `id`
    /// around the place `point`, as `reach` says.
    pub fn unit_around(
        &self,
        id: FocusId,
        point: TextPoint,
        reach: Reach,
    ) -> (TextPoint, TextPoint) {
        unit_around(&self.pieces.borrow(), id, point, reach)
    }

    /// The selectable text under the point `at`: its name and the place in it nearest the point.
    pub fn text_under(&self, at: (f32, f32)) -> Option<(FocusId, TextPoint)> {
        let id = self
            .pieces
            .borrow()
            .iter()
            .rev()
            .find(|piece| contains(piece.rect, at))?
            .id;
        Some((id, self.text_point(id, at)?))
    }

    /// The place in the selectable text `id` nearest the point `at`.
    pub fn text_point(&self, id: FocusId, at: (f32, f32)) -> Option<TextPoint> {
        point_in(&self.pieces.borrow(), id, at)
    }

    /// A selection of the whole of the selectable text `id` that was placed.
    pub fn select_all(&self, id: FocusId) -> Option<TextSelection> {
        all_of(&self.pieces.borrow(), id)
    }

    /// What `selection` covers, its lines broken where they were drawn on separate rows.
    pub fn selected_text(&self, selection: &TextSelection) -> String {
        text_of(&self.pieces.borrow(), selection)
    }

    /// The text of every piece placed, in paint order, a line each.
    pub fn placed_text(&self) -> Vec<String> {
        self.pieces
            .borrow()
            .iter()
            .map(|piece| piece.content.to_string())
            .collect()
    }
}

/// Whether the point `at` lies inside `rect`.
fn contains(rect: Rect, at: (f32, f32)) -> bool {
    at.0 >= rect.origin.x
        && at.1 >= rect.origin.y
        && at.0 < rect.origin.x + rect.size.width
        && at.1 < rect.origin.y + rect.size.height
}

/// The modifier that registers the area as one box of the selectable text `id`, such as
/// one message of a conversation, so the app can tell which box a point is in.
pub fn selectable_area(modifier: Modifier, id: FocusId) -> Modifier {
    let Some(registrar) = REGISTRAR.current() else {
        return modifier.test_tag(id.tag());
    };
    modifier
        .test_tag(id.tag())
        .on_placed_visible(move |rect| registrar.register_area(id, rect))
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
/// highlighting behind it the part of it the holder's selection covers.
pub(crate) fn piece(
    modifier: Modifier,
    id: FocusId,
    content: Rc<str>,
    shaped: Rc<RefCell<Option<Rc<TextLayout>>>>,
) -> Modifier {
    let Some(registrar) = REGISTRAR.current() else {
        return modifier;
    };
    let index: Rc<Cell<usize>> = Rc::default();
    let highlight = theme::color(Tone::from(Role::Focused).alpha(0.45));
    let (placed, numbered, holder) = (shaped.clone(), index.clone(), registrar.clone());
    modifier
        .on_placed(move |rect, _| {
            let Some(layout) = placed.borrow().clone() else {
                return;
            };
            numbered.set(holder.register(id, content.clone(), rect, layout));
        })
        .draw_behind(move |canvas| {
            let Some(layout) = shaped.borrow().clone() else {
                return;
            };
            let Some(selection) = registrar.selection.get().filter(|held| held.id == id) else {
                return;
            };
            let length = layout.text().len();
            if let Some(range) = selection.covering(index.get(), length) {
                canvas.selection(0.0, 0.0, &layout, range.start, range.end, highlight);
            }
        })
}

/// Emits `content` set in `style` as a piece of the selectable text `id`,
/// highlighting the part of it the frame's selection covers.
#[track_caller]
pub fn selectable(modifier: Modifier, id: FocusId, content: impl Into<String>, style: TextStyle) {
    let content: String = content.into();
    let shaped: Rc<RefCell<Option<Rc<TextLayout>>>> = Rc::default();
    let laid = shaped.clone();
    let modifier = piece(
        modifier.on_text_layout(move |layout| *laid.borrow_mut() = Some(layout)),
        id,
        Rc::from(content.as_str()),
        shaped,
    );
    rustle::text_styled(modifier, content, style);
}

/// Emits `content` set in `style` with rustle's text composables, as Compose's
/// `Text` inside a `SelectionContainer`: inside a [`selectable_scope`] the
/// text is a piece of that selectable text, elsewhere it is plain text.
///
/// A style of at most one line (`max_lines` of one), neither underlined nor
/// struck through, of one line of content, is cut short at its box with an
/// ellipsis ([`rustle::text_ellipsized`]), a selection running over the whole
/// of it; any other style is set as it says ([`rustle::text_styled`]),
/// wrapping at its box when its wrap says so.
#[track_caller]
pub fn styled_text(modifier: Modifier, content: impl Into<String>, style: TextStyle) {
    let content: String = content.into();
    let scope = scope();
    if style.max_lines == Some(1)
        && !style.underline
        && !style.strikethrough
        && !content.contains('\n')
    {
        return shortened(modifier, content, style, scope);
    }
    if let Some(id) = scope {
        return selectable(modifier, id, content, style);
    }
    rustle::text_styled(modifier, content, style);
}

/// Emits one line of `content` set in `style`, cut short with an ellipsis at
/// its box, a piece of the selectable text `scope` names when there is one.
#[track_caller]
fn shortened(modifier: Modifier, content: String, style: TextStyle, scope: Option<FocusId>) {
    let modifier = match scope {
        Some(id) => {
            let whole = TextStyle {
                max_lines: None,
                ..style
            };
            let shaped = rustle::layout_text(&content, &whole, None);
            let shaped = Rc::new(RefCell::new(Some(Rc::new(shaped))));
            piece(modifier, id, Rc::from(content.as_str()), shaped)
        }
        None => modifier,
    };
    rustle::text_ellipsized(modifier, content, style);
}

/// The place in the selectable text `id` nearest the window point `at`, among its painted pieces.
fn point_in(pieces: &[Piece], id: FocusId, at: (f32, f32)) -> Option<TextPoint> {
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
fn all_of(pieces: &[Piece], id: FocusId) -> Option<TextSelection> {
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
fn unit_around(
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

/// What `selection` covers of `pieces`, broken where the pieces were drawn on
/// separate rows; a piece the selection's ends no longer fall inside, since
/// the text changed under it, is left out.
fn text_of(pieces: &[Piece], selection: &TextSelection) -> String {
    let mut text = String::new();
    let mut last: Option<&Piece> = None;
    for piece in pieces.iter().filter(|piece| piece.id == selection.id) {
        let Some(range) = selection.covering(piece.index, piece.content.len()) else {
            continue;
        };
        let Some(covered) = piece.content.get(range) else {
            continue;
        };
        if let Some(before) = last {
            text.push_str(separator(before, piece));
        }
        text.push_str(covered);
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
