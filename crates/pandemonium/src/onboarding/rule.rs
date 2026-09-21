//! The hairline that separates one part of a page from the next.

use pm_ui::{Div, Styled, Theme, v_flex};

use crate::onboarding::setup::Message;

/// A hairline across the page.
pub(super) fn rule(theme: &Theme) -> Div<Message> {
    v_flex().w_full().h_px(1.0).bg(theme.colors.border_variant)
}
