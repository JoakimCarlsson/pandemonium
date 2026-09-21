//! A row of options with one of them selected.

use crate::div::{Div, h_flex};
use crate::style::{Style, Styled};
use crate::widgets::toggle_option::ToggleOption;

/// A row of options, of which `selected` is the current one.
pub fn toggle_row<M, I>(options: I, selected: Option<usize>) -> Div<M>
where
    M: Clone + 'static,
    I: IntoIterator<Item = (String, M)>,
{
    h_flex().gap(1).children(
        options
            .into_iter()
            .enumerate()
            .map(|(index, (label, message))| ToggleOption {
                label,
                message,
                selected: selected == Some(index),
                style: Style::default(),
            }),
    )
}
