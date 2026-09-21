//! A square control carrying one icon.

use crate::div::{Div, v_flex};
use crate::icons::{IconName, IconSize, icon};
use crate::style::Styled;
use crate::theme::Theme;

/// A control of `name` that sends `message` when it is clicked.
pub fn icon_button<M>(theme: &Theme, name: IconName, message: M) -> Div<M> {
    v_flex()
        .size_px(theme.size.icon_control)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(message)
        .child(
            icon(name)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
}
