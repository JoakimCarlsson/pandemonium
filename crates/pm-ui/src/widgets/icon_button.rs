//! A square control carrying one icon.

use pm_gfx::Rgba;

use crate::div::{Div, v_flex};
use crate::icons::{IconName, IconSize, icon};
use crate::style::Styled;
use crate::theme::Theme;

/// A control of `name` that sends `message` when it is clicked.
pub fn icon_button<M>(theme: &Theme, name: IconName, message: M) -> Div<M> {
    tinted_icon_button(theme, name, theme.colors.text_subtle, message)
}

/// The same control, with its icon drawn in `color` rather than the quiet one.
///
/// A control whose icon says something by its colour — a pin that is in, a
/// mark that is lit — is the same square as any other; only the tint differs.
/// The tint is ink: a text colour, never the accent, which a theme is free to
/// make as dark as its own background.
pub fn tinted_icon_button<M>(theme: &Theme, name: IconName, color: Rgba, message: M) -> Div<M> {
    v_flex()
        .size_px(theme.size.icon_control)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(message)
        .child(icon(name).size(IconSize::Medium).color(color))
}

/// The same control, turned `turn` radians round while what it started is
/// under way, or at rest and pressable when `turn` is `None`.
///
/// While it turns it is neither lit nor pressable: the turning is what says
/// it was pressed, and a second press would only ask for what is already
/// being done.
pub fn turning_icon_button<M>(
    theme: &Theme,
    name: IconName,
    turn: Option<f32>,
    message: M,
) -> Div<M> {
    let Some(turn) = turn else {
        return icon_button(theme, name, message);
    };
    v_flex()
        .size_px(theme.size.icon_control)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .child(
            icon(name)
                .size(IconSize::Medium)
                .color(theme.colors.text_subtle)
                .rotate(turn),
        )
}
