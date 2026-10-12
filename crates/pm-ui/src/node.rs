//! The node: a style, what it holds and whether it answers to the user.

use pm_input::FocusId;
use pm_style::layout::Item;
use pm_style::{Style, Utility};
use pm_text::Shaper;

/// One box of the tree: how it is styled and what it holds.
#[derive(Clone, Debug, Default)]
pub struct Node {
    /// How the box is laid out, painted and typeset.
    pub(crate) style: Style,
    /// The boxes inside it, in layout order.
    pub(crate) children: Vec<Node>,
    /// A line of text the box holds instead of children.
    pub(crate) text: Option<String>,
    /// What names the box to the user: it can be pressed and takes focus.
    pub(crate) focus: Option<FocusId>,
    /// A blinking caret at this byte offset of the text, shown while the
    /// named box holds focus.
    pub(crate) caret: Option<(usize, FocusId)>,
}

/// An empty, unstyled box.
pub fn node() -> Node {
    Node::default()
}

/// A box holding one line of `content`; style it with typography utilities.
pub fn text(content: impl Into<String>) -> Node {
    Node {
        text: Some(content.into()),
        ..Node::default()
    }
}

impl Node {
    /// Applies utilities in order, after whatever was applied before.
    pub fn styles(mut self, utilities: impl IntoIterator<Item = Utility>) -> Self {
        self.style.extend(utilities);
        self
    }

    /// Appends one child.
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// Appends every child of `children`.
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    /// Names the box `id`: it can then be pressed, and takes keyboard focus in
    /// tree order.
    pub fn focusable(mut self, id: FocusId) -> Self {
        self.focus = Some(id);
        self
    }

    /// Shows a blinking caret at byte offset `byte` of this node's text while
    /// the box named `owner` holds focus.
    pub fn caret(mut self, byte: usize, owner: FocusId) -> Self {
        self.caret = Some((byte, owner));
        self
    }

    /// The box this node and its children ask the layout for.
    pub(crate) fn item(&self, shaper: &mut Shaper) -> Item {
        let base = self.style.base();
        let intrinsic = self.text.as_deref().map(|content| {
            let extent = shaper.measure(content, &base.typography);
            (extent.width, extent.height)
        });
        Item {
            style: base.layout,
            border: base.decoration.borders.width,
            intrinsic,
            children: self
                .children
                .iter()
                .map(|child| child.item(shaper))
                .collect(),
        }
    }
}
