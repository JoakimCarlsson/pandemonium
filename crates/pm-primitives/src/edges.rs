//! Values that come once per side or once per corner, and the axes they run along.

/// One side of a box.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Side {
    /// The top edge.
    Top,
    /// The right-hand edge.
    Right,
    /// The bottom edge.
    Bottom,
    /// The left-hand edge.
    Left,
}

/// One corner of a box.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Corner {
    /// Where the top and left edges meet.
    TopLeft,
    /// Where the top and right edges meet.
    TopRight,
    /// Where the bottom and right edges meet.
    BottomRight,
    /// Where the bottom and left edges meet.
    BottomLeft,
}

/// A direction of travel: across the page or down it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
}

/// Which sides of a box a setting reaches: every side, a pair, or one.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Sides {
    /// All four sides.
    All,
    /// Left and right.
    Horizontal,
    /// Top and bottom.
    Vertical,
    /// The top side alone.
    Top,
    /// The right-hand side alone.
    Right,
    /// The bottom side alone.
    Bottom,
    /// The left-hand side alone.
    Left,
}

/// Which corners of a box a setting reaches: every corner, the two along a
/// side, or one.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CornerGroup {
    /// All four corners.
    All,
    /// The two corners along one side.
    Side(Side),
    /// A single corner.
    One(Corner),
}

/// One value per side of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges<T> {
    /// The value on the top edge.
    pub top: T,
    /// The value on the right-hand edge.
    pub right: T,
    /// The value on the bottom edge.
    pub bottom: T,
    /// The value on the left-hand edge.
    pub left: T,
}

impl<T: Copy> Edges<T> {
    /// The same value on every side.
    pub const fn all(value: T) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// One value for the left and right sides and another for the top and bottom.
    pub const fn symmetric(horizontal: T, vertical: T) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// The value on `side`.
    pub const fn get(&self, side: Side) -> T {
        match side {
            Side::Top => self.top,
            Side::Right => self.right,
            Side::Bottom => self.bottom,
            Side::Left => self.left,
        }
    }

    /// Replaces the value on `side`.
    pub fn set(&mut self, side: Side, value: T) {
        match side {
            Side::Top => self.top = value,
            Side::Right => self.right = value,
            Side::Bottom => self.bottom = value,
            Side::Left => self.left = value,
        }
    }

    /// Replaces the values on both sides of `axis`.
    pub fn set_axis(&mut self, axis: Axis, value: T) {
        match axis {
            Axis::Horizontal => {
                self.left = value;
                self.right = value;
            }
            Axis::Vertical => {
                self.top = value;
                self.bottom = value;
            }
        }
    }

    /// Replaces the value on every side.
    pub fn set_all(&mut self, value: T) {
        *self = Self::all(value);
    }

    /// Replaces the value on each side `sides` names.
    pub fn set_sides(&mut self, sides: Sides, value: T) {
        match sides {
            Sides::All => self.set_all(value),
            Sides::Horizontal => self.set_axis(Axis::Horizontal, value),
            Sides::Vertical => self.set_axis(Axis::Vertical, value),
            Sides::Top => self.top = value,
            Sides::Right => self.right = value,
            Sides::Bottom => self.bottom = value,
            Sides::Left => self.left = value,
        }
    }

    /// Applies `convert` to every side.
    pub fn map<U>(self, mut convert: impl FnMut(T) -> U) -> Edges<U> {
        Edges {
            top: convert(self.top),
            right: convert(self.right),
            bottom: convert(self.bottom),
            left: convert(self.left),
        }
    }
}

impl Edges<f32> {
    /// The combined extent of the left and right sides.
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// The combined extent of the top and bottom sides.
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// One value per corner of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners<T> {
    /// The value at the top-left corner.
    pub top_left: T,
    /// The value at the top-right corner.
    pub top_right: T,
    /// The value at the bottom-right corner.
    pub bottom_right: T,
    /// The value at the bottom-left corner.
    pub bottom_left: T,
}

impl<T: Copy> Corners<T> {
    /// The same value at every corner.
    pub const fn all(value: T) -> Self {
        Self {
            top_left: value,
            top_right: value,
            bottom_right: value,
            bottom_left: value,
        }
    }

    /// The value at `corner`.
    pub const fn get(&self, corner: Corner) -> T {
        match corner {
            Corner::TopLeft => self.top_left,
            Corner::TopRight => self.top_right,
            Corner::BottomRight => self.bottom_right,
            Corner::BottomLeft => self.bottom_left,
        }
    }

    /// Replaces the value at `corner`.
    pub fn set(&mut self, corner: Corner, value: T) {
        match corner {
            Corner::TopLeft => self.top_left = value,
            Corner::TopRight => self.top_right = value,
            Corner::BottomRight => self.bottom_right = value,
            Corner::BottomLeft => self.bottom_left = value,
        }
    }

    /// Replaces the value at both corners of `side`.
    pub fn set_side(&mut self, side: Side, value: T) {
        let (first, second) = match side {
            Side::Top => (Corner::TopLeft, Corner::TopRight),
            Side::Right => (Corner::TopRight, Corner::BottomRight),
            Side::Bottom => (Corner::BottomRight, Corner::BottomLeft),
            Side::Left => (Corner::BottomLeft, Corner::TopLeft),
        };
        self.set(first, value);
        self.set(second, value);
    }

    /// Replaces the value at every corner.
    pub fn set_all(&mut self, value: T) {
        *self = Self::all(value);
    }

    /// Replaces the value at each corner `group` names.
    pub fn set_group(&mut self, group: CornerGroup, value: T) {
        match group {
            CornerGroup::All => self.set_all(value),
            CornerGroup::Side(side) => self.set_side(side, value),
            CornerGroup::One(corner) => self.set(corner, value),
        }
    }
}
