//! Sticky scroll: the lines that open the blocks the first line in view is inside of, kept at the top of the file as it scrolls; pressing one goes to it.

use crate::areas::EditorEvent;
use crate::kit::{self, Frame};
use crate::marked::{CaretLook, Mark, Paint, marked_line};
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::Role;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{BoxAlignment, Modifier};

use crate::state::WorkspaceState;
use crate::viewer::kind_role;
use pm_editor::{Document, EditorSettings};

/// The most lines kept at the top.
pub const MOST: usize = 4;

/// Emits the line at `index` of `document` as a row kept at the top, `place` lines below the top, with the line number beside it when `settings` show them.
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
    let id = Target::Editor(EditorEvent::Sticky(place));
    let kind = theme::code_style(font, 1.4);
    let content = document.field.text()[line].to_string();
    let own = Modifier::new().fill_max_width().height(height).indicated(
        theme::InteractionTones::new().hover(Role::SurfaceHover),
        theme::Focused::Held(crate::focus::is_held(id)),
    );
    kit::row(kit::pressed(own, id), 0, move || {
        rustle::spacer(Modifier::new().width(3.0));
        let gutter = Modifier::new().width(theme::sp(if numbers { 13 } else { 2 }));
        row_arranged(
            gutter.padding_each(0.0, 0.0, theme::sp(3), 0.0),
            Arrangement::End,
            Alignment::Center,
            move || {
                if numbers {
                    kit::text(
                        Modifier::new(),
                        (index + 1).to_string(),
                        kind.color(theme::color(Role::Subtle)),
                    );
                }
            },
        );
        kit::row_top(
            Modifier::new()
                .weight(1.0)
                .clip()
                .padding_xy(theme::sp(2), 0.0),
            0,
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
    });
}

/// Emits the lines that open the blocks around the first line in view of `document`, scrolled so row `top` is at the top of its view, with lines holding `columns` characters when they wrap, as a box over the top of the view of the layer it is placed in; nothing when there are none or `settings` do not want them.
#[track_caller]
pub(crate) fn sticky(
    document: &Document,
    top: f32,
    columns: Option<usize>,
    settings: &EditorSettings,
) {
    if !settings.sticky_scroll {
        return;
    }
    let row = top.max(0.0).floor() as usize;
    let lines = document.sticky_lines(row, columns, MOST);
    if lines.is_empty() {
        return;
    }
    kit::column(
        Modifier::new()
            .align(BoxAlignment::TopStart)
            .fill_max_width()
            .background(theme::color(Role::Background))
            .ruled(theme::Side::Bottom),
        0,
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
        let Some(viewer) = self.chosen_viewer_mut() else {
            return self;
        };
        let first = viewer.top_row(columns, row).max(0.0).floor() as usize;
        let Some(document) = viewer.document_mut() else {
            return self;
        };
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
        viewer.scroll_to_row(shown as f32, columns, row);
        self
    }
}
