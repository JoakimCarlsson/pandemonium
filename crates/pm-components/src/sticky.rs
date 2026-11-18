//! Sticky scroll: the lines that open the blocks the first line in view is inside of, kept at the top of the file as it scrolls; pressing one goes to it.

use crate::kit::{
    CaretLook, Flex, FocusId, Hovered, Interactive, Looks, Mark, Paint, Radius, Role, Side, Styled,
    Type, column, gap_box, label, marked_line, row,
};
use rustle::{BoxAlignment, CursorIcon, Modifier};

use crate::state::WorkspaceState;
use crate::viewer::kind_role;
use pm_editor::{Document, EditorSettings};

/// The first of the names of the lines kept at the top, each following it by how many lines are above it among them.
pub const STICKY_BASE: u64 = 8_500_100;

/// One past the last of the names of the lines kept at the top.
pub const STICKY_END: u64 = 8_500_200;

/// The most lines kept at the top.
pub const MOST: usize = 4;

/// How many lines are above the line kept at the top that the press on `id` is about.
pub fn sticky_of(id: FocusId) -> Option<usize> {
    (STICKY_BASE..STICKY_END)
        .contains(&id.0)
        .then(|| (id.0 - STICKY_BASE) as usize)
}

/// Emits the line at `index` of `document` as a row kept at the top, `place` lines below the top, with the line number beside it when `settings` show them.
#[track_caller]
fn kept_line(document: &Document, index: usize, place: usize, settings: &EditorSettings) {
    let line = document.line(index);
    let marks: Vec<Mark> = document
        .line_highlights(index)
        .into_iter()
        .filter_map(|(range, kind)| kind_role(kind).map(|role| Mark::new(range, role, Paint::Ink)))
        .collect();
    let font = settings.font_size;
    let height = settings.row_height();
    let numbers = settings.line_numbers;
    let id = FocusId(STICKY_BASE + place as u64);
    let kind = Type::code(font).ratio(1.4);
    let content = document.field.text()[line].to_string();
    row(
        Modifier::new()
            .height(height)
            .looks_of(Looks::new().hover(Role::SurfaceHover), id)
            .cursor(CursorIcon::Pointer)
            .target(id),
        Flex::new().center(),
        move || {
            gap_box(Modifier::new().width(3.0));
            let gutter = if numbers {
                Modifier::new().w(13)
            } else {
                Modifier::new().w(2)
            };
            row(
                gutter.pad_side(Side::Right, 3),
                Flex::new().center().trailing(),
                move || {
                    if numbers {
                        label((index + 1).to_string(), kind.tone(Role::Subtle));
                    }
                },
            );
            row(
                Modifier::new().grow().clip_to(Radius::None).pad_x(2),
                Flex::new(),
                move || {
                    marked_line(
                        Modifier::new(),
                        &content,
                        kind.tone(Role::Text),
                        marks,
                        CaretLook::default(),
                    );
                },
            );
        },
    );
}

/// Emits the lines that open the blocks around the first line in view of `document`, scrolled `scroll` logical pixels with lines holding `columns` characters when they wrap, as a box over the top of the view of the layer it is placed in; nothing when there are none or `settings` do not want them.
#[track_caller]
pub(crate) fn sticky(
    document: &Document,
    scroll: f32,
    columns: Option<usize>,
    settings: &EditorSettings,
) {
    if !settings.sticky_scroll {
        return;
    }
    let row = (scroll.max(0.0) / settings.row_height()).floor() as usize;
    let lines = document.sticky_lines(row, columns, MOST);
    if lines.is_empty() {
        return;
    }
    column(
        Modifier::new()
            .placed(BoxAlignment::TopStart)
            .fill_max_width()
            .fill(Role::Background)
            .stroke_side(Side::Bottom, 1.0, Role::Border),
        Flex::new(),
        || {
            for (place, index) in lines.into_iter().enumerate() {
                rustle::key(place, || kept_line(document, index, place, settings));
            }
        },
    );
}

impl WorkspaceState {
    /// The state after the line kept at the top with `above` lines above it was pressed: the caret goes to it and the file scrolls so it shows just under those above.
    pub fn sticky_pressed(mut self, above: usize) -> Self {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let Some(index) = self.git.shown else {
            return self;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return self;
        };
        let crate::open_files::Contents::Text(document) = &mut viewer.contents else {
            return self;
        };
        let first = (viewer.scroll.max(0.0) / row).floor() as usize;
        let Some(line) = document
            .sticky_lines(first, columns, MOST)
            .get(above)
            .copied()
        else {
            return self;
        };
        document.jump(pm_protocol::Position::new(line as u32, 0));
        let shown = document
            .wrapped_row_of_line(line, columns)
            .saturating_sub(above);
        viewer.scroll = shown as f32 * row;
        self
    }
}
