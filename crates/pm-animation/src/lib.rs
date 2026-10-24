//! Animation styling: easing, transitions, transforms and keyframes.
//!
//! The crate holds what an element declares (a transition, a transform, a
//! named animation) and the maths that turns elapsed time into a value. It
//! owns no clock: the caller says how much time has passed.

mod bezier;
mod ease;
mod keyframes;
mod rule;
mod style;
mod transform;
mod transition;

pub use bezier::CubicBezier;
pub use ease::Ease;
pub use keyframes::{Animation, Iterations, Keyframe, Pose};
pub use rule::AnimationRule;
pub use style::AnimationStyle;
pub use transform::{Origin, Transform};
pub use transition::{Transition, TransitionProperty};
