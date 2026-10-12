//! The animation rule: one declaration about motion.

use pm_primitives::{Length, Rule};

use crate::{Animation, AnimationStyle, CubicBezier, Origin, TransitionProperty};

/// One declaration about animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationRule {
    /// The properties that ease.
    TransitionProperty(TransitionProperty),
    /// The easing duration in milliseconds.
    Duration(f32),
    /// The easing curve.
    Ease(CubicBezier),
    /// The easing delay in milliseconds.
    Delay(f32),
    /// The shift along x.
    TranslateX(Length),
    /// The shift along y.
    TranslateY(Length),
    /// The scale on both axes.
    Scale(f32),
    /// The scale along x.
    ScaleX(f32),
    /// The scale along y.
    ScaleY(f32),
    /// The rotation in degrees.
    Rotate(f32),
    /// The skew along x in degrees.
    SkewX(f32),
    /// The skew along y in degrees.
    SkewY(f32),
    /// The pivot of the transform.
    Origin(Origin),
    /// A running animation, or none.
    Animation(Option<Animation>),
}

impl Rule for AnimationRule {
    type Target = AnimationStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut AnimationStyle) {
        match self {
            Self::TransitionProperty(property) => target.transition.property = property,
            Self::Duration(milliseconds) => target.transition.duration_ms = milliseconds,
            Self::Ease(curve) => target.transition.ease = curve,
            Self::Delay(milliseconds) => target.transition.delay_ms = milliseconds,
            Self::TranslateX(length) => target.transform.translate.0 = length,
            Self::TranslateY(length) => target.transform.translate.1 = length,
            Self::Scale(factor) => target.transform.scale = (factor, factor),
            Self::ScaleX(factor) => target.transform.scale.0 = factor,
            Self::ScaleY(factor) => target.transform.scale.1 = factor,
            Self::Rotate(degrees) => target.transform.rotate = degrees,
            Self::SkewX(degrees) => target.transform.skew.0 = degrees,
            Self::SkewY(degrees) => target.transform.skew.1 = degrees,
            Self::Origin(origin) => target.transform.origin = origin,
            Self::Animation(animation) => target.animation = animation,
        }
    }
}
