//! The surface: the raised, bordered column that menus, popovers and modals are drawn on.

use pm_style::{
    Radius, Role, ShadowLayer, bg, border, border_color, flex, flex_col, overflow_hidden, rounded,
    shadow_layers, shadow_md,
};
use pm_ui::{Node, node};

/// The shadow under a floating surface: a tall, soft one that lifts it off what is behind.
const FLOATING_SHADOW: [ShadowLayer; 1] = [ShadowLayer::new(0.0, 12.0, 40.0, 0.0, 0.35, false)];

/// How a surface looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// Small corners and a short shadow, for a list of actions.
    #[default]
    Compact,
    /// Large corners and a tall, soft shadow, for a panel of choices that floats over a screen.
    Floating,
    /// Large corners, a short shadow and a clip at its edge, for a panel that holds a whole task.
    Dialog,
}

/// A surface being described: how it looks.
///
/// Build it with [`Surface::new`], change it with [`Surface::variant`], and
/// turn it into a node with [`Surface::build`]; the caller adds what it holds,
/// the room around it and its place.
#[derive(Clone, Copy, Debug, Default)]
pub struct Surface {
    /// How it looks.
    variant: Variant,
}

impl Surface {
    /// A compact surface.
    pub fn new() -> Self {
        Self::default()
    }

    /// The surface looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The node that draws the surface: a column on the surface colour, bordered and shadowed.
    pub fn build(self) -> Node {
        let base = node().styles([
            flex(),
            flex_col(),
            bg(Role::Surface),
            border(),
            border_color(Role::Border),
        ]);
        match self.variant {
            Variant::Compact => base.styles([rounded(Radius::Md), shadow_md()]),
            Variant::Floating => {
                base.styles([rounded(Radius::Xl), shadow_layers(&FLOATING_SHADOW)])
            }
            Variant::Dialog => base.styles([rounded(Radius::Xl), shadow_md(), overflow_hidden()]),
        }
    }
}

impl From<Surface> for Node {
    /// The node that draws the surface.
    fn from(surface: Surface) -> Self {
        surface.build()
    }
}
