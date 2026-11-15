//! The tally: how many lines were added, in the success tone, and how many
//! removed, in the danger tone.

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a tally is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TallyVariant {
    /// Both counts, even when nothing was added or removed.
    #[default]
    Full,
    /// Only the counts that are not zero.
    Compact,
}

/// A tally of added and removed lines with a variant and a size.
#[derive(Clone, Copy, Debug)]
pub struct Tally {
    /// The lines added.
    added: usize,
    /// The lines removed.
    removed: usize,
    /// How it is drawn.
    variant: TallyVariant,
    /// Its type size.
    size: Size,
}

impl Tally {
    /// A tally of `added` lines added and `removed` lines removed.
    pub fn new(added: usize, removed: usize) -> Self {
        Self {
            added,
            removed,
            variant: TallyVariant::Full,
            size: Size::Xs,
        }
    }

    /// The tally in `variant`.
    pub fn variant(mut self, variant: TallyVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The tally at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits `+added −removed`.
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let compact = self.variant == TallyVariant::Compact;
        row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(4.0),
            Alignment::Center,
            move || {
                if !compact || self.added > 0 {
                    Label::new(format!("+{}", self.added))
                        .variant(LabelVariant::Code)
                        .colour(extended.success)
                        .size(self.size)
                        .show();
                }
                if !compact || self.removed > 0 {
                    Label::new(format!("\u{2212}{}", self.removed))
                        .variant(LabelVariant::Code)
                        .colour(scheme.error)
                        .size(self.size)
                        .show();
                }
            },
        );
    }
}
