//! The settings row a switch lives in: a title, a description and the switch.

use crate::div::{Div, h_flex, v_flex};
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::switch::switch;

/// A settings row: a title, a description under it and a switch on the right.
pub fn switch_field<M: Clone + 'static>(
    theme: &Theme,
    title: Option<&str>,
    description: &str,
    on: bool,
    message: M,
) -> Div<M> {
    let mut labels = v_flex().gap_0p5().flex_1();
    if let Some(title) = title {
        labels = labels.child(text(title).font_medium());
    }
    labels = labels.child(text(description).text_sm().color(theme.colors.text_muted));

    h_flex()
        .w_full()
        .gap_4()
        .items_center()
        .justify_between()
        .child(labels)
        .child(switch(on, message))
}
