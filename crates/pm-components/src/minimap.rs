//! The minimap: a map of the whole file drawn small beside it, each line a row of coloured blocks where the text is, with the stretch the view shows marked; pressing or dragging on it scrolls the file.

use crate::areas::EditorEvent;
use crate::events;
use rustle::{Alignment, Arrangement};

use crate::target::Target;
use crate::theme;
use pm_treesitter::Kind;
use rustle::{Color, Modifier};

use crate::marked::{Mark, Paint};
use crate::state::WorkspaceState;
use crate::theme::Role;
use crate::viewer::kind_role;
use pm_editor::Document;

/// How wide the minimap is, in logical pixels.
pub const WIDTH: f32 = 84.0;

/// How tall a line of the minimap is, in logical pixels.
const ROW: f32 = 3.0;

/// The size of code, in logical pixels, whose characters are as wide as a column of blocks.
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
    /// The geometry of `document` scrolled so row `top` is at the top of a view `view` tall, rows `row` tall, holding `columns` characters when lines wrap, with the minimap `height` tall.
    fn of(
        document: &Document,
        top: f32,
        view: f32,
        row: f32,
        columns: Option<usize>,
        height: f32,
    ) -> Self {
        let lines = document.rows();
        let rows = document.wrapped_rows(columns).max(1) as f32;
        let view_rows = (view / row).max(1.0);
        let first = top.max(0.0);
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

    /// The top and the height of the mark over the stretch in view, for a view scrolled so row `top` is at its top, on a minimap `height` tall.
    fn slider(&self, top: f32, height: f32) -> (f32, f32) {
        let first = top.max(0.0);
        let size =
            (self.view_rows * self.per_row * ROW).clamp(LEAST_SLIDER, height.max(LEAST_SLIDER));
        let top = first * self.per_row * ROW - self.offset;
        (top.clamp(0.0, (height - size).max(0.0)), size)
    }

    /// The row to scroll to the top of the view to centre it on the point `y` logical pixels down the minimap.
    fn scroll_to(&self, y: f32) -> f32 {
        let line = (y + self.offset) / ROW;
        let centre = line / self.per_row.max(f32::EPSILON);
        (centre - self.view_rows / 2.0).clamp(0.0, (self.rows - 1.0).max(0.0))
    }
}

/// The blocks of the line `text`, whose highlighted stretches are `spans`, as marks over the columns of the line's characters up to what the minimap holds, tabs taking `tab` of them.
fn blocks(text: &str, spans: &[(std::ops::Range<usize>, Kind)], tab: usize) -> Vec<Mark> {
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
        marks.push(Mark::new(start..end, role.alpha(INK), Paint::Fill));
        start = end;
    }
    marks
}

/// The highlighted stretches of a line, as offsets into it.
type Spans = Vec<(std::ops::Range<usize>, Kind)>;

/// The blocks of a line worked out last: for the line's text and the tab width they were worked out at, the highlights they were worked out from, and its blocks.
type KeptBlocks = (String, usize, Spans, Vec<Mark>);

thread_local! {
    /// The blocks of the lines drawn last, by line, so that a line whose text and highlights did not change is not worked out again.
    static BLOCKS: std::cell::RefCell<std::collections::HashMap<usize, KeptBlocks>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The blocks of line `index` of `document`, whose highlighted stretches are `spans`, as [`blocks`] gives them, kept from the last time when neither the line's text nor its highlights changed since.
fn kept_blocks(
    document: &Document,
    index: usize,
    spans: &[(std::ops::Range<usize>, Kind)],
) -> Vec<Mark> {
    let text = &document.field.text()[document.line(index)];
    let tab = document.editing.indent;
    BLOCKS.with(|kept| {
        let mut kept = kept.borrow_mut();
        if let Some((held_text, held_tab, held_spans, marks)) = kept.get(&index)
            && held_text == text
            && *held_tab == tab
            && held_spans.as_slice() == spans
        {
            return marks.clone();
        }
        let marks = blocks(text, spans, tab);
        if kept.len() > 4096 {
            kept.clear();
        }
        kept.insert(index, (text.to_owned(), tab, spans.to_vec(), marks.clone()));
        marks
    })
}

/// One block of the minimap: where it starts across and down, how wide it is and its colour, in logical pixels from the top-left of the first row drawn.
type Block = (f32, f32, f32, Color);

/// The blocks of the lines `shown` of `document`, whose highlighted stretches are `spans` from line `low` on, each line a row [`ROW`] tall under the one before and each column `cell` wide.
fn blocks_of(
    document: &Document,
    shown: &[usize],
    low: usize,
    spans: &[Spans],
    cell: f32,
) -> Vec<Block> {
    let mut drawn = Vec::new();
    for (row, index) in shown.iter().enumerate() {
        let marks = kept_blocks(document, *index, &spans[index - low]);
        let top = row as f32 * ROW;
        drawn.extend(marks.into_iter().map(|mark| {
            (
                mark.range.start as f32 * cell,
                top,
                mark.range.len() as f32 * cell,
                mark.tone,
            )
        }));
    }
    drawn
}

/// Emits the minimap of `document`, scrolled so row `top` is at the top of a view `view` tall whose rows are `row` tall and hold `columns` characters when lines wrap, drawn `height` logical pixels tall.
#[track_caller]
pub(crate) fn minimap(
    document: &Document,
    top: f32,
    view: f32,
    row: f32,
    columns: Option<usize>,
    height: f32,
) {
    let geometry = Geometry::of(document, top, view, row, columns, height);
    let first = (geometry.offset / ROW).floor() as usize;
    let count = (height / ROW).ceil() as usize + 1;
    let shown = document.shown_lines(first, count);
    let (low, high) = shown
        .first()
        .zip(shown.last())
        .map_or((0, 0), |(low, high)| (*low, high + 1));
    let spans = document.minimap_highlights(low..high);
    let (slider_top, size) = geometry.slider(top, height);
    rustle::row_arranged(
        Modifier::new().width(WIDTH).fill_max_height(),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            let own = Modifier::new().weight(1.0).fill_max_height().clip();
            let own = events::pressed(own, Target::Editor(EditorEvent::Minimap))
                .chain(crate::events::minimap_drag(height));
            rustle::stack_with(own, || {
                let blocks = blocks_of(document, &shown, low, &spans, theme::code_cell(FONT));
                rustle::canvas(
                    Modifier::new()
                        .fill_max_size()
                        .offset(0.0, first as f32 * ROW - geometry.offset),
                    move |canvas| {
                        for &(x, y, width, color) in &blocks {
                            canvas.rect(x, y, width, ROW, color);
                        }
                    },
                );
                rustle::spacer(
                    Modifier::new()
                        .offset(0.0, slider_top)
                        .size(WIDTH, size)
                        .background(theme::color(Role::Muted.alpha(0.2))),
                );
            });
        },
    );
}

impl WorkspaceState {
    /// The state after a press or drag at `y` logical pixels down a minimap `height` tall: the file shown scrolls to centre its view there, no further than its view can scroll.
    pub fn minimap_pressed(mut self, y: f32, height: f32) -> Self {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let Some(viewer) = self.chosen_viewer_mut() else {
            return self;
        };
        let Some(document) = viewer.document() else {
            return self;
        };
        let view = viewer.scroll.scroll().viewport_length();
        let top = viewer.top_row(columns, row);
        let geometry = Geometry::of(document, top, view, row, columns, height);
        viewer.scroll_to_row(geometry.scroll_to(y), columns, row);
        self
    }
}
