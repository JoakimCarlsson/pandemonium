//! What the last frame painted and where: the geometry the host reads back.
//!
//! Every frame the kit records, in paint order, the rectangle of each
//! control named with [`Interactive::target`](crate::kit::Interactive::target)
//! or [`Interactive::region`](crate::kit::Interactive::region), how far each
//! scrolling box can scroll, where each visible line of each text field was
//! drawn with its shaped layout, and the pieces of selectable text. The host
//! uses it for what a handler cannot answer alone: the order Tab walks,
//! moving a caret up a line as the lines are drawn, the input method's caret
//! rectangle, keeping a caret in view, the size of a panel being resized, or
//! where the minimap is. [`root`] starts a fresh report every time the
//! interface is painted.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use rustle::{Affinity, Embedded, Modifier, Offset, PixelRounding, Rect, TextLayout};

use crate::kit::focus::FocusId;
use crate::kit::select::{Piece, TextPoint, TextSelection};

/// Where the caret goes when it is moved one place along the screen in a
/// line that holds right-to-left text: the step of a caret along the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// To this shown byte.
    To(usize),
    /// Off the end of the line the way it was moved.
    Edge,
}

/// A control or area named by an id and the part of it left visible where it was painted, in logical
/// pixels from the window's top left; empty when it is scrolled or clipped out of sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    /// What names it.
    pub id: FocusId,
    /// Where it was painted.
    pub rect: Rect,
    /// Whether it takes keyboard focus in tab order.
    pub focusable: bool,
}

/// A scrolling box: where it was painted, how far its content can scroll and how far it is scrolled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroller {
    /// What names it.
    pub id: FocusId,
    /// Where it was painted.
    pub rect: Rect,
    /// How far, in logical pixels, its content can scroll.
    pub room: f32,
    /// How far, in logical pixels from the top, it is scrolled, kept within the room.
    pub offset: f32,
}

/// The shaped text of a text field as drawn: one per field, covering all of
/// its lines, whether or not each line was on screen.
#[derive(Clone, Debug)]
pub struct FieldBox {
    /// The field.
    pub id: FocusId,
    /// The shown text of the field, shaped as it was drawn, wrapped when it wraps.
    pub layout: Rc<TextLayout>,
    /// Where the layout's top left was drawn, in the window, scroll included.
    pub origin: Offset,
    /// The part of the window the field shows its text in.
    pub view: Rect,
}

impl FieldBox {
    /// The shown byte nearest the window point `x`, `y`.
    pub fn offset_at(&self, x: f32, y: f32) -> usize {
        self.layout
            .hit_test(Offset::new(x - self.origin.x, y - self.origin.y))
    }

    /// Where the caret at shown byte `caret` is drawn, in the window: x, top, height.
    pub fn caret_at(&self, caret: usize) -> (f32, f32, f32) {
        let at = self.layout.caret(caret, Affinity::Downstream);
        (self.origin.x + at.x, self.origin.y + at.y, at.height)
    }
}

/// One line, or one piece of a line, of a text field drawn line by line with
/// [`marked_field_line`](crate::marked::marked_field_line).
#[derive(Clone, Debug)]
pub struct FieldLine {
    /// The field.
    pub id: FocusId,
    /// The shown byte of the field the piece starts at.
    pub start: usize,
    /// The piece's text, shaped as it was drawn.
    pub layout: Rc<TextLayout>,
    /// Where the piece was painted in the window.
    pub rect: Rect,
}

impl FieldLine {
    /// The shown byte one past the end of the piece.
    fn end(&self) -> usize {
        self.start + self.layout.text().len()
    }

    /// Whether the shown byte `caret` lies on the piece, its end included.
    fn holds(&self, caret: usize) -> bool {
        self.start <= caret && caret <= self.end()
    }

    /// The shown byte of the field nearest the window point `x`, `y`.
    fn offset_at(&self, x: f32, y: f32) -> usize {
        self.start
            + self
                .layout
                .hit_test(Offset::new(x - self.rect.origin.x, y - self.rect.origin.y))
    }

    /// Where the caret at shown byte `caret` of the field is drawn in the window: x, top, height.
    fn caret_at(&self, caret: usize) -> (f32, f32, f32) {
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

/// Everything the last frame painted that the host reads back.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Named controls and areas, in paint order.
    pub regions: Vec<Region>,
    /// Scrolling boxes, in paint order.
    pub scrollers: Vec<Scroller>,
    /// Text fields, in paint order.
    pub fields: Vec<FieldBox>,
    /// Pieces of text fields drawn line by line, in paint order.
    pub field_lines: Vec<FieldLine>,
    /// Boxes of selectable texts, by the text they are part of, in paint order.
    pub selectable_areas: Vec<(FocusId, Rect)>,
    /// Entries of long lists that were not built, kept in keyboard traversal order after the regions.
    pub virtual_focus: Vec<FocusId>,
    /// Pieces of selectable text, in paint order.
    pub pieces: Vec<Piece>,
}

/// The window a root composable fills: the main window is [`MAIN`], others
/// are numbered by the host, so each keeps a report of its own.
pub type WindowKey = u64;

/// The key of the main window.
pub const MAIN: WindowKey = 0;

thread_local! {
    /// The report of the last frame painted in each window on this thread.
    static REPORTS: RefCell<HashMap<WindowKey, Report>> = RefCell::new(HashMap::new());
    /// The window whose frame is being painted.
    static PAINTING: Cell<WindowKey> = const { Cell::new(MAIN) };
}

/// The window whose frame is being painted.
pub(crate) fn painting() -> WindowKey {
    PAINTING.with(Cell::get)
}

/// Runs `record` on the report of the frame being painted.
pub(crate) fn record(record: impl FnOnce(&mut Report)) {
    let window = PAINTING.with(Cell::get);
    REPORTS.with(|reports| record(reports.borrow_mut().entry(window).or_default()));
}

/// The report of the last frame painted in the main window.
pub fn last() -> Report {
    last_of(MAIN)
}

/// The report of the last frame painted in `window`; empty before one was painted.
pub fn last_of(window: WindowKey) -> Report {
    REPORTS.with(|reports| reports.borrow().get(&window).cloned().unwrap_or_default())
}

/// Reads the report of the last frame painted in the main window without copying it.
pub fn with<R>(read: impl FnOnce(&Report) -> R) -> R {
    with_of(MAIN, read)
}

/// Reads the report of the last frame painted in `window` without copying it.
pub fn with_of<R>(window: WindowKey, read: impl FnOnce(&Report) -> R) -> R {
    REPORTS.with(|reports| read(reports.borrow_mut().entry(window).or_default()))
}

/// Forgets the report of `window`, for a window that closed.
pub fn forget(window: WindowKey) {
    REPORTS.with(|reports| reports.borrow_mut().remove(&window));
}

/// The modifier that starts a fresh report of the main window whenever the
/// area it is on is painted; [`root`] puts it on the outermost box of a window.
pub fn starts_report() -> Modifier {
    starts_report_of(MAIN)
}

/// The modifier that starts a fresh report of `window` whenever the area it
/// is on is painted, and records what is painted inside it in that report.
pub fn starts_report_of(window: WindowKey) -> Modifier {
    Modifier::new().on_placed(move |_, _| {
        PAINTING.with(|painting| painting.set(window));
        REPORTS.with(|reports| {
            reports.borrow_mut().insert(window, Report::default());
        });
    })
}

/// Emits `content` filling the main window, starting a fresh report each time it is painted.
#[track_caller]
pub fn root(content: impl FnOnce()) {
    root_of(MAIN, content);
}

/// Emits `content` filling the window `window`, starting a fresh report of
/// that window each time it is painted.
#[track_caller]
pub fn root_of(window: WindowKey, content: impl FnOnce()) {
    rustle::stack_with(starts_report_of(window).fill_max_size(), content);
}

/// `rect` moved `dx` right and `dy` down.
fn moved(rect: Rect, dx: f32, dy: f32) -> Rect {
    Rect::new(
        Offset::new(rect.origin.x + dx, rect.origin.y + dy),
        rect.size,
    )
}

impl Report {
    /// The same report with everything moved `dx` right and `dy` down, for a
    /// host that measures from a point other than the window's top left, such
    /// as the corner of a frame inset by the band that holds its shadow.
    pub fn shifted(mut self, dx: f32, dy: f32) -> Self {
        if dx == 0.0 && dy == 0.0 {
            return self;
        }
        for region in &mut self.regions {
            region.rect = moved(region.rect, dx, dy);
        }
        for scroller in &mut self.scrollers {
            scroller.rect = moved(scroller.rect, dx, dy);
        }
        for field in &mut self.fields {
            field.origin = Offset::new(field.origin.x + dx, field.origin.y + dy);
            field.view = moved(field.view, dx, dy);
        }
        for line in &mut self.field_lines {
            line.rect = moved(line.rect, dx, dy);
        }
        for piece in &mut self.pieces {
            piece.rect = moved(piece.rect, dx, dy);
        }
        for (_, rect) in &mut self.selectable_areas {
            *rect = moved(*rect, dx, dy);
        }
        self
    }

    /// The box of a selectable text under the window point `at`: the name of
    /// its text, and how many boxes of that text were painted before it.
    pub fn selectable_at(&self, at: (f32, f32)) -> Option<(FocusId, usize)> {
        let index = self
            .selectable_areas
            .iter()
            .rposition(|(_, rect)| contains(*rect, at))?;
        let id = self.selectable_areas[index].0;
        let before = self.selectable_areas[..index]
            .iter()
            .filter(|(held, _)| *held == id)
            .count();
        Some((id, before))
    }

    /// The topmost named control or area containing the window point `at`.
    pub fn hit(&self, at: (f32, f32)) -> Option<FocusId> {
        self.regions
            .iter()
            .rev()
            .find(|region| contains(region.rect, at))
            .map(|region| region.id)
    }

    /// Where `id` was painted.
    pub fn region(&self, id: FocusId) -> Option<Rect> {
        self.regions
            .iter()
            .find(|region| region.id == id)
            .map(|region| region.rect)
    }

    /// Whether `id` was painted.
    pub fn drawn(&self, id: FocusId) -> bool {
        self.regions.iter().any(|region| region.id == id)
    }

    /// The scrolling box `id`.
    pub fn scroller(&self, id: FocusId) -> Option<&Scroller> {
        self.scrollers.iter().find(|scroller| scroller.id == id)
    }

    /// The topmost scrolling box containing the window point `at`.
    pub fn scroller_at(&self, at: (f32, f32)) -> Option<&Scroller> {
        self.scrollers
            .iter()
            .rev()
            .find(|scroller| contains(scroller.rect, at))
    }

    /// The order Tab walks: the focusable controls in paint order, then the entries not built.
    pub fn focus_order(&self) -> Vec<FocusId> {
        let mut order: Vec<FocusId> = Vec::new();
        for region in self.regions.iter().filter(|region| region.focusable) {
            if !order.contains(&region.id) {
                order.push(region.id);
            }
        }
        order.extend(
            self.virtual_focus
                .iter()
                .filter(|id| !order.contains(id))
                .copied()
                .collect::<Vec<_>>(),
        );
        order
    }

    /// The text field `id` as drawn.
    pub fn field(&self, id: FocusId) -> Option<&FieldBox> {
        self.fields.iter().rev().find(|field| field.id == id)
    }

    /// The shown byte of field `id` nearest the window point `x`, `y`; replaces `Output::offset_in`.
    pub fn offset_in(&self, id: FocusId, x: f32, y: f32) -> Option<usize> {
        if let Some(field) = self.field(id) {
            return Some(field.offset_at(x, y));
        }
        let rows = self.rows_of(id);
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

    /// The pieces of field `id` drawn line by line, grouped into rows from
    /// the top, each row from the left.
    fn rows_of(&self, id: FocusId) -> Vec<Vec<&FieldLine>> {
        let mut pieces: Vec<&FieldLine> = self
            .field_lines
            .iter()
            .filter(|piece| piece.id == id)
            .collect();
        pieces.sort_by(|a, b| {
            a.rect
                .origin
                .y
                .total_cmp(&b.rect.origin.y)
                .then(a.rect.origin.x.total_cmp(&b.rect.origin.x))
        });
        let mut rows: Vec<Vec<&FieldLine>> = Vec::new();
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

    /// The row of field `id` drawn line by line that holds the shown byte
    /// `caret`, by its place from the top, and the piece holding it.
    fn row_holding(
        &self,
        id: FocusId,
        caret: usize,
    ) -> Option<(Vec<Vec<&FieldLine>>, usize, &FieldLine)> {
        let rows = self.rows_of(id);
        let (index, piece) = rows.iter().enumerate().find_map(|(index, row)| {
            row.iter()
                .find(|piece| piece.holds(caret))
                .map(|piece| (index, *piece))
        })?;
        Some((rows, index, piece))
    }

    /// The shown byte `by` lines below the shown byte `caret` of field `id`, as
    /// its lines are drawn, keeping the caret's column; `None` past the first
    /// or the last line. Replaces `Output::vertical`.
    pub fn vertical(&self, id: FocusId, caret: usize, by: isize) -> Option<usize> {
        let Some(field) = self.field(id) else {
            let (rows, index, piece) = self.row_holding(id, caret)?;
            let (x, _, _) = piece.caret_at(caret);
            let row = rows.get(index.checked_add_signed(by)?)?;
            let target = row
                .iter()
                .find(|piece| x < piece.rect.origin.x + piece.rect.size.width)
                .or(row.last())?;
            let y = target.rect.origin.y + target.rect.size.height / 2.0;
            return Some(target.offset_at(x, y));
        };
        let layout = &field.layout;
        let line = layout.line_of_offset(caret, Affinity::Downstream);
        let target = line.checked_add_signed(by)?;
        if target >= layout.line_count() {
            return None;
        }
        let x = layout.caret(caret, Affinity::Downstream).x;
        Some(layout.offset_on_line(target, x).0)
    }

    /// The shown byte at the start, or the end when `end` is set, of the line
    /// the shown byte `caret` of field `id` is on, as drawn. Replaces `Output::line_edge`.
    pub fn line_edge(&self, id: FocusId, caret: usize, end: bool) -> Option<usize> {
        let Some(field) = self.field(id) else {
            let (rows, index, _) = self.row_holding(id, caret)?;
            let row = &rows[index];
            return if end {
                row.iter().map(|piece| piece.end()).max()
            } else {
                row.iter().map(|piece| piece.start).min()
            };
        };
        let layout = &field.layout;
        let edge = if end {
            layout.line_end(caret, Affinity::Downstream)
        } else {
            layout.line_start(caret, Affinity::Downstream)
        };
        Some(edge.0)
    }

    /// Where the caret at shown byte `caret` of field `id` goes when moved one
    /// place right, or left, along the screen, when its line holds
    /// right-to-left text; `None` when the line runs left to right
    /// throughout, so moving by bytes is right. Replaces `Output::visual_step`.
    pub fn visual_step(&self, id: FocusId, caret: usize, right: bool) -> Option<Step> {
        let Some(field) = self.field(id) else {
            let (_, _, piece) = self.row_holding(id, caret)?;
            return match step_in(&piece.layout, caret - piece.start, right)? {
                Step::To(byte) => Some(Step::To(piece.start + byte)),
                Step::Edge => Some(Step::Edge),
            };
        };
        step_in(&field.layout, caret, right)
    }

    /// The word, or the run of lines up to a blank line, of the selectable
    /// text `id` around the place `point`, as `reach` says; replaces `Output::unit_around`.
    pub fn unit_around(
        &self,
        id: FocusId,
        point: TextPoint,
        reach: pm_field::Reach,
    ) -> (TextPoint, TextPoint) {
        crate::kit::select::unit_around(&self.pieces, id, point, reach)
    }

    /// The caret of field `id` at shown byte `caret` as a rectangle in the
    /// window, one pixel wide, for the input method's candidate window.
    pub fn caret_rect(&self, id: FocusId, caret: usize) -> Option<Rect> {
        let (x, y, height) = match self.field(id) {
            Some(field) => field.caret_at(caret),
            None => self.row_holding(id, caret)?.2.caret_at(caret),
        };
        Some(Rect::new(Offset::new(x, y), rustle::Size::new(1.0, height)))
    }

    /// The selectable text under the window point `at`: its name and the place in it nearest the point.
    pub fn text_under(&self, at: (f32, f32)) -> Option<(FocusId, TextPoint)> {
        let piece = self
            .pieces
            .iter()
            .rev()
            .find(|piece| contains(piece.rect, at))?;
        Some((piece.id, self.text_point(piece.id, at)?))
    }

    /// The place in the selectable text `id` nearest the window point `at`.
    pub fn text_point(&self, id: FocusId, at: (f32, f32)) -> Option<TextPoint> {
        crate::kit::select::point_in(&self.pieces, id, at)
    }

    /// A selection of the whole of the selectable text `id` that was painted.
    pub fn select_all(&self, id: FocusId) -> Option<TextSelection> {
        crate::kit::select::all_of(&self.pieces, id)
    }

    /// What `selection` covers, its lines broken where they were drawn on separate rows.
    pub fn selected_text(&self, selection: &TextSelection) -> String {
        crate::kit::select::text_of(&self.pieces, selection)
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

/// Whether the window point `at` lies inside `rect`.
pub fn contains(rect: Rect, at: (f32, f32)) -> bool {
    at.0 >= rect.origin.x
        && at.1 >= rect.origin.y
        && at.0 < rect.origin.x + rect.size.width
        && at.1 < rect.origin.y + rect.size.height
}

/// Lays `content` out at `width` by `height` logical pixels without drawing
/// it, with `focused` holding focus, and returns what was painted where.
///
/// For tests and snapshots that ask which controls were drawn and where: no
/// GPU is needed, text is measured with the fonts of this thread.
pub fn lay_out(
    width: f32,
    height: f32,
    focused: Option<FocusId>,
    content: impl Fn() + 'static,
) -> Report {
    crate::kit::frame::set(crate::kit::frame::Frame {
        focused,
        ..crate::kit::frame::Frame::default()
    });
    rustle_wgpu::set_legible_text(false);
    let tokens = crate::kit::theme::Tokens::default();
    let root = move || crate::kit::theme::provide(tokens.clone(), || root(&content));
    let mut ui = Embedded::without_renderer(root, || {});
    ui.set_pixel_rounding(PixelRounding::Logical);
    ui.resize((width.round() as u32, height.round() as u32), 1.0);
    ui.update();
    ui.invalidate();
    ui.update();
    last()
}
