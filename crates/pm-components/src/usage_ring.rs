//! A small ring that fills as something is used up, such as the agent's
//! context or the account's allowance, with its share beside it and what it
//! measures in a tooltip.

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// The share used above which the ring warns.
const WARN_AT: f32 = 0.8;

/// The share used above which the ring is in danger.
const DANGER_AT: f32 = 0.95;

/// What a usage ring measures, which picks its tone while there is room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UsageRingVariant {
    /// How full the agent's context is, in the accent tone.
    #[default]
    Context,
    /// How much of the account's allowance is used, in the success tone.
    Allowance,
}

/// A usage ring with a variant and a size.
#[derive(Clone)]
pub struct UsageRing {
    /// How much is used, from 0 to 1.
    fraction: f32,
    /// What the ring measures, shown when hovered.
    detail: String,
    /// What it measures.
    variant: UsageRingVariant,
    /// Its size.
    size: Size,
}

impl UsageRing {
    /// A context ring `fraction` full.
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
            detail: String::new(),
            variant: UsageRingVariant::Context,
            size: Size::Sm,
        }
    }

    /// The ring telling `detail` when hovered.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The ring in `variant`.
    pub fn variant(mut self, variant: UsageRingVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The ring at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the ring.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let side = match self.size {
            Size::Xs => 12.0,
            Size::Sm => 14.0,
            Size::Md => 18.0,
            Size::Lg => 22.0,
        };
        let tone = if self.fraction >= DANGER_AT {
            scheme.error
        } else if self.fraction >= WARN_AT {
            extended.warning
        } else {
            match self.variant {
                UsageRingVariant::Context => scheme.primary,
                UsageRingVariant::Allowance => extended.success,
            }
        };
        let track = scheme.outline_variant;
        let fraction = self.fraction;
        let percent = format!("{:.0}%", fraction * 100.0);
        tooltip(&self.detail, move || {
            row_arranged(
                Modifier::new().padding_xy(4.0, 0.0),
                Arrangement::SpacedBy(crate::spacing(4.0)),
                Alignment::Center,
                move || {
                    canvas(Modifier::new().size(side, side), move |canvas| {
                        let middle = side / 2.0;
                        let radius = middle - 1.5;
                        canvas.stroke_circle(middle, middle, radius, 2.0, track);
                        canvas.arc(middle, middle, radius, 2.0, 0.0, fraction, tone);
                    });
                    Label::new(percent)
                        .variant(LabelVariant::Subtle)
                        .size(Size::Xs)
                        .show();
                },
            );
        });
    }
}
