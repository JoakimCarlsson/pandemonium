//! The frame of a window beside the main one: the title bar that closes it over what it holds, with the border the window draws itself when it has no frame from the desktop.

use pm_style::{Role, bg, border, border_color, flex, flex_1, flex_col, h_full, min_h, w_full};
use pm_ui::{Node, node};

use crate::TitleBar;
use crate::title_bar::FRAMELESS;

/// A window's frame being described: what its body holds, and what is laid over the body.
///
/// Build it with [`Window::new`], fill it with [`Window::body`], lay menus
/// over it with [`Window::over`], and turn it into a node with
/// [`Window::build`]. The node fills the window.
#[derive(Clone, Debug)]
pub struct Window {
    /// What fills the window under the title bar.
    body: Node,
    /// What is laid over the window, such as menus.
    over: Vec<Node>,
}

impl Window {
    /// A frame with nothing in it.
    pub fn new() -> Self {
        Self {
            body: node().styles([flex(), flex_col(), flex_1(), min_h(0), w_full()]),
            over: Vec::new(),
        }
    }

    /// The frame with `child` filling the window under the title bar.
    pub fn body(mut self, child: Node) -> Self {
        self.body = self.body.child(child);
        self
    }

    /// The frame with every node of `nodes` laid over the window.
    pub fn over(mut self, nodes: impl IntoIterator<Item = Node>) -> Self {
        self.over.extend(nodes);
        self
    }

    /// The node that draws the frame.
    pub fn build(self) -> Node {
        node()
            .styles([flex(), flex_col(), w_full(), h_full(), bg(Role::Surface)])
            .styles(
                FRAMELESS
                    .then(|| [border(), border_color(Role::Border)])
                    .into_iter()
                    .flatten(),
            )
            .child(TitleBar::closing().build())
            .child(self.body)
            .children(self.over)
    }
}

impl Default for Window {
    /// A frame with nothing in it.
    fn default() -> Self {
        Self::new()
    }
}
