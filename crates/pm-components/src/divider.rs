//! A hairline between rows or groups.

use rustle::prelude::*;

use rustle::color_scheme;

/// How a divider is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DividerVariant {
    /// The border tone.
    #[default]
    Border,
    /// A fainter line, for rows inside a card.
    Faint,
}

/// A horizontal hairline with a variant.
#[derive(Clone)]
pub struct Divider {
    /// How it is drawn.
    variant: DividerVariant,
    /// Space on either side of it, along its length.
    inset: f32,
}

impl Divider {
    /// A divider in the border tone.
    pub fn new() -> Self {
        Self {
            variant: DividerVariant::Border,
            inset: 0.0,
        }
    }

    /// The divider in `variant`.
    pub fn variant(mut self, variant: DividerVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The divider kept `inset` away from either end.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    /// Emits the divider.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let colour = match self.variant {
            DividerVariant::Border => scheme.outline_variant,
            DividerVariant::Faint => scheme.surface_container_high,
        };
        row_with(
            Modifier::new().fill_max_width().padding_xy(self.inset, 0.0),
            || {
                spacer(
                    Modifier::new()
                        .fill_max_width()
                        .height(1.0)
                        .background(colour),
                )
            },
        );
    }
}

impl Default for Divider {
    /// [`Divider::new`].
    fn default() -> Self {
        Self::new()
    }
}
