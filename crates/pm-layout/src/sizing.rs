//! Width, height, their bounds and the aspect ratio.

use pm_primitives::{Length, Rule};

/// How large a box is asked to be.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizingStyle {
    /// The width; [`Length::Auto`] lets layout decide.
    pub width: Length,
    /// The height; [`Length::Auto`] lets layout decide.
    pub height: Length,
    /// The narrowest the box may be; [`Length::Auto`] sets no bound.
    pub min_width: Length,
    /// The shortest the box may be; [`Length::Auto`] sets no bound.
    pub min_height: Length,
    /// The widest the box may be; [`Length::Auto`] sets no bound.
    pub max_width: Length,
    /// The tallest the box may be; [`Length::Auto`] sets no bound.
    pub max_height: Length,
    /// Width divided by height, when one of the two is derived from the other.
    pub aspect_ratio: Option<f32>,
}

/// One declaration about size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizingRule {
    /// Sets the width; [`Length::Auto`] lets layout decide.
    Width(Length),
    /// Sets the height; [`Length::Auto`] lets layout decide.
    Height(Length),
    /// Width and height together.
    Size(Length),
    /// The lower bound on width.
    MinWidth(Length),
    /// The lower bound on height.
    MinHeight(Length),
    /// The upper bound on width.
    MaxWidth(Length),
    /// The upper bound on height.
    MaxHeight(Length),
    /// Width divided by height.
    AspectRatio(f32),
    /// Derives nothing from a ratio.
    AspectAuto,
}

impl Rule for SizingRule {
    type Target = SizingStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut SizingStyle) {
        match self {
            Self::Width(length) => target.width = length,
            Self::Height(length) => target.height = length,
            Self::Size(length) => {
                target.width = length;
                target.height = length;
            }
            Self::MinWidth(length) => target.min_width = length,
            Self::MinHeight(length) => target.min_height = length,
            Self::MaxWidth(length) => target.max_width = length,
            Self::MaxHeight(length) => target.max_height = length,
            Self::AspectRatio(ratio) => target.aspect_ratio = Some(ratio),
            Self::AspectAuto => target.aspect_ratio = None,
        }
    }
}
