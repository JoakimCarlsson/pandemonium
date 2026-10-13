//! The strip: a bordered, rounded row holding its children side by side.

use pm_style::{
    Radius, Role, border, border_color, flex, gap, h, items_center, px, rounded, w_full,
};
use pm_ui::{Node, node};

/// A strip being described: how tall it is.
///
/// Build it with [`Strip::new`] and turn it into a node with
/// [`Strip::build`]; the caller adds what it holds.
#[derive(Clone, Copy, Debug)]
pub struct Strip {
    /// How tall it is, in steps of four logical pixels.
    height: u32,
}

impl Strip {
    /// An empty strip `height` steps tall.
    pub fn new(height: u32) -> Self {
        Self { height }
    }

    /// The node that draws the strip.
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
