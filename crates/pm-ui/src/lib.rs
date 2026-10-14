//! The element tree: nodes styled with utilities, laid out with flex and grid,
//! and painted as quads.
//!
//! A screen is a function that builds a [`Node`] tree from the caller's own
//! state. [`draw`] lays it out against the viewport, resolves each node's
//! style for the pointer and the clock, and paints the result into a
//! [`pm_renderer::DrawList`]. The tree is rebuilt every frame; only the
//! subtrees made with [`retained`] survive to the next, built and laid out
//! once for as long as what they show stays the same. A box given a
//! [`Node::tooltip`] shows it over everything once the pointer has rested on
//! it for a moment. Text inside a box made [`Node::selectable`] can be
//! selected with the pointer: [`Input::selection`] says what is selected,
//! and [`Output`] finds places in the text and copies what a selection covers.

mod node;
mod paint;
mod retain;
mod select;
mod tooltip;
mod wrap;

pub use node::{Node, node, text};
pub use paint::{FieldBox, Input, Output, Region, Scroller, draw};
pub use pm_renderer::Picture;
pub use pm_style::Cursor;
pub use retain::retained;
pub use select::{DrawnPiece, TextPiece, TextPoint, TextSelection, Unit};
pub use wrap::{Line, wrap};
