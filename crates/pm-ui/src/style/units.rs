//! The spacing scale and the enumerations a style is expressed in.
//!
//! Every measurement in the UI is a step of one scale, so a gap and a padding
//! chosen independently still line up. Four logical pixels a step, as Tailwind
//! has it: `p(4)` is sixteen pixels, exactly as `p-4` is there.

/// Logical pixels in one step of the spacing scale.
pub const STEP: f32 = 4.0;

/// The size of `steps` on the spacing scale, in logical pixels.
pub const fn space(steps: f32) -> f32 {
    steps * STEP
}

/// The axis a container stacks its children along.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    /// Children are placed left to right.
    Horizontal,
    /// Children are placed top to bottom.
    Vertical,
}

/// How an element is sized along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// As large as the element's own content needs.
    Auto,
    /// Exactly this many logical pixels.
    Px(f32),
    /// The whole extent the parent offers.
    Full,
}

/// How children are aligned on the axis they are *not* stacked along.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Align {
    /// Packed against the start edge.
    Start,
    /// Centred.
    Center,
    /// Packed against the end edge.
    End,
    /// Stretched to fill the cross axis.
    Stretch,
}

/// How leftover space on the stacking axis is distributed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Justify {
    /// Children sit at the start, leftover space after them.
    Start,
    /// Children sit in the middle of the leftover space.
    Center,
    /// Children sit at the end, leftover space before them.
    End,
    /// Leftover space is split evenly between children.
    Between,
}

/// Padding or inset on each edge, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    /// Space above the content.
    pub top: f32,
    /// Space right of the content.
    pub right: f32,
    /// Space below the content.
    pub bottom: f32,
    /// Space left of the content.
    pub left: f32,
}

impl Edges {
    /// The same amount on every edge.
    pub const fn all(amount: f32) -> Self {
        Self {
            top: amount,
            right: amount,
            bottom: amount,
            left: amount,
        }
    }

    /// Total space taken on the horizontal axis.
    pub const fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total space taken on the vertical axis.
    pub const fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}
