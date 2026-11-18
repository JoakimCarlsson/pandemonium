//! The card: a rounded, bordered row that stands alone and holds its children side by side.

use crate::kit::sp;
use crate::kit::{Radius, Role, Styled};
use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};

/// A card being described: how tall it is.
///
/// Build it with [`Card::new`] and draw it with [`Card::build`], or with
/// [`Card::build_with`] when the caller adds what it holds. A card does not
/// answer to a press; a row that does is a [`Tile`](crate::Tile).
#[derive(Clone, Debug)]
pub struct Card {
    /// How tall it is, in steps of four logical pixels.
    height: u32,
    /// What the caller puts around the card's box, outside its own.
    modifier: Modifier,
    /// The padding that replaces the card's own, when the caller gave one.
    inner: Option<Modifier>,
    /// How the card lays out what it holds, in place of its own, when the caller gave one.
    arranged: Option<(Arrangement, Alignment)>,
}

impl Card {
    /// An empty card `height` steps tall.
    pub fn new(height: u32) -> Self {
        Self {
            height,
            modifier: Modifier::new(),
            inner: None,
            arranged: None,
        }
    }

    /// The card with `modifier` put around its box, outside its own: sizes, growth, margins, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The card with `modifier` in place of its own padding, innermost on its box.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = Some(modifier);
        self
    }

    /// The card spreading its children by `arrangement` and placing them across by `alignment` in place of its own.
    pub fn arranged(mut self, arrangement: Arrangement, alignment: Alignment) -> Self {
        self.arranged = Some((arrangement, alignment));
        self
    }

    /// Emits the card holding what `content` emits, side by side.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let own = Modifier::new()
            .h(self.height as f32)
            .fill_max_width()
            .stroke(Role::Border, Radius::Xl);
        let own = own.chain(self.inner.unwrap_or_else(|| Modifier::new().pad_x(4)));
        let (arrangement, alignment) = self
            .arranged
            .unwrap_or((Arrangement::SpacedBy(sp(3)), Alignment::Center));
        row_arranged(self.modifier.chain(own), arrangement, alignment, content);
    }

    /// Emits the empty card.
    #[track_caller]
    pub fn build(self) {
        self.build_with(|| {});
    }
}
