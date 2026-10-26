//! The element tree: nodes styled with utilities, laid out with flex and grid,
//! and painted as quads.
//!
//! A screen is a function that builds a [`Node`] tree from the caller's own
//! state. [`draw`] lays it out against the viewport, resolves each node's
//! style for the pointer and the clock, and paints the result into a
//! [`pm_renderer::DrawList`]. A [`Scene`] keeps the complete tree and layout
//! across interaction frames; [`retained`] keeps individual subtrees built
//! and laid out for as long as their content stays the same. A box given a
//! [`Node::tooltip`] shows it over everything once the pointer has rested on
//! it for a moment. Text inside a box made [`Node::selectable`] can be
//! selected with the pointer: [`Input::selection`] says what is selected,
//! and [`Output`] finds places in the text and copies what a selection covers.

mod field_layout;
mod node;
mod paint;
mod retain;
mod scene;
mod select;
mod tooltip;
mod wrap;

pub use field_layout::FieldLayout;
pub use node::{CaretLook, CaretShape, Mark, Node, Paint, node, text};
pub use paint::{FieldBox, Input, Output, Region, SCROLLBAR_GRIP, Scroller, draw};
pub use pm_renderer::Picture;
pub use pm_style::Cursor;
pub use retain::retained;
pub use scene::Scene;
pub use select::{DrawnPiece, TextPiece, TextPoint, TextSelection};
pub use wrap::{Line, wrap};
