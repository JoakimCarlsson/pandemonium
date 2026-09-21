//! The header: the logo, the welcome line and the button out of the flow.

use pm_ui::{Div, Styled, Theme, button, h_flex, text, v_flex};

use crate::onboarding::mark::mark;
use crate::onboarding::setup::{Message, Setup};

/// Width of the button that leaves the setup flow.
const FINISH_WIDTH: f32 = 200.0;

/// The logo, the welcome line and the button out of the flow.
pub(super) fn header(theme: &Theme, setup: &Setup) -> Div<Message> {
    let finish = if setup.finished {
        button("Back to Setup", Message::Reopen)
            .outlined()
            .w_px(FINISH_WIDTH)
    } else {
        button("Finish Setup", Message::Finish)
            .filled()
            .w_px(FINISH_WIDTH)
    };

    h_flex()
        .w_full()
        .gap_4()
        .items_center()
        .justify_between()
        .child(
            h_flex().gap_4().items_center().child(mark(theme)).child(
                v_flex()
                    .gap_0p5()
                    .child(text("Welcome to Pandemonium").text_xxl().font_semibold())
                    .child(
                        text("Conduct agents where the code is")
                            .text_sm()
                            .italic()
                            .color(theme.colors.text_muted),
                    ),
            ),
        )
        .child(finish)
}
