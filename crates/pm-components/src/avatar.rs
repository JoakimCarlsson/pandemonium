//! The avatar: a project's colour with its first letter, at a measure per size.

use crate::kit;
use crate::theme;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, Color, row_arranged};

use crate::theme::Radius;

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
            size: Size::Sm,
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
            .size(theme::sp(side), theme::sp(side))
            .background_rounded(theme::color(self.color), Radius::Md.px());
        let initial = self.initial;
        row_arranged(own, Arrangement::Center, Alignment::Center, move || {
            kit::text(
                Modifier::new(),
                initial,
                theme::text_style(font, 1.2)
                    .font_weight(rustle::FontWeight::SEMI_BOLD)
                    .color(theme::color(Color::WHITE)),
            );
        });
    }
}

impl From<Avatar> for rustle::Node {
    /// The drawn avatar, held as a value.
    fn from(avatar: Avatar) -> Self {
        rustle::compose(move || avatar.build())
    }
}
