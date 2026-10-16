//! The search box: a line to type what is looked for in, behind a magnifying glass, in an outlined box.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_none, gap, h, items_center, px, rounded,
    w_full,
};
use pm_text::Field;
use pm_ui::{Node, node};

use crate::input::{self, Input};
use crate::parts::glyph;

/// How big a search box is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Size {
    /// 32px tall, for a narrow column.
    Sm,
    /// 36px tall.
    #[default]
    Default,
    /// 40px tall, heading a page.
    Lg,
}

/// A search box being described: the line it edits, the name it answers to, what it says while empty and how big it is.
///
/// Build it with [`SearchBox::new`], change it with the methods that
/// follow, and turn it into a node with [`SearchBox::build`].
#[derive(Clone, Debug)]
pub struct SearchBox<'a> {
    /// The line being typed.
    field: &'a Field,
    /// The name it answers to.
    id: FocusId,
    /// What shows while it is empty.
    placeholder: &'a str,
    /// How big it is.
    size: Size,
}

impl<'a> SearchBox<'a> {
    /// A search box editing `field`, answering to `id`.
    pub fn new(field: &'a Field, id: FocusId) -> Self {
        Self {
            field,
            id,
            placeholder: "",
            size: Size::Default,
        }
    }

    /// The box saying `placeholder` while it is empty.
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// The box at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The node that draws the box.
    pub fn build(self) -> Node {
        let (height, inset) = match self.size {
            Size::Sm => (8, 2),
            Size::Default => (9, 3),
            Size::Lg => (10, 3),
        };
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                w_full(),
                h(height),
                px(inset),
                flex_none(),
                rounded(Radius::Lg),
                bg(Role::Surface),
                border(),
                border_color(Role::Border),
            ])
            .child(glyph(IconName::Search, 4.0, Role::Muted))
            .child(
                Input::new(self.field, self.id)
                    .variant(input::Variant::Ghost)
                    .size(input::Size::Sm)
                    .placeholder(self.placeholder)
                    .build(),
            )
    }
}
