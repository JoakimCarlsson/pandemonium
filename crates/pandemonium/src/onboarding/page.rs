//! The page itself: the header, a rule and whichever body the state asks for.

use pm_ui::{Div, Styled, Theme, v_flex};

use crate::onboarding::basics::basics;
use crate::onboarding::header::header;
use crate::onboarding::ready::ready;
use crate::onboarding::rule::rule;
use crate::onboarding::setup::{Message, Setup};

/// Width the page is capped at, however wide the window is.
const PAGE_WIDTH: f32 = 780.0;

/// Builds the page for `setup`, drawn in `theme`.
pub fn page(theme: &Theme, setup: &Setup) -> Div<Message> {
    let body = if setup.finished {
        ready(theme, setup)
    } else {
        basics(theme, setup)
    };

    v_flex().w_full().child(
        v_flex()
            .w_full()
            .max_w_px(PAGE_WIDTH)
            .mx_auto()
            .p_12()
            .gap_6()
            .child(header(theme, setup))
            .child(rule(theme))
            .child(body),
    )
}
