//! The tally: how many lines were added and how many removed.

use crate::select::styled_text;
use crate::theme;
use crate::theme::Role;
use crate::theme::sp;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};

/// A tally being described: the two counts and how big they are set.
///
/// Build it with [`Tally::new`], change it with [`Tally::size`], and emit it
/// with [`Tally::build`].
#[derive(Clone, Debug)]
pub struct Tally {
    /// The lines added.
    added: usize,
    /// The lines removed.
    removed: usize,
    /// How tall the type is, in logical pixels.
    pixels: u32,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Tally {
    /// A tally of `added` lines added and `removed` lines removed, in 12px type.
    pub fn new(added: usize, removed: usize) -> Self {
        Self {
            added,
            removed,
            pixels: 12,
            modifier: Modifier::new(),
        }
    }

    /// The tally in type `pixels` tall.
    pub fn size(mut self, pixels: u32) -> Self {
        self.pixels = pixels;
        self
    }

    /// The tally with `modifier` put around its box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits `+added −removed`, the first in the success colour and the second in the danger colour.
    #[track_caller]
    pub fn build(self) {
        let kind = theme::mono_style(self.pixels, 1.4);
        let (added, removed) = (self.added, self.removed);
        row_arranged(
            self.modifier,
            Arrangement::SpacedBy(sp(1.5)),
            Alignment::Center,
            move || {
                styled_text(
                    Modifier::new(),
                    format!("+{added}"),
                    kind.color(theme::color(Role::Success)),
                );
                styled_text(
                    Modifier::new(),
                    format!("−{removed}"),
                    kind.color(theme::color(Role::Danger)),
                );
            },
        );
    }
}
