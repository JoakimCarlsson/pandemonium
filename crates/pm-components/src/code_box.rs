//! The code box: lines in the code font inside a bordered box that cuts them off at a height.

use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_col, max_h, my, overflow_hidden, rounded,
};
use pm_ui::{Node, node};

/// A code box being described: the lines it holds and how tall it may grow.
///
/// Build it with [`CodeBox::new`], change it with [`CodeBox::max_height`],
/// and turn it into a node with [`CodeBox::build`].
#[derive(Clone, Debug)]
pub struct CodeBox {
    /// The lines, top to bottom.
    lines: Vec<Node>,
    /// The height past which the lines are cut off, in logical pixels.
    height: f32,
}

impl CodeBox {
    /// A box of `lines`, cut off at 280 logical pixels.
    pub fn new(lines: impl IntoIterator<Item = Node>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
            height: 280.0,
        }
    }

    /// The box cut off at `pixels` logical pixels tall.
    pub fn max_height(mut self, pixels: f32) -> Self {
        self.height = pixels;
        self
    }

    /// The node that draws the box.
    pub fn build(self) -> Node {
        node()
            .styles([
                flex(),
                flex_col(),
                my(1),
                rounded(Radius::Lg),
                border(),
                border_color(Role::Border),
                bg(Role::Background),
                max_h(self.height / 4.0),
                overflow_hidden(),
            ])
            .children(self.lines)
    }
}
