//! The pane a picture is looked at in.
//!
//! The picture is drawn as large as the pane allows and never larger than it
//! is, centred, with what it is beneath it: how many pixels, and how much of
//! the disk. A picture the editor cannot read says why in the same place.

use pm_ui::{Div, Styled, Theme, h_flex, picture, text, v_flex};

use crate::image::{Decoding, Shown};
use crate::message::Message;

/// Builds the pane showing `shown`.
pub fn image_pane(theme: &Theme, shown: Shown) -> Div<Message> {
    let body = match &shown.picture {
        Decoding::Ready(image) => h_flex()
            .w_full()
            .flex_1()
            .p(4)
            .items_center()
            .justify_center()
            .overflow_hidden()
            .child(picture(image.clone())),
        Decoding::Pending => note(theme, "Loading…".to_owned()),
        Decoding::Failed(said) => note(theme, said.clone()),
    };
    let measures = match &shown.picture {
        Decoding::Ready(image) => format!(
            "{} × {} · {}",
            image.width(),
            image.height(),
            size(shown.bytes)
        ),
        _ => size(shown.bytes),
    };

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(body)
        .child(
            h_flex()
                .w_full()
                .h_px(theme.size.row)
                .px(1.5)
                .gap(1)
                .items_center()
                .bg(theme.colors.surface)
                .child(
                    text(shown.name)
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_muted),
                )
                .child(h_flex().flex_1())
                .child(
                    text(measures)
                        .text_xs()
                        .font_mono()
                        .color(theme.colors.text_subtle),
                ),
        )
}

/// A line of `said` in the middle of the pane, where the picture would be.
fn note(theme: &Theme, said: String) -> Div<Message> {
    h_flex()
        .w_full()
        .flex_1()
        .items_center()
        .justify_center()
        .child(
            text(said)
                .text_sm()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// `bytes` in the unit that reads easiest.
fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut amount = bytes as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{bytes} B"),
        _ => format!("{amount:.1} {}", UNITS[unit]),
    }
}
