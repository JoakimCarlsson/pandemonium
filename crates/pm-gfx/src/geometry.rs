//! Points, sizes and rectangles in logical pixels.

/// A position in logical pixels, measured from the top-left of the window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Distance from the left edge.
    pub x: f32,
    /// Distance from the top edge.
    pub y: f32,
}

impl Point {
    /// Creates a point at `x`, `y`.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns this point moved by `dx` and `dy`.
    pub const fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
}

/// A width and height in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// Extent along the x axis.
    pub width: f32,
    /// Extent along the y axis.
    pub height: f32,
}

impl Size {
    /// Creates a size of `width` by `height`.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Creates a size with both extents set to zero.
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// An axis-aligned rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// The top-left corner.
    pub origin: Point,
    /// The extent from the origin.
    pub size: Size,
}

impl Rect {
    /// Creates a rectangle from a corner and an extent.
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Creates a rectangle from loose coordinates.
    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::new(Point::new(x, y), Size::new(width, height))
    }

    /// The x coordinate of the left edge.
    pub const fn left(&self) -> f32 {
        self.origin.x
    }

    /// The y coordinate of the top edge.
    pub const fn top(&self) -> f32 {
        self.origin.y
    }

    /// The x coordinate just past the right edge.
    pub const fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    /// The y coordinate just past the bottom edge.
    pub const fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Whether `point` falls inside this rectangle.
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.left()
            && point.x < self.right()
            && point.y >= self.top()
            && point.y < self.bottom()
    }

    /// Returns this rectangle moved by `dx` and `dy`.
    pub const fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.origin.offset(dx, dy), self.size)
    }

    /// Returns this rectangle shrunk by `amount` on every edge.
    pub fn inset(self, amount: f32) -> Self {
        Self::from_xywh(
            self.left() + amount,
            self.top() + amount,
            (self.size.width - amount * 2.0).max(0.0),
            (self.size.height - amount * 2.0).max(0.0),
        )
    }

    /// Returns this rectangle grown by `amount` on every edge.
    pub fn outset(self, amount: f32) -> Self {
        Self::from_xywh(
            self.left() - amount,
            self.top() - amount,
            self.size.width + amount * 2.0,
            self.size.height + amount * 2.0,
        )
    }

    /// Whether this rectangle and `other` share any area at all.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.left() < other.right()
            && other.left() < self.right()
            && self.top() < other.bottom()
            && other.top() < self.bottom()
    }

    /// The largest rectangle contained by both this one and `other`.
    pub fn intersect(self, other: Self) -> Self {
        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Self::from_xywh(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
    }
}
