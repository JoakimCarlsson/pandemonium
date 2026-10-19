//! The modal: a panel centred over a darkened window, which pressing outside closes.

use pm_input::FocusId;
use pm_style::{
    Color, absolute, bg, flex, flex_none, inset, items_center, justify_center, w_px, z,
};
use pm_ui::{Node, node};

use crate::{Surface, surface};

/// How dark the window behind a modal is made, from 0.0 to 1.0.
const DIM: f32 = 0.55;

/// A modal being described: how wide its panel is and what the panel holds.
///
/// Build it with [`Modal::new`], fill it with [`Modal::child`] and
/// [`Modal::children`], and turn it into a node with [`Modal::build`]. The
/// node covers its parent, so it belongs last in the root of a screen.
#[derive(Clone, Debug)]
pub struct Modal {
    /// What pressing the dimmed window outside the panel does.
    outside: FocusId,
    /// The panel itself, which keeps a press on it from counting as outside.
    panel: Node,
}

impl Modal {
    /// A modal whose dimmed window answers to `outside` and whose panel answers to `inside`.
    pub fn new(outside: FocusId, inside: FocusId) -> Self {
        Self {
            outside,
            panel: Surface::new()
                .variant(surface::Variant::Dialog)
                .build()
                .styles([flex_none()])
                .focusable(inside),
        }
    }

    /// The modal with a panel `pixels` wide.
    pub fn width(mut self, pixels: f32) -> Self {
        self.panel = self.panel.styles([w_px(pixels)]);
        self
    }

    /// The modal with `child` after what its panel holds.
    pub fn child(mut self, child: Node) -> Self {
        self.panel = self.panel.child(child);
        self
    }

    /// The modal with every node of `children` after what its panel holds.
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.panel = self.panel.children(children);
        self
    }

    /// The node that draws the modal.
    pub fn build(self) -> Node {
        node()
            .styles([
                absolute(),
                inset(0),
                z(70),
                flex(),
                items_center(),
                justify_center(),
                bg(Color::Black.alpha(DIM)),
            ])
            .child(self.panel)
            .focusable(self.outside)
    }
}

impl From<Modal> for Node {
    /// The node that draws the modal.
    fn from(modal: Modal) -> Self {
        modal.build()
    }
}
