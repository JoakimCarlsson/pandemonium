//! Layout: spacing, sizing, display, positioning, flex and grid.
//!
//! Each style module owns one slice of a [`LayoutStyle`], the plain data a
//! layout reads, and the rules that write into it. [`solve`] turns a tree of
//! [`Item`]s carrying those styles into placed rectangles.

mod breakpoint;
mod display;
mod flex;
mod grid;
mod positioning;
mod rule;
mod sizing;
mod solve;
mod spacing;
mod style;

pub use breakpoint::{Breakpoint, Container};
pub use display::{Display, DisplayRule, DisplayStyle, Overflow, Visibility};
pub use flex::{
    AlignContent, AlignItems, FlexDirection, FlexItem, FlexRule, FlexStyle, FlexWrap,
    JustifyContent,
};
pub use grid::{AutoFlow, GridItem, GridLine, GridRule, GridStyle, Track, TrackList};
pub use positioning::{Position, PositioningRule, PositioningStyle, ZIndex};
pub use rule::LayoutRule;
pub use sizing::{SizingRule, SizingStyle};
pub use solve::{Item, Placed, solve};
pub use spacing::{Margin, SpacingRule, SpacingStyle};
pub use style::LayoutStyle;
