//! The logo mark.

use pm_ui::{Div, Styled, Theme, space, text, v_flex};

use crate::onboarding::setup::Message;

/// The logo: an accent tile with the editor's initial in it.
pub(super) fn mark(theme: &Theme) -> Div<Message> {
    v_flex()
        .size_px(space(10.0))
        .items_center()
        .justify_center()
        .rounded(theme.radius.lg)
        .bg(theme.colors.accent)
        .child(
            text("P")
                .text_xl()
                .font_bold()
                .color(theme.colors.text_on_accent),
        )
}
