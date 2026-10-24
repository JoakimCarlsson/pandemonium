//! Background fills and gradients.

use pm_color::Color;
use pm_decoration::{BackgroundRule, GradientDirection};

use crate::Utility;

/// A flat background colour.
pub fn bg(color: impl Into<Color>) -> Utility {
    BackgroundRule::Color(color.into()).into()
}

/// A gradient running in `direction`; set its colours with
/// [`gradient_from`], [`gradient_via`] and [`gradient_to`].
pub fn bg_gradient(direction: GradientDirection) -> Utility {
    BackgroundRule::Gradient(direction).into()
}

/// A gradient running at `degrees` clockwise from pointing up.
pub fn bg_gradient_angle(degrees: f32) -> Utility {
    bg_gradient(GradientDirection::Angle(degrees))
}

/// The gradient's start colour.
pub fn gradient_from(color: impl Into<Color>) -> Utility {
    BackgroundRule::From(color.into()).into()
}

/// The gradient's middle colour.
pub fn gradient_via(color: impl Into<Color>) -> Utility {
    BackgroundRule::Via(color.into()).into()
}

/// The gradient's end colour.
pub fn gradient_to(color: impl Into<Color>) -> Utility {
    BackgroundRule::To(color.into()).into()
}

constants! {
    /// A transparent background.
    bg_transparent = BackgroundRule::Color(Color::Transparent);
    /// No gradient.
    bg_none = BackgroundRule::NoGradient;
    /// A gradient towards the top.
    bg_gradient_to_t = BackgroundRule::Gradient(GradientDirection::Top);
    /// A gradient towards the top-right corner.
    bg_gradient_to_tr = BackgroundRule::Gradient(GradientDirection::TopRight);
    /// A gradient towards the right.
    bg_gradient_to_r = BackgroundRule::Gradient(GradientDirection::Right);
    /// A gradient towards the bottom-right corner.
    bg_gradient_to_br = BackgroundRule::Gradient(GradientDirection::BottomRight);
    /// A gradient towards the bottom.
    bg_gradient_to_b = BackgroundRule::Gradient(GradientDirection::Bottom);
    /// A gradient towards the bottom-left corner.
    bg_gradient_to_bl = BackgroundRule::Gradient(GradientDirection::BottomLeft);
    /// A gradient towards the left.
    bg_gradient_to_l = BackgroundRule::Gradient(GradientDirection::Left);
    /// A gradient towards the top-left corner.
    bg_gradient_to_tl = BackgroundRule::Gradient(GradientDirection::TopLeft);
}
