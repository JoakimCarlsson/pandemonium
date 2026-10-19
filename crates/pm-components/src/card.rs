//! The card: a rounded, bordered row that stands alone and holds its children side by side.

use pm_style::{
    Radius, Role, border, border_color, flex, gap, h, items_center, px, rounded, w_full,
};
use pm_ui::{Node, node};

/// A card being described: how tall it is.
///
/// Build it with [`Card::new`] and turn it into a node with [`Card::build`];
/// the caller adds what it holds. A card does not answer to a press; a row
/// that does is a [`Tile`](crate::Tile).
#[derive(Clone, Copy, Debug)]
pub struct Card {
    /// How tall it is, in steps of four logical pixels.
    height: u32,
}

impl Card {
    /// An empty card `height` steps tall.
    pub fn new(height: u32) -> Self {
        Self { height }
    }

    /// The node that draws the card.
    pub fn build(self) -> Node {
        node().styles([
            flex(),
            items_center(),
            gap(3),
            h(self.height),
            px(4),
            w_full(),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
        ])
    }
}

impl From<Card> for Node {
    /// The node that draws the card.
    fn from(card: Card) -> Self {
        card.build()
    }
}
