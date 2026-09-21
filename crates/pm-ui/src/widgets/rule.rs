//! The hairline that separates one part of a screen from the next.

use crate::div::{Div, v_flex};
use crate::style::Styled;
use crate::theme::Theme;

/// A hairline across whatever the parent offers.
pub fn rule<M>(theme: &Theme) -> Div<M> {
    v_flex().w_full().h_px(1.0).bg(theme.colors.border_variant)
}
