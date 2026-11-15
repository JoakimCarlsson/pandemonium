//! A small pill of text, such as a count or a state.

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::Label;
use crate::size::Size;
use rustle::color_scheme;

/// How a badge is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeVariant {
    /// A raised surface in the muted tone.
    #[default]
    Neutral,
    /// The accent.
    Accent,
    /// The success tone.
    Success,
    /// The warning tone.
    Warning,
    /// The danger tone.
    Danger,
}

/// A badge with a variant and a size.
#[derive(Clone)]
pub struct Badge {
    /// Its text.
    label: String,
    /// How it is drawn.
    variant: BadgeVariant,
    /// Its size.
    size: Size,
}

impl Badge {
    /// A neutral badge reading `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: BadgeVariant::Neutral,
            size: Size::Xs,
        }
    }

    /// The badge in `variant`.
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The badge at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the badge.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let tint = match self.variant {
            BadgeVariant::Neutral => scheme.on_surface_variant,
            BadgeVariant::Accent => scheme.primary,
            BadgeVariant::Success => extended.success,
            BadgeVariant::Warning => extended.warning,
            BadgeVariant::Danger => scheme.error,
        };
        let fill = match self.variant {
            BadgeVariant::Neutral => scheme.surface_container_high,
            _ => Color::rgba(tint.r, tint.g, tint.b, 40),
        };
        let (height, padding) = match self.size {
            Size::Xs => (18.0, 6.0),
            Size::Sm => (20.0, 8.0),
            Size::Md => (22.0, 8.0),
            Size::Lg => (26.0, 10.0),
        };
        let label = self.label;
        let size = self.size;
        stack_aligned(
            Modifier::new()
                .height(height)
                .background_rounded(fill, height / 2.0)
                .padding_xy(padding, 0.0),
            BoxAlignment::Center,
            move || {
                Label::new(label)
                    .size(if size == Size::Xs { Size::Xs } else { Size::Sm })
                    .colour(tint)
                    .show();
            },
        );
    }
}
