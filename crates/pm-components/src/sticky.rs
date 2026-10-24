//! Sticky scroll: the lines that open the blocks the first line in view is inside of, kept at the top of the file as it scrolls; pressing one goes to it.

use pm_input::FocusId;
use pm_style::{
    Role, absolute, bg, border_b, border_color, code_px, cursor_pointer, flex, flex_1, flex_col,
    flex_none, font_mono, h_px, items_center, justify_end, leading_ratio, left, min_w_px,
    overflow_hidden, pr, px, right, text_color, top, w, w_px,
};
use pm_ui::{Node, node, text};

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

/// The line at `index` of `document` as a row kept at the top, `place` lines below the top, with the line number beside it when `settings` show them.
fn kept_line(document: &Document, index: usize, place: usize, settings: &EditorSettings) -> Node {
    let line = document.line(index);
    let marks = document
        .line_highlights(index)
        .into_iter()
        .filter_map(|(range, kind)| {
            kind_role(kind).map(|role| pm_ui::Mark {
                range,
                color: role.into(),
                paint: pm_ui::Paint::Ink,
            })
        });
    let font = settings.font_size;
    let height = settings.row_height();
    node()
        .styles([
            flex(),
            items_center(),
            h_px(height),
            flex_none(),
            bg(Role::SurfaceHover).hover(),
            cursor_pointer(),
        ])
        .focusable(FocusId(STICKY_BASE + place as u64))
        .child(node().styles([w_px(3.0), flex_none()]))
        .child(
            node()
                .styles([flex(), items_center(), justify_end(), flex_none(), pr(3)])
                .styles([if settings.line_numbers { w(13) } else { w(2) }])
                .children(settings.line_numbers.then(|| {
                    text((index + 1).to_string()).styles([
                        code_px(font),
                        leading_ratio(1.4),
                        font_mono(),
                        text_color(Role::Subtle),
                    ])
                })),
        )
        .child(
            node()
                .styles([flex(), flex_1(), min_w_px(0), overflow_hidden(), px(2)])
                .child(
                    text(&document.field.text()[line])
                        .styles([
                            code_px(font),
                            leading_ratio(1.4),
                            font_mono(),
                            text_color(Role::Text),
                            flex_none(),
                        ])
                        .marks(marks),
                ),
        )
}

/// The lines that open the blocks around the first line in view of `document`, scrolled `scroll` logical pixels with lines holding `columns` characters when they wrap, as a box over the top of the view; nothing when there are none or `settings` do not want them.
pub(crate) fn sticky(
    document: &Document,
    scroll: f32,
    columns: Option<usize>,
    settings: &EditorSettings,
) -> Option<Node> {
    if !settings.sticky_scroll {
        return None;
    }
    let row = (scroll.max(0.0) / settings.row_height()).floor() as usize;
    let lines = document.sticky_lines(row, columns, MOST);
    if lines.is_empty() {
        return None;
    }
    Some(
        node()
            .styles([
                absolute(),
                top(0),
                left(0),
                right(0),
                flex(),
                flex_col(),
                bg(Role::Background),
                border_b(),
                border_color(Role::Border),
            ])
            .children(
                lines
                    .into_iter()
                    .enumerate()
                    .map(|(place, index)| kept_line(document, index, place, settings)),
            ),
    )
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
