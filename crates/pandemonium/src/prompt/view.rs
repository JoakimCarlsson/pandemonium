//! The screen a question is asked on: a card over a sheet, centred.
//!
//! The shape is Zed's, because it is the shape this question has everywhere:
//! what is being asked, a quieter line naming what it is about, and the
//! answers stacked beneath as full-width buttons with the one Enter takes
//! filled. Nothing else on the screen answers while it is up — the sheet
//! under it takes every click that misses.

use pm_ui::{Div, Styled, Theme, button, text, v_flex};

use crate::message::Message;
use crate::prompt::state::Prompt;

/// How wide the card is drawn.
pub const WIDTH: f32 = 320.0;

/// How tall one answer's button is, for placing the card before it is laid out.
const ANSWER_HEIGHT: f32 = 40.0;

/// How tall the card is above its answers and the lines under the question.
const HEADING_HEIGHT: f32 = 56.0;

/// How tall one of those lines is.
const DETAIL_HEIGHT: f32 = 18.0;

/// Builds the card the question is asked on.
pub fn prompt(theme: &Theme, prompt: &Prompt) -> Div<Message> {
    let active = prompt.active();

    v_flex()
        .w_px(WIDTH)
        .p(2)
        .gap(2)
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border_focused)
        .child(text(prompt.message().to_owned()).font_medium())
        .when(!prompt.detail().is_empty(), |card| {
            card.child(
                v_flex()
                    .w_full()
                    .gap(0.25)
                    .children(prompt.detail().iter().map(|line| {
                        text(line.clone())
                            .text_sm()
                            .font_mono()
                            .color(theme.colors.text_muted)
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

/// How tall the card asking `prompt` comes out, near enough to centre it by.
///
/// The card is placed rather than laid out, so where it goes is worked out
/// before anything has been measured: a question is a heading, a line about
/// what it is about and a button each, and none of them wraps.
pub fn height(prompt: &Prompt) -> f32 {
    HEADING_HEIGHT
        + prompt.detail().len() as f32 * DETAIL_HEIGHT
        + prompt.answers().len() as f32 * ANSWER_HEIGHT
}
