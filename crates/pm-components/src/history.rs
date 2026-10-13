//! The history page: every session of every project.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border_b, border_color, cursor_pointer, flex, flex_1, flex_col, flex_none,
    font_medium, gap, h, items_center, min_h, min_w_px, overflow_hidden, px, rounded, text_px,
};
use pm_ui::{Node, node};

use crate::parts::{clipped, line};
use crate::sidebar::status_icon;
use crate::state::{SESSION_BASE, WorkspaceState};

/// The history page for `state`.
pub fn history(state: &WorkspaceState) -> Node {
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    h(12),
                    px(5),
                    flex_none(),
                    border_b(),
                    border_color(Role::Border),
                ])
                .child(line("History", 14, 1.4, Role::Text).styles([font_medium(), text_px(14)])),
        )
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    gap(0.5),
                    flex_1(),
                    min_h(0),
                    overflow_hidden(),
                    px(5),
                    pm_style::py(3),
                ])
                .children(
                    state
                        .sessions
                        .iter()
                        .enumerate()
                        .rev()
                        .map(|(index, session)| {
                            node()
                                .styles([
                                    flex(),
                                    items_center(),
                                    gap(3),
                                    h(10),
                                    px(3),
                                    rounded(Radius::Md),
                                    cursor_pointer(),
                                    bg(Role::SurfaceHover).hover(),
                                ])
                                .child(status_icon(session.status))
                                .child(clipped(&session.title, 14, Role::Text))
                                .child(line(
                                    &state.projects[session.project].name,
                                    12,
                                    1.4,
                                    Role::Muted,
                                ))
                                .child(line(&session.updated, 12, 1.4, Role::Subtle))
                                .focusable(FocusId(SESSION_BASE + index as u64))
                        }),
                ),
        )
}
