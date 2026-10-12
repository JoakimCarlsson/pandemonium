//! Position scheme, insets and stacking.

use pm_layout::{Position, PositioningRule, ZIndex};
use pm_primitives::{Length, Number, Sides};

use crate::Utility;

/// The inset on `sides`, as any length.
pub fn inset_length(sides: Sides, length: Length) -> Utility {
    PositioningRule::Inset(sides, length).into()
}

/// Inset from every edge, in steps; negative moves outward.
pub fn inset(steps: impl Number) -> Utility {
    inset_length(Sides::All, Length::steps(steps))
}

/// Inset from the left and right edges, in steps.
pub fn inset_x(steps: impl Number) -> Utility {
    inset_length(Sides::Horizontal, Length::steps(steps))
}

/// Inset from the top and bottom edges, in steps.
pub fn inset_y(steps: impl Number) -> Utility {
    inset_length(Sides::Vertical, Length::steps(steps))
}

/// Inset from the top edge, in steps.
pub fn top(steps: impl Number) -> Utility {
    inset_length(Sides::Top, Length::steps(steps))
}

/// Inset from the right edge, in steps.
pub fn right(steps: impl Number) -> Utility {
    inset_length(Sides::Right, Length::steps(steps))
}

/// Inset from the bottom edge, in steps.
pub fn bottom(steps: impl Number) -> Utility {
    inset_length(Sides::Bottom, Length::steps(steps))
}

/// Inset from the left edge, in steps.
pub fn left(steps: impl Number) -> Utility {
    inset_length(Sides::Left, Length::steps(steps))
}

/// Stacking layer; higher is nearer.
pub fn z(layer: i32) -> Utility {
    PositioningRule::ZIndex(ZIndex::Layer(layer)).into()
}

constants! {
    /// In normal flow.
    static_position = PositioningRule::Position(Position::Static);
    /// In normal flow, shifted by its insets.
    relative = PositioningRule::Position(Position::Relative);
    /// Out of flow, against the nearest positioned ancestor.
    absolute = PositioningRule::Position(Position::Absolute);
    /// Out of flow, against the window.
    fixed = PositioningRule::Position(Position::Fixed);
    /// Pinned once a scroll edge reaches its inset.
    sticky = PositioningRule::Position(Position::Sticky);
    /// Inset from every edge by zero.
    inset_0 = PositioningRule::Inset(Sides::All, Length::Px(0.0));
    /// Inset from every edge by the parent's whole extent.
    inset_full = PositioningRule::Inset(Sides::All, Length::FULL);
    /// Insets left to layout.
    inset_auto = PositioningRule::Inset(Sides::All, Length::Auto);
    /// Stacks in tree order.
    z_auto = PositioningRule::ZIndex(ZIndex::Auto);
}
