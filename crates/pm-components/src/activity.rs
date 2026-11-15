//! The chips above the prompt naming what runs beside the agent's turn:
//! commands left running and subagents.

use rustle::prelude::*;

use crate::chip::{Chip, ChipVariant};
use crate::size::Size;

/// The visual treatment of the activity chips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActivityVariant {
    /// Outlined chips, as above the prompt.
    #[default]
    Outline,
    /// Filled chips.
    Filled,
}

/// A row of chips, one per thing running beside the turn.
#[derive(Clone, Debug, Default)]
pub struct Activity {
    /// Each chip's glyph and label.
    items: Vec<(Icon, String)>,
    /// Its visual treatment.
    variant: ActivityVariant,
    /// Its size.
    size: Size,
}

impl Activity {
    /// An empty row of activity chips.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            variant: ActivityVariant::Outline,
            size: Size::Xs,
        }
    }

    /// The row with a chip showing `glyph` and `label`.
    pub fn item(mut self, glyph: Icon, label: impl Into<String>) -> Self {
        self.items.push((glyph, label.into()));
        self
    }

    /// The row with `variant`.
    pub fn variant(mut self, variant: ActivityVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the row, or nothing when nothing runs.
    #[track_caller]
    pub fn show(self) {
        if self.items.is_empty() {
            return;
        }
        let variant = match self.variant {
            ActivityVariant::Outline => ChipVariant::Outline,
            ActivityVariant::Filled => ChipVariant::Filled,
        };
        flow_row(Modifier::new().fill_max_width(), 6.0, 6.0, || {
            for (glyph, label) in &self.items {
                Chip::new(label.clone())
                    .icon(glyph.clone())
                    .variant(variant)
                    .size(self.size)
                    .show();
            }
        });
    }
}
