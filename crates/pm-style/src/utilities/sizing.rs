//! Width, height, bounds and aspect ratio.

use pm_layout::{Container, SizingRule};
use pm_primitives::{Length, Number};

use crate::Utility;

/// Width, in steps.
pub fn w(steps: impl Number) -> Utility {
    SizingRule::Width(Length::steps(steps)).into()
}

/// Height, in steps.
pub fn h(steps: impl Number) -> Utility {
    SizingRule::Height(Length::steps(steps)).into()
}

/// Width and height, in steps.
pub fn size(steps: impl Number) -> Utility {
    SizingRule::Size(Length::steps(steps)).into()
}

/// Width, in raw logical pixels.
pub fn w_px(pixels: impl Number) -> Utility {
    SizingRule::Width(Length::Px(pixels.value())).into()
}

/// Height, in raw logical pixels.
pub fn h_px(pixels: impl Number) -> Utility {
    SizingRule::Height(Length::Px(pixels.value())).into()
}

/// Width as a fraction of the parent, such as `w_frac(1, 3)`.
pub fn w_frac(numerator: impl Number, denominator: impl Number) -> Utility {
    SizingRule::Width(Length::ratio(numerator.value(), denominator.value())).into()
}

/// Height as a fraction of the parent, such as `h_frac(1, 2)`.
pub fn h_frac(numerator: impl Number, denominator: impl Number) -> Utility {
    SizingRule::Height(Length::ratio(numerator.value(), denominator.value())).into()
}

/// Width and height, in raw logical pixels.
pub fn size_px(pixels: impl Number) -> Utility {
    SizingRule::Size(Length::Px(pixels.value())).into()
}

/// Lower bound on width, in raw logical pixels.
pub fn min_w_px(pixels: impl Number) -> Utility {
    SizingRule::MinWidth(Length::Px(pixels.value())).into()
}

/// Upper bound on width, in raw logical pixels.
pub fn max_w_px(pixels: impl Number) -> Utility {
    SizingRule::MaxWidth(Length::Px(pixels.value())).into()
}

/// Upper bound on height, in raw logical pixels.
pub fn max_h_px(pixels: impl Number) -> Utility {
    SizingRule::MaxHeight(Length::Px(pixels.value())).into()
}

/// Lower bound on width, in steps.
pub fn min_w(steps: impl Number) -> Utility {
    SizingRule::MinWidth(Length::steps(steps)).into()
}

/// Lower bound on height, in steps.
pub fn min_h(steps: impl Number) -> Utility {
    SizingRule::MinHeight(Length::steps(steps)).into()
}

/// Upper bound on width, in steps.
pub fn max_w(steps: impl Number) -> Utility {
    SizingRule::MaxWidth(Length::steps(steps)).into()
}

/// Upper bound on height, in steps.
pub fn max_h(steps: impl Number) -> Utility {
    SizingRule::MaxHeight(Length::steps(steps)).into()
}

/// Upper bound on width from a named container size.
pub fn max_w_container(container: Container) -> Utility {
    SizingRule::MaxWidth(Length::Px(container.width())).into()
}

/// Width from a named container size.
pub fn w_container(container: Container) -> Utility {
    SizingRule::Width(Length::Px(container.width())).into()
}

/// Width divided by height, such as `aspect(16, 9)`.
pub fn aspect(width: impl Number, height: impl Number) -> Utility {
    SizingRule::AspectRatio(width.value() / height.value()).into()
}

constants! {
    /// Width of the whole parent.
    w_full = SizingRule::Width(Length::FULL);
    /// Height of the whole parent.
    h_full = SizingRule::Height(Length::FULL);
    /// Width and height of the whole parent.
    size_full = SizingRule::Size(Length::FULL);
    /// Width decided by layout.
    w_auto = SizingRule::Width(Length::Auto);
    /// Height decided by layout.
    h_auto = SizingRule::Height(Length::Auto);
    /// Width of the viewport.
    w_screen = SizingRule::Width(Length::Screen);
    /// Height of the viewport.
    h_screen = SizingRule::Height(Length::Screen);
    /// Narrowest width the content allows.
    w_min = SizingRule::Width(Length::MinContent);
    /// Widest width the content wants.
    w_max = SizingRule::Width(Length::MaxContent);
    /// Content width, capped at the offer.
    w_fit = SizingRule::Width(Length::FitContent);
    /// Shortest height the content allows.
    h_min = SizingRule::Height(Length::MinContent);
    /// Tallest height the content wants.
    h_max = SizingRule::Height(Length::MaxContent);
    /// Content height, capped at the offer.
    h_fit = SizingRule::Height(Length::FitContent);
    /// No lower bound on width beyond the parent's whole width.
    min_w_full = SizingRule::MinWidth(Length::FULL);
    /// No lower bound on height beyond the parent's whole height.
    min_h_full = SizingRule::MinHeight(Length::FULL);
    /// Upper bound on width at the parent's width.
    max_w_full = SizingRule::MaxWidth(Length::FULL);
    /// Upper bound on height at the parent's height.
    max_h_full = SizingRule::MaxHeight(Length::FULL);
    /// Upper bound on width at the viewport's width.
    max_w_screen = SizingRule::MaxWidth(Length::Screen);
    /// Lower bound on height at the viewport's height.
    min_h_screen = SizingRule::MinHeight(Length::Screen);
    /// Width equal to height.
    aspect_square = SizingRule::AspectRatio(1.0);
    /// Sixteen to nine.
    aspect_video = SizingRule::AspectRatio(16.0 / 9.0);
    /// No ratio.
    aspect_auto = SizingRule::AspectAuto;
}
