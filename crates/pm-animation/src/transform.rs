//! Two-dimensional transforms: translate, scale, rotate and skew.

use pm_primitives::Length;

/// The point a transform pivots around, as fractions of the element's size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Origin {
    /// Horizontal position, 0.0 left to 1.0 right.
    pub x: f32,
    /// Vertical position, 0.0 top to 1.0 bottom.
    pub y: f32,
}

impl Origin {
    /// The middle of the element.
    pub const CENTER: Self = Self { x: 0.5, y: 0.5 };
}

impl Default for Origin {
    /// [`Origin::CENTER`].
    fn default() -> Self {
        Self::CENTER
    }
}

/// A two-dimensional transform applied after layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// The shift along x and y; fractions are of the element's own size.
    pub translate: (Length, Length),
    /// The scale along x and y.
    pub scale: (f32, f32),
    /// The rotation in degrees, clockwise.
    pub rotate: f32,
    /// The skew along x and y in degrees.
    pub skew: (f32, f32),
    /// The pivot.
    pub origin: Origin,
}

impl Transform {
    /// Leaves the element where layout put it.
    pub const IDENTITY: Self = Self {
        translate: (Length::Px(0.0), Length::Px(0.0)),
        scale: (1.0, 1.0),
        rotate: 0.0,
        skew: (0.0, 0.0),
        origin: Origin::CENTER,
    };

    /// The transform as the affine matrix `[a, b, c, d, tx, ty]` for an
    /// element of `width` by `height`, pivoting about the origin.
    pub fn matrix(self, width: f32, height: f32) -> [f32; 6] {
        let shift_x = self.translate.0.resolve(width, 0.0).unwrap_or(0.0);
        let shift_y = self.translate.1.resolve(height, 0.0).unwrap_or(0.0);
        let (sine, cosine) = self.rotate.to_radians().sin_cos();
        let skew_x = self.skew.0.to_radians().tan();
        let skew_y = self.skew.1.to_radians().tan();
        let a = cosine * self.scale.0 + (-sine) * skew_y * self.scale.0;
        let b = sine * self.scale.0 + cosine * skew_y * self.scale.0;
        let c = (cosine * skew_x - sine) * self.scale.1;
        let d = (sine * skew_x + cosine) * self.scale.1;
        let pivot_x = self.origin.x * width;
        let pivot_y = self.origin.y * height;
        [
            a,
            b,
            c,
            d,
            shift_x + pivot_x - (a * pivot_x + c * pivot_y),
            shift_y + pivot_y - (b * pivot_x + d * pivot_y),
        ]
    }

    /// The transform `amount` of the way from `self` to `other`.
    pub fn lerp(self, other: Self, amount: f32) -> Self {
        let mix = |from: f32, to: f32| from + (to - from) * amount;
        let length = |from: Length, to: Length| match (from, to) {
            (Length::Px(a), Length::Px(b)) => Length::Px(mix(a, b)),
            (Length::Fraction(a), Length::Fraction(b)) => Length::Fraction(mix(a, b)),
            (Length::Px(0.0), Length::Fraction(b)) => Length::Fraction(mix(0.0, b)),
            (Length::Fraction(a), Length::Px(0.0)) => Length::Fraction(mix(a, 0.0)),
            (_, to) => to,
        };
        Self {
            translate: (
                length(self.translate.0, other.translate.0),
                length(self.translate.1, other.translate.1),
            ),
            scale: (
                mix(self.scale.0, other.scale.0),
                mix(self.scale.1, other.scale.1),
            ),
            rotate: mix(self.rotate, other.rotate),
            skew: (
                mix(self.skew.0, other.skew.0),
                mix(self.skew.1, other.skew.1),
            ),
            origin: other.origin,
        }
    }
}

impl Default for Transform {
    /// [`Transform::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}
