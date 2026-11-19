//! Sticky scroll: the lines that open the blocks the first line in view is inside of, kept at the top of the file as it scrolls; pressing one goes to it.

use crate::events::Interactive;
use crate::events::reduce;
use crate::focus::FocusId;
use crate::marked::{CaretLook, Mark, Paint, marked_line};
use crate::select::styled_text;
use crate::theme;
use crate::theme::Role;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
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
    let kind = theme::code_style(font, 1.4);
    let content = document.field.text()[line].to_string();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(height)
            .interaction_focused(crate::focus::is_held(id))
            .interaction_background(
                (theme::InteractionTones::new().hover(Role::SurfaceHover)).colors(),
            )
            .cursor(CursorIcon::Pointer)
            .control(id, reduce(id)),
        Arrangement::Start,
        Alignment::Center,
        move || {
            rustle::spacer(Modifier::new().width(3.0));
            let gutter = if numbers {
                Modifier::new().width(theme::sp(13))
            } else {
                Modifier::new().width(theme::sp(2))
            };
            row_arranged(
                gutter.padding_each(0.0, 0.0, theme::sp(3), 0.0),
                Arrangement::End,
                Alignment::Center,
                move || {
                    if numbers {
                        styled_text(
                            Modifier::new(),
                            (index + 1).to_string(),
                            kind.color(theme::color(Role::Subtle)),
                        );
                    }
                },
            );
            row_arranged(
                Modifier::new()
                    .weight(1.0)
                    .clip()
                    .padding_xy(theme::sp(2), 0.0),
                Arrangement::Start,
                Alignment::Start,
                move || {
                    marked_line(
                        Modifier::new(),
                        &content,
                        kind.color(theme::color(Role::Text)),
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
    column_arranged(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .fill_max_width()
            .background(theme::color(Role::Background))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0),
        Arrangement::Start,
        Alignment::Start,
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
        let first = (viewer.scroll.offset().max(0.0) / row).floor() as usize;
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
        viewer.scroll.jump_to(shown as f32 * row);
        self
    }
}
