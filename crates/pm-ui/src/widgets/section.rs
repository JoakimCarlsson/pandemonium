//! A titled block: a title, an optional description and one control.

use crate::div::{Div, v_flex};
use crate::element::IntoElement;
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;

/// A titled block: the title, an optional description and one control.
pub fn section<M: 'static>(
    theme: &Theme,
    title: &str,
    description: Option<&str>,
    control: impl IntoElement<M>,
) -> Div<M> {
    let mut heading = v_flex().gap(0.5).child(text(title).font_medium());
    if let Some(description) = description {
        heading = heading.child(text(description).text_sm().color(theme.colors.text_muted));
    }

    v_flex().w_full().gap(2).child(heading).child(control)
}
