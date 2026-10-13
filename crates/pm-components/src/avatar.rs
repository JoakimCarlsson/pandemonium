//! The avatar: a project's colour with its first letter, at a measure per size.

use pm_style::{
    Color, Radius, bg, flex, font_semibold, items_center, justify_center, leading_ratio, rounded,
    size, text_color, text_px,
};
use pm_ui::{Node, node, text};

use crate::Project;

/// How big an avatar is.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Size {
    /// A 20px square with a 13px letter: sits in a row.
    #[default]
    Default,
    /// A 32px square with an 18px letter: stands on its own.
    Lg,
}

/// An avatar being described: the letter it shows, its colour and its size.
///
/// Build it with [`Avatar::new`] or [`Avatar::of`], change it with
/// [`Avatar::size`], and turn it into a node with [`Avatar::build`].
#[derive(Clone, Debug)]
pub struct Avatar {
    /// The first letter of the name, in capitals.
    initial: String,
    /// What it is filled with.
    color: Color,
    /// How big it is.
    size: Size,
}

impl Avatar {
    /// An avatar showing the first letter of `name` on `color`.
    pub fn new(name: &str, color: Color) -> Self {
        Self {
            initial: name
                .chars()
                .next()
                .map(|initial| initial.to_uppercase().to_string())
                .unwrap_or_default(),
            color,
            size: Size::Default,
        }
    }

    /// The avatar of `project`: its name's first letter on the colour its path gives it.
    pub fn of(project: &Project) -> Self {
        Self::new(&project.name, project.color())
    }

    /// The avatar at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The node that draws the avatar.
    pub fn build(self) -> Node {
        let (side, font) = match self.size {
            Size::Default => (5.0, 13),
            Size::Lg => (8.0, 18),
        };
        node()
            .styles([
                flex(),
                items_center(),
                justify_center(),
                size(side),
                rounded(Radius::Md),
                bg(self.color),
            ])
            .child(text(self.initial).styles([
                text_px(font),
                leading_ratio(1.2),
                font_semibold(),
                text_color(Color::White),
            ]))
    }
}

impl From<Avatar> for Node {
    /// The node that draws the avatar.
    fn from(avatar: Avatar) -> Self {
        avatar.build()
    }
}
