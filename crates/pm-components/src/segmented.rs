//! A row of a few choices, one of them picked.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::Label;
use crate::size::Size;
use rustle::color_scheme;

/// How a row of choices is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentedVariant {
    /// Choices in a sunken track, the picked one raised.
    #[default]
    Track,
    /// Choices side by side, the picked one filled with the accent.
    Accent,
}

/// A row of choices with a variant and a size.
#[derive(Clone)]
pub struct Segmented {
    /// The choices, as they read.
    options: Vec<String>,
    /// The place of the picked choice.
    picked: usize,
    /// How it is drawn.
    variant: SegmentedVariant,
    /// Its size.
    size: Size,
    /// What picking a choice does, given its place.
    on_pick: Option<Rc<dyn Fn(usize)>>,
}

impl Segmented {
    /// A row of `options` with the one at `picked` picked.
    pub fn new(options: Vec<String>, picked: usize) -> Self {
        Self {
            options,
            picked,
            variant: SegmentedVariant::Track,
            size: Size::Sm,
            on_pick: None,
        }
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: SegmentedVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row calling `on_pick` with the place of the choice pressed.
    pub fn on_pick(mut self, on_pick: impl Fn(usize) + 'static) -> Self {
        self.on_pick = Some(Rc::new(on_pick));
        self
    }

    /// About how wide a row of `options` is at `size`, to choose between it and a list.
    pub fn width(options: &[String], size: Size) -> f32 {
        let (_, padding, letter) = Self::metrics(size);
        options
            .iter()
            .map(|option| option.chars().count() as f32 * letter + padding * 2.0)
            .sum::<f32>()
            + 4.0
    }

    /// The height, the padding of a choice and the width of a letter at `size`.
    fn metrics(size: Size) -> (f32, f32, f32) {
        match size {
            Size::Xs => (22.0, 8.0, 6.5),
            Size::Sm => (26.0, 10.0, 7.2),
            Size::Md => (30.0, 12.0, 7.8),
            Size::Lg => (36.0, 14.0, 8.6),
        }
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let (height, padding, _) = Self::metrics(self.size);
        let track = match self.variant {
            SegmentedVariant::Track => scheme.background,
            SegmentedVariant::Accent => Color::TRANSPARENT,
        };
        row_arranged(
            Modifier::new()
                .height(height)
                .background_rounded(track, 6.0)
                .border(1.0, scheme.outline_variant, 6.0)
                .padding(2.0),
            Arrangement::SpacedBy(2.0),
            Alignment::Center,
            move || {
                for (place, option) in self.options.iter().enumerate() {
                    let picked = place == self.picked;
                    let (fill, tint) = match (self.variant, picked) {
                        (SegmentedVariant::Track, true) => {
                            (scheme.surface_container_highest, scheme.on_surface)
                        }
                        (SegmentedVariant::Accent, true) => (scheme.primary, scheme.on_primary),
                        (_, false) => (Color::TRANSPARENT, scheme.on_surface_variant),
                    };
                    let mut modifier = Modifier::new()
                        .fill_max_height()
                        .background_rounded(fill, 4.0)
                        .padding_xy(padding, 0.0);
                    if let Some(on_pick) = self.on_pick.clone() {
                        modifier = modifier
                            .cursor(CursorIcon::Pointer)
                            .clickable(move || on_pick(place));
                    }
                    stack_aligned(modifier, BoxAlignment::Center, || {
                        Label::new(option.clone()).colour(tint).show();
                    });
                }
            },
        );
    }
}
