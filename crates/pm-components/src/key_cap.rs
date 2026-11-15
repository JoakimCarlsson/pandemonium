//! The keys of a binding, each drawn as a key cap.

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a key cap is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyCapVariant {
    /// A raised surface with a hairline border.
    #[default]
    Raised,
    /// A dashed look for a binding being recorded, in the accent.
    Recording,
}

/// A key cap with a variant and a size.
#[derive(Clone)]
pub struct KeyCap {
    /// The keys, as they are shown, such as `Ctrl+S`.
    keys: String,
    /// How it is drawn.
    variant: KeyCapVariant,
    /// Its size.
    size: Size,
}

impl KeyCap {
    /// A cap reading `keys`.
    pub fn new(keys: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            variant: KeyCapVariant::Raised,
            size: Size::Xs,
        }
    }

    /// The cap in `variant`.
    pub fn variant(mut self, variant: KeyCapVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The cap at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the cap.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let outline = match self.variant {
            KeyCapVariant::Raised => scheme.outline_variant,
            KeyCapVariant::Recording => scheme.primary,
        };
        let height = match self.size {
            Size::Xs => 20.0,
            Size::Sm => 22.0,
            Size::Md => 26.0,
            Size::Lg => 30.0,
        };
        let height = crate::interface_font().sized(height);
        let keys = self.keys;
        stack_aligned(
            Modifier::new()
                .height(height)
                .background_rounded(scheme.surface_container, 4.0)
                .border(1.0, outline, 4.0)
                .padding_xy(6.0, 0.0),
            BoxAlignment::Center,
            move || {
                Label::new(keys)
                    .variant(LabelVariant::Code)
                    .size(Size::Xs)
                    .show();
            },
        );
    }
}
