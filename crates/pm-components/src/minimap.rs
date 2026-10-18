//! The minimap: a map of the whole file drawn small beside it, each line a row of coloured blocks where the text is, with the stretch the view shows marked; pressing or dragging on it scrolls the file.

use pm_input::FocusId;
use pm_style::{
    Color, Length, Role, Sides, absolute, bg, border_color, border_l, code_px, cursor_pointer,
    flex, flex_col, flex_none, font_mono, h_px, inset_length, leading_ratio, overflow_hidden,
    relative, text_color, w_px,
};
use pm_treesitter::Kind;
use pm_ui::{Mark, Node, Paint, node, text};

use crate::state::WorkspaceState;
use crate::viewer::kind_role;
use pm_editor::Document;

/// The name of the minimap, which presses and drags on it are about.
pub const MINIMAP: FocusId = FocusId(8_500_000);

/// How wide the minimap is, in logical pixels.
pub const WIDTH: f32 = 84.0;

/// How tall a line of the minimap is, in logical pixels.
const ROW: f32 = 3.0;

/// How big the text its blocks are made from is, in logical pixels; its characters are about this wide.
const FONT: f32 = 2.0;

/// The most characters of a line the minimap draws.
const MOST_COLUMNS: usize = 64;

/// How opaque the blocks are.
const INK: f32 = 0.7;

/// The least tall the mark over the stretch in view is, in logical pixels.
const LEAST_SLIDER: f32 = 6.0;

/// Where the lines of a file are on the minimap and where the view is, for a minimap `height` logical pixels tall.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry {
    /// How many lines show.
    lines: usize,
    /// How far down the minimap's lines are shifted, in logical pixels, when they do not all fit.
    offset: f32,
    /// How many of the lines show for each row of the file.
    per_row: f32,
    /// How many rows the view holds.
    view_rows: f32,
    /// How many rows the file has.
    rows: f32,
}

impl Geometry {
    /// The geometry of `document` scrolled `scroll` logical pixels in a view `view` tall, rows `row` tall, holding `columns` characters when lines wrap, with the minimap `height` tall.
    fn of(
        document: &Document,
        scroll: f32,
        view: f32,
        row: f32,
        columns: Option<usize>,
        height: f32,
    ) -> Self {
        let lines = document.rows();
        let rows = document.wrapped_rows(columns).max(1) as f32;
        let view_rows = (view / row).max(1.0);
        let first = scroll.max(0.0) / row;
        let content = lines as f32 * ROW;
        let share = (first / (rows - view_rows).max(1.0)).clamp(0.0, 1.0);
        Self {
            lines,
            offset: (content - height).max(0.0) * share,
            per_row: lines as f32 / rows,
            view_rows,
            rows,
        }
    }

    /// The top and the height of the mark over the stretch in view, for a view scrolled `scroll` with rows `row` tall, on a minimap `height` tall.
    fn slider(&self, scroll: f32, row: f32, height: f32) -> (f32, f32) {
        let first = scroll.max(0.0) / row;
        let size =
            (self.view_rows * self.per_row * ROW).clamp(LEAST_SLIDER, height.max(LEAST_SLIDER));
        let top = first * self.per_row * ROW - self.offset;
        (top.clamp(0.0, (height - size).max(0.0)), size)
    }

    /// How far to scroll, in logical pixels, to centre the view on the point `y` logical pixels down the minimap, rows `row` tall.
    fn scroll_to(&self, y: f32, row: f32) -> f32 {
        let line = (y + self.offset) / ROW;
        let centre = line / self.per_row.max(f32::EPSILON);
        ((centre - self.view_rows / 2.0) * row).clamp(0.0, (self.rows - 1.0) * row)
    }
}

/// The blocks of the line `text`, whose highlighted stretches are `spans`, as marks over a text of as many spaces as the line has characters up to what the minimap holds, tabs taking `tab` of them.
fn blocks(text: &str, spans: &[(std::ops::Range<usize>, Kind)], tab: usize) -> (usize, Vec<Mark>) {
    let mut roles: Vec<Option<Role>> = Vec::new();
    let mut column_of = Vec::with_capacity(text.len() + 1);
    for (at, character) in text.char_indices() {
        if roles.len() >= MOST_COLUMNS {
            break;
        }
        let width = if character == '\t' { tab.max(1) } else { 1 };
        column_of.resize(at, roles.len());
        column_of.push(roles.len());
        let ink = (!character.is_whitespace()).then_some(Role::Muted);
        roles.extend(std::iter::repeat_n(ink, width));
    }
    column_of.resize(text.len() + 1, roles.len());
    for (range, kind) in spans {
        let Some(role) = kind_role(*kind) else {
            continue;
        };
        let (from, to) = (
            column_of[range.start.min(text.len())],
            column_of[range.end.min(text.len())],
        );
        for held in roles.iter_mut().take(to).skip(from) {
            if held.is_some() {
                *held = Some(role);
            }
        }
    }
    let mut marks: Vec<Mark> = Vec::new();
    let mut start = 0;
    while start < roles.len() {
        let Some(role) = roles[start] else {
            start += 1;
            continue;
        };
        let end = roles[start..]
            .iter()
            .position(|held| *held != Some(role))
            .map_or(roles.len(), |gap| start + gap);
        marks.push(Mark {
            range: start..end,
            color: Color::Role(role, INK),
            paint: Paint::Fill,
        });
        start = end;
    }
    (roles.len(), marks)
}

/// The line at `index` of `document` as a row of the minimap.
fn row_of(document: &Document, index: usize, spans: &[(std::ops::Range<usize>, Kind)]) -> Node {
    let line = document.line(index);
    let (width, marks) = blocks(&document.field.text()[line], spans, document.editing.indent);
    text(" ".repeat(width))
        .styles([
            code_px(FONT),
            leading_ratio(ROW / FONT),
            font_mono(),
            text_color(Color::Transparent),
            flex_none(),
            h_px(ROW),
        ])
        .marks(marks)
}

/// The minimap of `document`, scrolled `scroll` logical pixels in a view `view` tall whose rows are `row` tall and hold `columns` characters when lines wrap, drawn `height` logical pixels tall.
pub(crate) fn minimap(
    document: &Document,
    scroll: f32,
    view: f32,
    row: f32,
    columns: Option<usize>,
    height: f32,
) -> Node {
    let geometry = Geometry::of(document, scroll, view, row, columns, height);
    let first = (geometry.offset / ROW).floor() as usize;
    let count = (height / ROW).ceil() as usize + 1;
    let shown = document.shown_lines(first, count);
    let (low, high) = shown
        .first()
        .zip(shown.last())
        .map_or((0, 0), |(low, high)| (*low, high + 1));
    let spans = document.minimap_highlights(low..high);
    let (top, size) = geometry.slider(scroll, row, height);
    let lines = shown
        .iter()
        .map(|index| row_of(document, *index, &spans[index - low]));
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            relative(),
            overflow_hidden(),
            w_px(WIDTH),
            border_l(),
            border_color(Role::Border),
            cursor_pointer(),
        ])
        .focusable(MINIMAP)
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    flex_none(),
                    absolute(),
                    inset_length(Sides::Top, Length::Px(first as f32 * ROW - geometry.offset)),
                ])
                .children(lines),
        )
        .child(node().styles([
            absolute(),
            inset_length(Sides::Top, Length::Px(top)),
            w_px(WIDTH),
            h_px(size),
            bg(Color::Role(Role::Muted, 0.2)),
        ]))
}

impl WorkspaceState {
    /// The state after a press or drag at `y` logical pixels down a minimap `height` tall: the file shown scrolls to centre its view there, no further than `room` logical pixels, the view being `view` tall.
    pub fn minimap_pressed(mut self, y: f32, height: f32, view: f32, room: f32) -> Self {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let Some(index) = self.git.shown else {
            return self;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return self;
        };
        let crate::open_files::Contents::Text(document) = &viewer.contents else {
            return self;
        };
        let geometry = Geometry::of(document, viewer.scroll, view, row, columns, height);
        viewer.scroll = geometry.scroll_to(y, row).clamp(0.0, room.max(0.0));
        self
    }
}
