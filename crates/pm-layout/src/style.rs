//! The layout slice of a style, assembled from its parts.

use crate::{
    DisplayStyle, FlexItem, FlexStyle, GridItem, GridStyle, PositioningStyle, SizingStyle,
    SpacingStyle,
};

/// Everything a layout pass reads about one element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutStyle {
    /// Padding, margin and gaps.
    pub spacing: SpacingStyle,
    /// Width, height and their bounds.
    pub sizing: SizingStyle,
    /// Layout kind, overflow and visibility.
    pub display: DisplayStyle,
    /// Position scheme, insets and stacking.
    pub positioning: PositioningStyle,
    /// How the element lays out its children when it is a flex container.
    pub flex: FlexStyle,
    /// How the element behaves as a flex item.
    pub flex_item: FlexItem,
    /// How the element lays out its children when it is a grid container.
    pub grid: GridStyle,
    /// How the element is placed when it is a grid item.
    pub grid_item: GridItem,
}
