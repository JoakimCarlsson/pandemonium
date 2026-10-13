//! The font stacks text is set in.

/// Families tried in order until one is installed.
pub type Stack = &'static [&'static str];

/// The font stacks of a theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fonts {
    /// The interface typeface.
    pub sans: Stack,
    /// The fixed-width typeface.
    pub mono: Stack,
}

impl Fonts {
    /// The stacks every theme starts from.
    pub const DEFAULT: Self = Self {
        sans: &["Inter", "Inter Display", "Noto Sans", "DejaVu Sans"],
        mono: &["JetBrains Mono", "DejaVu Sans Mono"],
    };
}

impl Default for Fonts {
    /// [`Fonts::DEFAULT`].
    fn default() -> Self {
        Self::DEFAULT
    }
}
