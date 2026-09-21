//! A titled block: a title, an optional description and one control.

use pm_ui::{Div, IntoElement, Styled, Theme, text, v_flex};

use crate::onboarding::setup::Message;

/// A titled block: the title, an optional description and one control.
pub(super) fn section(
    theme: &Theme,
    title: &str,
    description: Option<&str>,
    control: impl IntoElement<Message>,
) -> Div<Message> {
    let mut block = v_flex().w_full().gap_2();
    let mut heading = v_flex().gap_0p5().child(text(title).font_medium());
    if let Some(description) = description {
        heading = heading.child(text(description).text_sm().color(theme.colors.text_muted));
    }

    block = block.child(heading).child(control);
    block
}
