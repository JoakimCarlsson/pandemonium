//! The element tree: nodes styled with utilities, laid out with flex and grid,
//! and painted as quads.
//!
//! A screen is a function that builds a [`Node`] tree from the caller's own
//! state. [`draw`] lays it out against the viewport, resolves each node's
//! style for the pointer and the clock, and paints the result into a
//! [`pm_renderer::DrawList`]. The tree is rebuilt every frame; nothing of it
//! survives to the next.

mod node;
mod paint;
mod wrap;

pub use node::{Node, node, text};
pub use paint::{FieldBox, Input, Output, Region, Scroller, draw};
pub use pm_renderer::Picture;
pub use pm_style::Cursor;
pub use wrap::{Line, wrap};
