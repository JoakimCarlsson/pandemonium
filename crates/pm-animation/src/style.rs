//! The animation slice of a style.

use crate::{Animation, Transform, Transition};

/// Everything the animation driver reads about one element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnimationStyle {
    /// How changes of style are eased.
    pub transition: Transition,
    /// The transform applied after layout.
    pub transform: Transform,
    /// The running animation, if any.
    pub animation: Option<Animation>,
}
