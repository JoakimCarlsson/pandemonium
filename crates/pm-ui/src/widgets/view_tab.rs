//! One view of a bar that shows one view at a time.
//!
//! The bottom panel and the sidebar beside the panes each name their views
//! along a bar and show one of them; the one in front is underlined, the way
//! a tab stands on what it opens, rather than filled in like a button.

use crate::div::{Div, h_flex};
use crate::style::{Side, Styled};
use crate::text::text;
use crate::theme::Theme;

/// Thickness of the line under the view in front.
const UNDERLINE: f32 = 2.0;

/// Builds the label of one view, underlined while it is `chosen`, with
/// `badge` beside its name when there is something to count.
pub fn view_tab<M: Clone + 'static>(
    theme: &Theme,
    label: &str,
    chosen: bool,
    badge: Option<Div<M>>,
    message: M,
) -> Div<M> {
    let color = match chosen {
        true => theme.colors.text,
        false => theme.colors.text_muted,
    };

    h_flex()
        .h_full()
        .px(2)
        .gap(1)
        .items_center()
        .justify_center()
        .hover_bg(theme.colors.surface_hover)
        .when(chosen, |tab| {
            tab.border_side(Side::Bottom, UNDERLINE, theme.colors.border_focused)
        })
        .on_click(message)
        .child(text(label).text_sm().color(color))
        .when_some(badge, |row, badge| row.child(badge))
}
