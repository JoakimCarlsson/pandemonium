//! Padding, margin and the gap between children.

use pm_primitives::{Edges, Rule, Sides};

/// The outer space on one side of a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Margin {
    /// An exact number of logical pixels; negative pulls the box outward.
    Px(f32),
    /// Takes an equal share of the leftover space.
    Auto,
}

impl Default for Margin {
    /// No margin.
    fn default() -> Self {
        Self::Px(0.0)
    }
}

/// The space inside, around and between boxes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpacingStyle {
    /// Space between a box's border and its content, in logical pixels.
    pub padding: Edges<f32>,
    /// Space outside a box's border.
    pub margin: Edges<Margin>,
    /// Space between adjacent columns, in logical pixels.
    pub column_gap: f32,
    /// Space between adjacent rows, in logical pixels.
    pub row_gap: f32,
}

/// One declaration about spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpacingRule {
    /// Padding on the given sides, in logical pixels.
    Padding(Sides, f32),
    /// Margin on the given sides.
    Margin(Sides, Margin),
    /// The gap between rows and columns, in logical pixels.
    Gap(f32),
    /// The gap between columns, in logical pixels.
    ColumnGap(f32),
    /// The gap between rows, in logical pixels.
    RowGap(f32),
}

impl Rule for SpacingRule {
    type Target = SpacingStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut SpacingStyle) {
        match self {
            Self::Padding(sides, pixels) => target.padding.set_sides(sides, pixels),
            Self::Margin(sides, margin) => target.margin.set_sides(sides, margin),
            Self::Gap(pixels) => {
                target.column_gap = pixels;
                target.row_gap = pixels;
            }
            Self::ColumnGap(pixels) => target.column_gap = pixels,
            Self::RowGap(pixels) => target.row_gap = pixels,
        }
    }
}
