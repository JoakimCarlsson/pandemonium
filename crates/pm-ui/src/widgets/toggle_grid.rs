//! A grid of options with one of them selected.

use crate::div::{Div, h_flex, v_flex};
use crate::style::{Style, Styled};
use crate::widgets::toggle_option::ToggleOption;

/// A grid of options `columns` wide, of which `selected` is the current one.
pub fn toggle_grid<M, I>(options: I, selected: Option<usize>, columns: usize) -> Div<M>
where
    M: Clone + 'static,
    I: IntoIterator<Item = (String, M)>,
{
    let options: Vec<_> = options.into_iter().collect();
    let mut rows = v_flex().gap_1().w_full();
    let mut offset = 0;

    for row in options.chunks(columns.max(1)) {
        let mut line = h_flex().gap_1().w_full();
        for (index, (label, message)) in row.iter().enumerate() {
            line = line.child(ToggleOption {
                label: label.clone(),
                message: message.clone(),
                selected: selected == Some(offset + index),
                style: Style::default(),
            });
        }
        rows = rows.child(line);
        offset += row.len();
    }

    rows
}
