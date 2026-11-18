//! The avatar: a project's colour with its first letter, at a measure per size.

use rustle::Modifier;

use crate::kit::{Flex, Radius, Styled, Tone, Type, label, row};

use crate::Project;
use crate::size::Size;

/// An avatar being described: the letter it shows, its colour and its size.
///
/// Build it with [`Avatar::new`] or [`Avatar::of`], change it with
/// [`Avatar::size`], and emit it with [`Avatar::build`].
#[derive(Clone, Debug)]
pub struct Avatar {
    /// The first letter of the name, in capitals.
    initial: String,
    /// What it is filled with.
    color: Tone,
    /// How big it is.
    size: Size,
    /// What the caller puts around its box, outside its own.
    modifier: Modifier,
}

impl Avatar {
    /// An avatar showing the first letter of `name` on `color`.
    pub fn new(name: &str, color: Tone) -> Self {
        Self {
            initial: name
                .chars()
                .next()
                .map(|initial| initial.to_uppercase().to_string())
                .unwrap_or_default(),
            color,
            size: Size::Sm,
            modifier: Modifier::new(),
        }
    }

    /// The avatar of `project`: its name's first letter on the colour its path gives it.
    pub fn of(project: &Project) -> Self {
        Self::new(&project.name, project.color())
    }

    /// The avatar at `size`: a square 16, 20, 24 or 32px on a side.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The avatar with `modifier` put around its box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the avatar: its letter centred on its colour.
    #[track_caller]
    pub fn build(self) {
        let (side, font) = match self.size {
            Size::Xs => (4.0, 11),
            Size::Sm => (5.0, 13),
            Size::Md => (6.0, 15),
            Size::Lg => (8.0, 18),
        };
        let own = Modifier::new()
            .square(side)
            .fill_rounded(self.color, Radius::Md);
        let initial = self.initial;
        row(
            self.modifier.chain(own),
            Flex::new().center().middle(),
            move || {
                label(
                    initial,
                    Type::new(font).ratio(1.2).semibold().tone(Tone::White),
                );
            },
        );
    }
}

impl From<Avatar> for rustle::Node {
    /// The drawn avatar, held as a value.
    fn from(avatar: Avatar) -> Self {
        rustle::compose(move || avatar.build())
    }
}
