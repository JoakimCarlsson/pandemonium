//! How far something extends along one axis.

use crate::Number;

/// A size along one axis, before the layout pass has an offer to resolve it
/// against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Whatever the layout decides: content-sized, or stretched by alignment.
    Auto,
    /// An exact number of logical pixels.
    Px(f32),
    /// A fraction of the parent's extent, from 0.0 to 1.0.
    Fraction(f32),
    /// The whole viewport along this axis.
    Screen,
    /// The narrowest the content can be without overflowing.
    MinContent,
    /// The widest the content wants to be.
    MaxContent,
    /// As large as the content, but no larger than the offer.
    FitContent,
}

impl Length {
    /// All of the parent's extent.
    pub const FULL: Self = Self::Fraction(1.0);

    /// `count` steps of the spacing scale.
    pub fn steps(count: impl Number) -> Self {
        Self::Px(count.step_pixels())
    }

    /// The fraction `numerator` over `denominator` of the parent's extent.
    pub fn ratio(numerator: f32, denominator: f32) -> Self {
        Self::Fraction(numerator / denominator)
    }

    /// The pixels this length comes to when the parent offers `available`
    /// and the viewport is `viewport` along the same axis; `None` for the
    /// lengths the content decides.
    pub fn resolve(self, available: f32, viewport: f32) -> Option<f32> {
        match self {
            Self::Px(pixels) => Some(pixels),
            Self::Fraction(fraction) => Some(available * fraction),
            Self::Screen => Some(viewport),
            Self::Auto | Self::MinContent | Self::MaxContent | Self::FitContent => None,
        }
    }
}

impl Default for Length {
    /// [`Length::Auto`].
    fn default() -> Self {
        Self::Auto
    }
}
