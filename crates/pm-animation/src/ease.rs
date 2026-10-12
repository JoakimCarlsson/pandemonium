//! Named easing curves.

use crate::CubicBezier;

/// A named easing curve.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Ease {
    /// A cubic Bézier curve.
    In,
    /// A cubic Bézier curve.
    Out,
    /// A cubic Bézier curve.
    InOut,
}

impl Ease {
    /// The curve's control points.
    pub const fn curve(self) -> CubicBezier {
        match self {
            Self::In => CubicBezier::new(0.4, 0.0, 1.0, 1.0),
            Self::Out => CubicBezier::new(0.0, 0.0, 0.2, 1.0),
            Self::InOut => CubicBezier::new(0.4, 0.0, 0.2, 1.0),
        }
    }
}

impl Ease {
    /// Every named curve.
    pub const ALL: [Self; 3] = [Self::In, Self::Out, Self::InOut];
}
