//! The avatar: a project's colour with its first letter.

use rustle::prelude::*;

use crate::size::Size;

/// A rounded square in a colour with an initial on it.
#[derive(Clone, Debug)]
pub struct Avatar {
    /// The uppercase first letter of the name.
    initial: String,
    /// The fill.
    colour: Color,
    /// Its size.
    size: Size,
}

impl Avatar {
    /// An avatar for `name` filled with `colour`.
    pub fn new(name: &str, colour: Color) -> Self {
        Self {
            initial: name
                .chars()
                .next()
                .map(|initial| initial.to_uppercase().to_string())
                .unwrap_or_default(),
            colour,
            size: Size::Sm,
        }
    }

    /// The avatar at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the avatar.
    pub fn show(self) {
        let (side, font) = match self.size {
            Size::Xs => (14.0, 9.0),
            Size::Sm => (18.0, 11.0),
            Size::Md => (24.0, 15.0),
            Size::Lg => (32.0, 18.0),
        };
        let initial = self.initial;
        stack_aligned(
            Modifier::new()
                .size(side, side)
                .background_rounded(self.colour, side * 0.3),
            BoxAlignment::Center,
            move || {
                text_styled(
                    Modifier::new(),
                    initial,
                    TextStyle {
                        color: Color::rgb(255, 255, 255),
                        font_size: font,
                        line_height: font * 1.2,
                        font_weight: FontWeight::SEMI_BOLD,
                        ..TextStyle::default()
                    },
                );
            },
        );
    }
}
