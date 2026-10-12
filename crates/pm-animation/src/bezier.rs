//! The cubic Bézier timing function.

/// A timing curve through (0, 0) and (1, 1) shaped by two control points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    /// The x coordinate of the first control point.
    pub x1: f32,
    /// The y coordinate of the first control point.
    pub y1: f32,
    /// The x coordinate of the second control point.
    pub x2: f32,
    /// The y coordinate of the second control point.
    pub y2: f32,
}

impl CubicBezier {
    /// Constant speed.
    pub const LINEAR: Self = Self::new(0.0, 0.0, 1.0, 1.0);

    /// A curve from its two control points.
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// The eased progress, from 0.0 to 1.0, after `time` of the duration has
    /// passed, also from 0.0 to 1.0.
    pub fn progress(self, time: f32) -> f32 {
        let time = time.clamp(0.0, 1.0);
        let mut parameter = time;
        for _ in 0..8 {
            let error = Self::coordinate(self.x1, self.x2, parameter) - time;
            if error.abs() < 1e-6 {
                return Self::coordinate(self.y1, self.y2, parameter);
            }
            let slope = Self::slope(self.x1, self.x2, parameter);
            if slope.abs() < 1e-6 {
                break;
            }
            parameter -= error / slope;
        }
        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        parameter = time;
        for _ in 0..32 {
            let error = Self::coordinate(self.x1, self.x2, parameter) - time;
            if error.abs() < 1e-6 {
                break;
            }
            if error > 0.0 {
                high = parameter;
            } else {
                low = parameter;
            }
            parameter = (low + high) / 2.0;
        }
        Self::coordinate(self.y1, self.y2, parameter)
    }

    /// One coordinate of the curve at `parameter`, given that axis' two control values.
    fn coordinate(first: f32, second: f32, parameter: f32) -> f32 {
        let inverse = 1.0 - parameter;
        3.0 * inverse * inverse * parameter * first
            + 3.0 * inverse * parameter * parameter * second
            + parameter * parameter * parameter
    }

    /// The derivative of [`Self::coordinate`] with respect to `parameter`.
    fn slope(first: f32, second: f32, parameter: f32) -> f32 {
        let inverse = 1.0 - parameter;
        3.0 * inverse * inverse * first
            + 6.0 * inverse * parameter * (second - first)
            + 3.0 * parameter * parameter * (1.0 - second)
    }
}

impl Default for CubicBezier {
    /// [`CubicBezier::LINEAR`].
    fn default() -> Self {
        Self::LINEAR
    }
}
