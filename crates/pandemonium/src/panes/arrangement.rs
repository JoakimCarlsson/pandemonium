//! Layout presets describing where existing tabs should be placed.

use pm_ui::Axis;

use crate::panes::Item;

/// A division of panes built from tabs already held by the window.
pub enum Arrangement {
    /// A group of tabs, in their new order.
    Pane(Vec<Item>),
    /// A split whose children receive the given shares of its space.
    Split {
        /// The axis dividing the children.
        axis: Axis,
        /// Each child's share and its arrangement, in drawing order.
        children: Vec<(f32, Self)>,
    },
}
