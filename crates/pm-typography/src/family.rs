//! Which typeface, and whether it leans.

/// A generic family; the renderer maps each to the fonts it has loaded.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum FontFamily {
    /// The interface typeface.
    #[default]
    Sans,
    /// A typeface with serifs.
    Serif,
    /// A fixed-width typeface.
    Mono,
}

/// Whether glyphs lean.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum FontSlant {
    /// Upright.
    #[default]
    Normal,
    /// Leaning.
    Italic,
}
