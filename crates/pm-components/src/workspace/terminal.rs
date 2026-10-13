//! The terminal dock under the session.

use pm_icon::IconName;
use pm_style::{
    Radius, Role, bg, border_b, border_color, border_t, flex, flex_col, flex_none, font_mono, gap,
    h, h_px, items_center, leading_ratio, p, px, rounded, text_color, text_px, w_full,
};
use pm_ui::{Node, node, text};

use super::state::TERMINAL;
use crate::Button;
use crate::parts::{glyph, line, spacer};

/// How tall the dock is, in logical pixels.
const HEIGHT: f32 = 280.0;

/// The dock for a session of `project`: one shell, waiting at its prompt.
pub(super) fn terminal(project: &str) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_full(),
            h_px(HEIGHT),
            bg(Role::Background),
            border_t(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(1),
                    h(10),
                    px(2),
                    flex_none(),
                    border_b(),
                    border_color(Role::Border),
                ])
                .child(
                    node()
                        .styles([
                            flex(),
                            items_center(),
                            gap(2),
                            h(8),
                            px(3),
                            rounded(Radius::Lg),
                            bg(Role::SurfaceSelected),
                        ])
                        .child(glyph(IconName::Terminal, 3.5, Role::Text))
                        .child(line("Terminal 1", 13, 1.4, Role::Text)),
                )
                .child(spacer())
                .child(Button::icon(IconName::Close, TERMINAL).build()),
        )
        .child(
            node()
                .styles([p(4)])
                .child(text(format!("{project} $")).styles([
                    text_px(13),
                    leading_ratio(1.5),
                    font_mono(),
                    text_color(Role::Muted),
                ])),
        )
}
