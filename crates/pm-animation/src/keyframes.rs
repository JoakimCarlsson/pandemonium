//! Keyframed animations and the four named ones.

use pm_primitives::Length;

use crate::{CubicBezier, Origin, Transform};

/// What an animation changes at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// The transform.
    pub transform: Transform,
    /// The opacity, 0.0 to 1.0.
    pub opacity: f32,
}

impl Pose {
    /// Untransformed and opaque.
    pub const REST: Self = Self {
        transform: Transform::IDENTITY,
        opacity: 1.0,
    };

    /// The pose `amount` of the way from `self` to `other`.
    pub fn lerp(self, other: Self, amount: f32) -> Self {
        Self {
            transform: self.transform.lerp(other.transform, amount),
            opacity: self.opacity + (other.opacity - self.opacity) * amount,
        }
    }
}

/// A pose at a point of the animation, and the curve towards the next one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    /// Where in the animation, 0.0 start to 1.0 end.
    pub offset: f32,
    /// The pose at that point.
    pub pose: Pose,
    /// The curve used from this keyframe to the next; the animation's own when unset.
    pub ease: Option<CubicBezier>,
}

/// How many times an animation runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Iterations {
    /// A fixed number of times.
    Count(f32),
    /// Until it is removed.
    Infinite,
}

/// A named animation: keyframes, how long they take and how often they run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    /// The keyframes, ordered by offset, from 0.0 to 1.0.
    pub frames: &'static [Keyframe],
    /// The length of one run, in milliseconds.
    pub duration_ms: f32,
    /// The curve between keyframes that name none.
    pub ease: CubicBezier,
    /// How often it runs.
    pub iterations: Iterations,
}

/// A keyframe at `offset` holding `pose`, easing towards the next with `ease`.
const fn frame(offset: f32, pose: Pose, ease: Option<CubicBezier>) -> Keyframe {
    Keyframe { offset, pose, ease }
}

/// The pose shifted down by `fraction` of the element's height.
const fn translated_y(fraction: f32) -> Pose {
    Pose {
        transform: Transform {
            translate: (Length::Px(0.0), Length::Fraction(fraction)),
            ..Transform::IDENTITY
        },
        opacity: 1.0,
    }
}

impl Animation {
    /// A full clockwise turn every second, forever.
    pub const SPIN: Self = Self {
        frames: &[
            frame(0.0, Pose::REST, None),
            frame(
                1.0,
                Pose {
                    transform: Transform {
                        rotate: 360.0,
                        origin: Origin::CENTER,
                        ..Transform::IDENTITY
                    },
                    opacity: 1.0,
                },
                None,
            ),
        ],
        duration_ms: 1000.0,
        ease: CubicBezier::LINEAR,
        iterations: Iterations::Infinite,
    };

    /// A ripple that grows to twice the size while fading out, forever.
    pub const PING: Self = Self {
        frames: &[
            frame(0.0, Pose::REST, None),
            frame(
                0.75,
                Pose {
                    transform: Transform {
                        scale: (2.0, 2.0),
                        ..Transform::IDENTITY
                    },
                    opacity: 0.0,
                },
                None,
            ),
            frame(
                1.0,
                Pose {
                    transform: Transform {
                        scale: (2.0, 2.0),
                        ..Transform::IDENTITY
                    },
                    opacity: 0.0,
                },
                None,
            ),
        ],
        duration_ms: 1000.0,
        ease: CubicBezier::new(0.0, 0.0, 0.2, 1.0),
        iterations: Iterations::Infinite,
    };

    /// A slow fade to half opacity and back, forever.
    pub const PULSE: Self = Self {
        frames: &[
            frame(0.0, Pose::REST, None),
            frame(
                0.5,
                Pose {
                    transform: Transform::IDENTITY,
                    opacity: 0.5,
                },
                None,
            ),
            frame(1.0, Pose::REST, None),
        ],
        duration_ms: 2000.0,
        ease: CubicBezier::new(0.4, 0.0, 0.6, 1.0),
        iterations: Iterations::Infinite,
    };

    /// A bounce a quarter of the element's height, forever.
    pub const BOUNCE: Self = Self {
        frames: &[
            frame(
                0.0,
                translated_y(-0.25),
                Some(CubicBezier::new(0.8, 0.0, 1.0, 1.0)),
            ),
            frame(0.5, Pose::REST, Some(CubicBezier::new(0.0, 0.0, 0.2, 1.0))),
            frame(1.0, translated_y(-0.25), None),
        ],
        duration_ms: 1000.0,
        ease: CubicBezier::LINEAR,
        iterations: Iterations::Infinite,
    };

    /// The pose `elapsed_ms` after the animation started; `None` once every
    /// iteration has finished.
    pub fn sample(self, elapsed_ms: f32) -> Option<Pose> {
        if self.duration_ms <= 0.0 || self.frames.is_empty() {
            return None;
        }
        let runs = elapsed_ms.max(0.0) / self.duration_ms;
        let position = match self.iterations {
            Iterations::Infinite => runs.fract(),
            Iterations::Count(count) if runs >= count => return None,
            Iterations::Count(_) => runs.fract(),
        };
        let next = self
            .frames
            .iter()
            .position(|keyframe| keyframe.offset > position)
            .unwrap_or(self.frames.len() - 1);
        let previous = next.saturating_sub(1);
        let (from, to) = (self.frames[previous], self.frames[next]);
        let span = to.offset - from.offset;
        if span <= 0.0 {
            return Some(to.pose);
        }
        let curve = from.ease.unwrap_or(self.ease);
        let amount = curve.progress((position - from.offset) / span);
        Some(from.pose.lerp(to.pose, amount))
    }
}
