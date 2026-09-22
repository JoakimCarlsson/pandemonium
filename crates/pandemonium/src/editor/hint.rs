//! The small panel that says what the editor knows about one place.
//!
//! A hover, a signature and the text of a diagnostic are three things a
//! server says about where the cursor is and one way of showing them: a
//! panel beside the place, holding a few lines of plain text. It is not a
//! document view — a hover that runs to forty lines is a hover the reader
//! reads the top of and then goes to the definition.

use pm_ui::{Div, Styled, Theme, text, v_flex};

use crate::message::Message;

/// Widest the panel is drawn.
const WIDTH: f32 = 520.0;

/// Most lines of it shown, however much the server said.
const LINES: usize = 16;

/// Builds the panel saying `content`, beside the place it is about.
pub fn hint(theme: &Theme, content: &str) -> Div<Message> {
    let lines = content
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.starts_with("```"))
        .take(LINES)
        .map(|line| {
            text(line.to_owned())
                .text_sm()
                .font_mono()
                .color(theme.colors.text_muted)
        })
        .collect::<Vec<_>>();

    v_flex()
        .max_w_px(WIDTH)
        .px(1.5)
        .py(1)
        .gap(0.25)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.md)
        .on_click(Message::DismissPopup)
        .children(lines)
}
