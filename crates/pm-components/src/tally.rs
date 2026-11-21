//! The tally: how many lines were added and how many removed.

use crate::theme;
use crate::theme::Role;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, TextOverflow};

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
}

impl Tally {
    /// A tally of `added` lines added and `removed` lines removed, in 12px type.
    pub fn new(added: usize, removed: usize) -> Self {
        Self {
            added,
            removed,
            pixels: 12,
        }
    }

    /// The tally in type `pixels` tall.
    pub fn size(mut self, pixels: u32) -> Self {
        self.pixels = pixels;
        self
    }

    /// Emits `+added −removed`, the first in the success colour and the second in the danger colour.
    #[track_caller]
    pub fn build(self) {
        let (added, removed) = (self.added, self.removed);
        rustle::row_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(theme::sp(1.5)),
            Alignment::Center,
            move || {
                for (words, tone) in [
                    (format!("+{added}"), Role::Success),
                    (format!("−{removed}"), Role::Danger),
                ] {
                    let kind = theme::mono_style(self.pixels, 1.4).color(theme::color(tone));
                    rustle::basic_text(Modifier::new(), words, kind, TextOverflow::Ellipsis);
                }
            },
        );
    }
}
