//! The scrolling body under a page's header, its content in a column of
//! bounded width.

use rustle::prelude::*;

use crate::size::Size;

/// How a page body places its column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageBodyVariant {
    /// Centred, no wider than the size allows.
    #[default]
    Centred,
    /// Across the whole width.
    Full,
}

/// A scrolling page body with a variant and a size.
#[derive(Clone)]
pub struct PageBody {
    /// How it places its column.
    variant: PageBodyVariant,
    /// How wide its column grows when centred.
    size: Size,
}

impl PageBody {
    /// A centred body of medium width.
    pub fn new() -> Self {
        Self {
            variant: PageBodyVariant::Centred,
            size: Size::Md,
        }
    }

    /// The body in `variant`.
    pub fn variant(mut self, variant: PageBodyVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The body at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the body around `content`, laid out as a column with room
    /// between its parts.
    #[track_caller]
    pub fn show(self, content: impl FnOnce()) {
        let scroll = remember_scroll_state();
        let widest = match (self.variant, self.size) {
            (PageBodyVariant::Full, _) => f32::INFINITY,
            (_, Size::Xs) => 560.0,
            (_, Size::Sm) => 720.0,
            (_, Size::Md) => 860.0,
            (_, Size::Lg) => 1080.0,
        };
        column_arranged(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .vertical_scroll(&scroll),
            Arrangement::Start,
            Alignment::Center,
            || {
                column_arranged(
                    Modifier::new()
                        .width_in(0.0, widest)
                        .fill_max_width()
                        .padding_each(20.0, 20.0, 20.0, 32.0),
                    Arrangement::SpacedBy(24.0),
                    Alignment::Start,
                    content,
                );
            },
        );
    }
}

impl Default for PageBody {
    /// [`PageBody::new`].
    fn default() -> Self {
        Self::new()
    }
}
