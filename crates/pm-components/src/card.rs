//! The card: a raised surface that groups what belongs together.

use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_col, font_semibold, gap, p, rounded,
    shadow_lg, text_color, text_sm,
};
use pm_ui::{Node, node, text};

/// A padded, rounded surface with a hairline border and a shadow, headed by
/// `title` and stacking its children below it with a gap between them.
pub fn card(title: &str) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            gap(4),
            p(5),
            rounded(Radius::Xl),
            bg(Role::Surface),
            border(),
            border_color(Role::Border),
            shadow_lg(),
        ])
        .child(text(title).styles([text_sm(), font_semibold(), text_color(Role::Muted)]))
}
