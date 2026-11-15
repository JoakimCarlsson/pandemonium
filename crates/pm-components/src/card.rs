//! A bordered, rounded surface around other content.

use rustle::prelude::*;

use crate::size::Size;
use rustle::color_scheme;

/// How a card is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardVariant {
    /// The raised surface with a hairline border.
    #[default]
    Outlined,
    /// The raised surface without a border, as a chat bubble.
    Filled,
    /// The window background with a hairline border, as an input.
    Sunken,
}

/// A surface with a variant and a size.
#[derive(Clone)]
pub struct Card {
    /// How it is drawn.
    variant: CardVariant,
    /// Its padding and corner size.
    size: Size,
    /// Whether the border shows focus.
    focused: bool,
    /// The modifier it is laid out with.
    modifier: Modifier,
}

impl Card {
    /// An outlined card.
    pub fn new() -> Self {
        Self {
            variant: CardVariant::Outlined,
            size: Size::Sm,
            focused: false,
            modifier: Modifier::new(),
        }
    }

    /// The card in `variant`.
    pub fn variant(mut self, variant: CardVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The card at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The card with a focus outline when `focused`.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// The card laid out with `modifier` around it.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// Emits the card around `content`, laid out as a column.
    pub fn show(self, content: impl FnOnce()) {
        let scheme = color_scheme();
        let (padding_x, padding_y, radius) = match self.size {
            Size::Xs => (8.0, 4.0, 6.0),
            Size::Sm => (12.0, 8.0, 8.0),
            Size::Md => (14.0, 10.0, 10.0),
            Size::Lg => (16.0, 12.0, 12.0),
        };
        let fill = match self.variant {
            CardVariant::Outlined | CardVariant::Filled => scheme.surface_container,
            CardVariant::Sunken => scheme.background,
        };
        let mut modifier = self.modifier.background_rounded(fill, radius);
        if self.variant != CardVariant::Filled || self.focused {
            let outline = if self.focused {
                scheme.outline
            } else {
                scheme.outline_variant
            };
            modifier = modifier.border(1.0, outline, radius);
        }
        column_with(modifier.padding_xy(padding_x, padding_y), content);
    }
}

impl Default for Card {
    /// [`Card::new`].
    fn default() -> Self {
        Self::new()
    }
}
