//! A theme in both appearances.

use crate::Theme;

/// Whether a theme is a light or a dark one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appearance {
    /// Dark surfaces, light text.
    Dark,
    /// Light surfaces, dark text.
    Light,
}

/// A theme in both appearances: what a reader chooses as one thing.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeFamily {
    /// The family's name.
    pub name: &'static str,
    /// The variant for dark surfaces.
    pub dark: Theme,
    /// The variant for light surfaces.
    pub light: Theme,
}

impl ThemeFamily {
    /// The variant of this family for `appearance`.
    pub fn variant(&self, appearance: Appearance) -> &Theme {
        match appearance {
            Appearance::Dark => &self.dark,
            Appearance::Light => &self.light,
        }
    }
}
