//! The colours the reader has repainted over whichever theme is chosen.
//!
//! An override belongs to an appearance, not to a theme: repainting the dark
//! background repaints it in every family's dark variant, so a reader can
//! try a colour against each family without writing it down per family.
//! Saving them as a theme of their own is how they are kept to one family.

use std::collections::BTreeMap;

use pm_gfx::Rgba;
use pm_ui::{Appearance, Theme};

use crate::config::tokens::TOKENS;

/// The repainted colours, by index into [`TOKENS`], one set per appearance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeOverrides {
    /// What is repainted over a dark variant.
    dark: BTreeMap<usize, Rgba>,
    /// What is repainted over a light variant.
    light: BTreeMap<usize, Rgba>,
}

impl ThemeOverrides {
    /// Overrides made of the `dark` set and the `light` one.
    pub fn of(dark: BTreeMap<usize, Rgba>, light: BTreeMap<usize, Rgba>) -> Self {
        Self { dark, light }
    }

    /// What is repainted over the `appearance` variant.
    pub fn colors(&self, appearance: Appearance) -> &BTreeMap<usize, Rgba> {
        match appearance {
            Appearance::Dark => &self.dark,
            Appearance::Light => &self.light,
        }
    }

    /// What is repainted over the `appearance` variant, to change.
    fn colors_mut(&mut self, appearance: Appearance) -> &mut BTreeMap<usize, Rgba> {
        match appearance {
            Appearance::Dark => &mut self.dark,
            Appearance::Light => &mut self.light,
        }
    }

    /// Repaints `token` in `color` over the `appearance` variant.
    pub fn set(&mut self, appearance: Appearance, token: usize, color: Rgba) {
        if token < TOKENS.len() {
            self.colors_mut(appearance).insert(token, color);
        }
    }

    /// Leaves `token` to the theme again over the `appearance` variant.
    pub fn clear(&mut self, appearance: Appearance, token: usize) {
        self.colors_mut(appearance).remove(&token);
    }

    /// Leaves every colour to the theme again over the `appearance` variant.
    pub fn clear_all(&mut self, appearance: Appearance) {
        self.colors_mut(appearance).clear();
    }

    /// `theme` with the colours repainted over its appearance.
    pub fn apply(&self, mut theme: Theme) -> Theme {
        for (token, color) in self.colors(theme.appearance) {
            TOKENS[*token].write(&mut theme, *color);
        }
        theme
    }
}
