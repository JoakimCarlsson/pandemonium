//! Every theme family on offer, side by side, one of them chosen.

use crate::div::{Div, h_flex};
use crate::style::Styled;
use crate::theme::{Appearance, families};
use crate::widgets::theme_preview::theme_preview;

/// A row of previews of every family, `selected` among them.
///
/// `shown` is the appearance each tile is painted in, or none to split each
/// tile into both; `choose` is what picking the family at an index sends.
pub fn theme_gallery<M: Clone + 'static>(
    shown: Option<Appearance>,
    selected: usize,
    choose: impl Fn(usize) -> M,
) -> Div<M> {
    let previews = families()
        .iter()
        .enumerate()
        .map(|(index, family)| {
            theme_preview(*family, shown, index == selected, choose(index)).flex_1()
        })
        .collect::<Vec<_>>();

    h_flex().w_full().gap(2).children(previews)
}
