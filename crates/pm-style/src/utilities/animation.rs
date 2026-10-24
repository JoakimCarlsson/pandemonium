//! Transitions, transforms and named animations.

use pm_animation::{Animation, AnimationRule, CubicBezier, Ease, Origin, TransitionProperty};
use pm_primitives::{Length, Number};

use crate::Utility;

/// Transition duration, in milliseconds.
pub fn duration(milliseconds: impl Number) -> Utility {
    AnimationRule::Duration(milliseconds.value()).into()
}

/// Transition delay, in milliseconds.
pub fn delay(milliseconds: impl Number) -> Utility {
    AnimationRule::Delay(milliseconds.value()).into()
}

/// A named easing curve.
pub fn ease(curve: Ease) -> Utility {
    AnimationRule::Ease(curve.curve()).into()
}

/// A custom easing curve from two control points.
pub fn ease_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Utility {
    AnimationRule::Ease(CubicBezier::new(x1, y1, x2, y2)).into()
}

/// Shift along x, in steps; negative shifts left.
pub fn translate_x(steps: impl Number) -> Utility {
    AnimationRule::TranslateX(Length::steps(steps)).into()
}

/// Shift along y, in steps; negative shifts up.
pub fn translate_y(steps: impl Number) -> Utility {
    AnimationRule::TranslateY(Length::steps(steps)).into()
}

/// Shift along x by a fraction of the element's width.
pub fn translate_x_frac(numerator: impl Number, denominator: impl Number) -> Utility {
    AnimationRule::TranslateX(Length::ratio(numerator.value(), denominator.value())).into()
}

/// Shift along y by a fraction of the element's height.
pub fn translate_y_frac(numerator: impl Number, denominator: impl Number) -> Utility {
    AnimationRule::TranslateY(Length::ratio(numerator.value(), denominator.value())).into()
}

/// Scale on both axes, as a percentage; 100 leaves it unchanged.
pub fn scale(percent: impl Number) -> Utility {
    AnimationRule::Scale(percent.value() / 100.0).into()
}

/// Scale along x, as a percentage.
pub fn scale_x(percent: impl Number) -> Utility {
    AnimationRule::ScaleX(percent.value() / 100.0).into()
}

/// Scale along y, as a percentage.
pub fn scale_y(percent: impl Number) -> Utility {
    AnimationRule::ScaleY(percent.value() / 100.0).into()
}

/// Rotation in degrees, clockwise.
pub fn rotate(degrees: impl Number) -> Utility {
    AnimationRule::Rotate(degrees.value()).into()
}

/// Skew along x, in degrees.
pub fn skew_x(degrees: impl Number) -> Utility {
    AnimationRule::SkewX(degrees.value()).into()
}

/// Skew along y, in degrees.
pub fn skew_y(degrees: impl Number) -> Utility {
    AnimationRule::SkewY(degrees.value()).into()
}

/// The pivot of the transform, as fractions of the element's size.
pub fn origin(x: f32, y: f32) -> Utility {
    AnimationRule::Origin(Origin { x, y }).into()
}

constants! {
    /// Eases colours, opacity, shadow and transform.
    transition = AnimationRule::TransitionProperty(TransitionProperty::Common);
    /// Eases every property.
    transition_all = AnimationRule::TransitionProperty(TransitionProperty::All);
    /// Eases colours.
    transition_colors = AnimationRule::TransitionProperty(TransitionProperty::Colors);
    /// Eases opacity.
    transition_opacity = AnimationRule::TransitionProperty(TransitionProperty::Opacity);
    /// Eases shadow and ring.
    transition_shadow = AnimationRule::TransitionProperty(TransitionProperty::Shadow);
    /// Eases the transform.
    transition_transform = AnimationRule::TransitionProperty(TransitionProperty::Transform);
    /// Eases nothing.
    transition_none = AnimationRule::TransitionProperty(TransitionProperty::None);
    /// Starts slowly.
    ease_in = AnimationRule::Ease(Ease::In.curve());
    /// Ends slowly.
    ease_out = AnimationRule::Ease(Ease::Out.curve());
    /// Starts and ends slowly.
    ease_in_out = AnimationRule::Ease(Ease::InOut.curve());
    /// Constant speed.
    ease_linear = AnimationRule::Ease(CubicBezier::LINEAR);
    /// Pivot at the middle.
    origin_center = AnimationRule::Origin(Origin { x: 0.5, y: 0.5 });
    /// Pivot at the top-left corner.
    origin_top_left = AnimationRule::Origin(Origin { x: 0.0, y: 0.0 });
    /// Pivot at the top-right corner.
    origin_top_right = AnimationRule::Origin(Origin { x: 1.0, y: 0.0 });
    /// Pivot at the bottom-left corner.
    origin_bottom_left = AnimationRule::Origin(Origin { x: 0.0, y: 1.0 });
    /// Pivot at the bottom-right corner.
    origin_bottom_right = AnimationRule::Origin(Origin { x: 1.0, y: 1.0 });
    /// Turns forever.
    animate_spin = AnimationRule::Animation(Some(Animation::SPIN));
    /// Ripples outward forever.
    animate_ping = AnimationRule::Animation(Some(Animation::PING));
    /// Fades in and out forever.
    animate_pulse = AnimationRule::Animation(Some(Animation::PULSE));
    /// Bounces forever.
    animate_bounce = AnimationRule::Animation(Some(Animation::BOUNCE));
    /// No animation.
    animate_none = AnimationRule::Animation(None);
}
