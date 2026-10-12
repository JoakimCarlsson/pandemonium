//! The layout rule: one declaration of any layout concern.

use pm_primitives::Rule;

use crate::{
    DisplayRule, FlexRule, GridRule, LayoutStyle, PositioningRule, SizingRule, SpacingRule,
};

/// One declaration about layout, whichever concern it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutRule {
    /// Padding, margin or gap.
    Spacing(SpacingRule),
    /// Width, height or their bounds.
    Sizing(SizingRule),
    /// Layout kind, overflow or visibility.
    Display(DisplayRule),
    /// Position scheme, inset or stacking.
    Positioning(PositioningRule),
    /// Flex container or item property.
    Flex(FlexRule),
    /// Grid container or item property.
    Grid(GridRule),
}

impl Rule for LayoutRule {
    type Target = LayoutStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut LayoutStyle) {
        match self {
            Self::Spacing(rule) => rule.apply(&mut target.spacing),
            Self::Sizing(rule) => rule.apply(&mut target.sizing),
            Self::Display(rule) => rule.apply(&mut target.display),
            Self::Positioning(rule) => rule.apply(&mut target.positioning),
            Self::Flex(rule) => {
                let mut pair = (target.flex, target.flex_item);
                rule.apply(&mut pair);
                (target.flex, target.flex_item) = pair;
            }
            Self::Grid(rule) => {
                let mut pair = (target.grid, target.grid_item);
                rule.apply(&mut pair);
                (target.grid, target.grid_item) = pair;
            }
        }
    }
}
