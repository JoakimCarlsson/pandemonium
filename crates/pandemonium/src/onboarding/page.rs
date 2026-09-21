//! The page and its chrome: the mark, the header and the page setup leaves.

use pm_ui::{Div, Styled, Theme, button, h_flex, rule, space, text, v_flex};

use crate::onboarding::basics::basics;
use crate::onboarding::setup::{Message, Setup};

/// Width the page is capped at, however wide the window is.
const PAGE_WIDTH: f32 = 780.0;

/// Width of the button that leaves the setup flow.
const FINISH_WIDTH: f32 = 200.0;

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

/// The logo, the welcome line and the button out of the flow.
fn header(theme: &Theme, setup: &Setup) -> Div<Message> {
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

/// The logo: an accent tile with the editor's initial in it.
fn mark(theme: &Theme) -> Div<Message> {
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

/// The page setup leaves behind: what was chosen, and the way back.
fn ready(theme: &Theme, setup: &Setup) -> Div<Message> {
    v_flex()
        .w_full()
        .gap_2()
        .p_4()
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .child(text("Setup finished").font_medium())
        .child(
            text(format!(
                "{} keymap · {}",
                setup.keymap.label(),
                if setup.vim_mode {
                    "vim mode on"
                } else {
                    "vim mode off"
                },
            ))
            .text_sm()
            .color(theme.colors.text_muted),
        )
        .child(
            text("Projects and sessions land here next.")
                .text_sm()
                .color(theme.colors.text_subtle),
        )
}
