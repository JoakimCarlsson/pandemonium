//! Where a box sits relative to its parent and to the other boxes.

use pm_primitives::{Edges, Length, Rule, Sides};

/// The positioning scheme of a box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Position {
    /// In normal flow; insets do nothing.
    #[default]
    Static,
    /// In normal flow, then shifted by its insets.
    Relative,
    /// Out of flow, placed against the nearest positioned ancestor.
    Absolute,
    /// Out of flow, placed against the window.
    Fixed,
    /// In normal flow until a scroll edge reaches its inset, then pinned.
    Sticky,
}

/// The stacking order of a positioned box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ZIndex {
    /// Stacks in tree order.
    #[default]
    Auto,
    /// Stacks on the given layer; higher is nearer.
    Layer(i32),
}

/// The positioning slice of a style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PositioningStyle {
    /// The positioning scheme.
    pub position: Position,
    /// Offsets from each edge of the reference box; [`Length::Auto`] leaves it unset.
    pub inset: Edges<Length>,
    /// The stacking order.
    pub z_index: ZIndex,
}

/// One declaration about positioning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PositioningRule {
    /// The positioning scheme.
    Position(Position),
    /// The inset on the given sides.
    Inset(Sides, Length),
    /// The stacking order.
    ZIndex(ZIndex),
}

impl Rule for PositioningRule {
    type Target = PositioningStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut PositioningStyle) {
        match self {
            Self::Position(position) => target.position = position,
            Self::Inset(sides, length) => target.inset.set_sides(sides, length),
            Self::ZIndex(z_index) => target.z_index = z_index,
        }
    }
}
