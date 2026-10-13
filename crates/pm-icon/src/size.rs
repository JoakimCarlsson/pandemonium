//! The sizes an icon is drawn at.

/// The sizes an icon is drawn at; the artwork is designed on a sixteen-pixel grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconSize {
    /// 12px: inside a tab or a row of text.
    ///
    /// The artwork is drawn on a grid of sixteen, so anything below
    /// [`IconSize::Medium`] rasterizes its strokes across two pixels rather
    /// than into one: a size for an icon that has to fit, not one to reach
    /// for because the icon is small.
    XSmall,
    /// 14px: a smaller icon, where one at its own size would crowd its row.
    #[default]
    Small,
    /// 16px: the artwork's own size, which is the size it is sharpest at.
    Medium,
}

impl IconSize {
    /// The side of the square this size draws inside.
    pub const fn pixels(self) -> f32 {
        match self {
            Self::XSmall => 12.0,
            Self::Small => 14.0,
            Self::Medium => 16.0,
        }
    }
}
