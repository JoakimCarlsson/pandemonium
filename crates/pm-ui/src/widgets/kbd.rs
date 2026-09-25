//! The keys a command answers to, drawn as the caps they are pressed on.

use crate::div::{Div, h_flex};
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;

/// Height of the outlined cap the keys are written in.
const CAP_HEIGHT: f32 = 20.0;

/// Builds the outlined cap `keys` are written in.
pub fn kbd<M: Clone + 'static>(theme: &Theme, keys: impl Into<String>) -> Div<M> {
    h_flex()
        .h_px(CAP_HEIGHT)
        .px(1.5)
        .items_center()
        .rounded(theme.radius.md)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .child(
            text(keys)
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
}
