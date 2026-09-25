//! The screen a question is asked on: a card over a sheet, centred.
//!
//! The shape is Zed's, because it is the shape this question has everywhere:
//! what is being asked, a quieter line naming what it is about, and the
//! answers stacked beneath as full-width buttons with the one Enter takes
//! filled. Nothing else on the screen answers while it is up — the sheet
//! under it takes every click that misses.

use pm_ui::{Div, Font, Styled, TextSize, Theme, button, paragraph, v_flex};

use crate::message::Message;
use crate::prompt::state::Prompt;

/// How wide the card is drawn.
pub const WIDTH: f32 = 320.0;

/// Builds the card the question is asked on.
pub fn prompt(theme: &Theme, prompt: &Prompt, width: f32) -> Div<Message> {
    let active = prompt.active();

    v_flex()
        .w_px(width)
        .p(2)
        .gap(2)
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border_focused)
        .child(
            paragraph()
                .span(
                    prompt.message().to_owned(),
                    Font::new(TextSize::Base).weight(500),
                    theme.colors.text,
                )
                .break_long_words()
                .w_full(),
        )
        .when(!prompt.detail().is_empty(), |card| {
            card.child(
                v_flex()
                    .w_full()
                    .gap(0.25)
                    .children(prompt.detail().iter().map(|line| {
                        paragraph()
                            .span(
                                line.clone(),
                                Font::new(TextSize::Sm).mono(),
                                theme.colors.text_muted,
                            )
                            .break_long_words()
                            .w_full()
                    })),
            )
        })
        .child(
            v_flex()
                .w_full()
                .gap(1)
                .children(prompt.answers().iter().enumerate().map(|(place, answer)| {
                    let taken = button(answer.label.clone(), Message::ChoosePrompt(place)).w_full();
                    match place == active {
                        true => taken.filled(),
                        false => taken.outlined(),
                    }
                })),
        )
}
