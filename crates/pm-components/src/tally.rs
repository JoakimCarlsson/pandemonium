//! The tally: how many lines were added and how many removed.

use pm_style::{
    Role, flex, flex_none, font_mono, gap, items_center, leading_ratio, text_color, text_px,
};
use pm_ui::{Node, node, text};

/// A tally being described: the two counts and how big they are set.
///
/// Build it with [`Tally::new`], change it with [`Tally::size`], and turn it
/// into a node with [`Tally::build`].
#[derive(Clone, Copy, Debug)]
pub struct Tally {
    /// The lines added.
    added: usize,
    /// The lines removed.
    removed: usize,
    /// How tall the type is, in logical pixels.
    pixels: u32,
}

impl Tally {
    /// A tally of `added` lines added and `removed` lines removed, in 12px type.
    pub fn new(added: usize, removed: usize) -> Self {
        Self {
            added,
            removed,
            pixels: 12,
        }
    }

    /// The tally in type `pixels` tall.
    pub fn size(mut self, pixels: u32) -> Self {
        self.pixels = pixels;
        self
    }

    /// The node that draws `+added −removed`, the first in the success colour and the second in the danger colour.
    pub fn build(self) -> Node {
        let count = |content: String, tone: Role| {
            text(content).styles([
                text_px(self.pixels),
                leading_ratio(1.4),
                font_mono(),
                text_color(tone),
            ])
        };
        node()
            .styles([flex(), items_center(), gap(1.5), flex_none()])
            .child(count(format!("+{}", self.added), Role::Success))
            .child(count(format!("−{}", self.removed), Role::Danger))
    }
}
