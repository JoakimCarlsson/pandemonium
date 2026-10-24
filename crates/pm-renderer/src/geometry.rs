//! Rectangles and the affine transforms quads are drawn through.

/// An axis-aligned rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// The extent along x.
    pub width: f32,
    /// The extent along y.
    pub height: f32,
}

impl Rect {
    /// A rectangle from its top-left corner and its size.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The rectangle grown by `amount` on every side; negative shrinks it.
    pub fn inflate(self, amount: f32) -> Self {
        Self::new(
            self.x - amount,
            self.y - amount,
            (self.width + amount * 2.0).max(0.0),
            (self.height + amount * 2.0).max(0.0),
        )
    }

    /// The same rectangle moved by `dx` and `dy`.
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    /// Whether the point lies inside the rectangle.
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// A two-dimensional affine transform: `x' = a x + c y + tx`, `y' = b x + d y + ty`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    /// The x coefficient of x'.
    pub a: f32,
    /// The x coefficient of y'.
    pub b: f32,
    /// The y coefficient of x'.
    pub c: f32,
    /// The y coefficient of y'.
    pub d: f32,
    /// The shift along x.
    pub tx: f32,
    /// The shift along y.
    pub ty: f32,
}

impl Affine {
    /// Leaves every point where it is.
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    /// A transform from its six coefficients.
    pub const fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        Self { a, b, c, d, tx, ty }
    }

    /// A transform from `[a, b, c, d, tx, ty]`.
    pub const fn from_array(matrix: [f32; 6]) -> Self {
        Self::new(
            matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
        )
    }

    /// A shift by `dx` and `dy`.
    pub const fn translation(dx: f32, dy: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, dx, dy)
    }

    /// The transform that applies `self` first and `next` second.
    pub fn then(self, next: Self) -> Self {
        Self {
            a: next.a * self.a + next.c * self.b,
            b: next.b * self.a + next.d * self.b,
            c: next.a * self.c + next.c * self.d,
            d: next.b * self.c + next.d * self.d,
            tx: next.a * self.tx + next.c * self.ty + next.tx,
            ty: next.b * self.tx + next.d * self.ty + next.ty,
        }
    }

    /// Where the point goes.
    pub fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.tx,
            self.b * x + self.d * y + self.ty,
        )
    }
}

impl Default for Affine {
    /// [`Affine::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}
