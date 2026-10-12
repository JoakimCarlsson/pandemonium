//! The vocabulary every style crate shares: the spacing step, lengths, the
//! four-sided and four-cornered containers, axes and the rule trait.
//!
//! Nothing here knows about colour, text or painting; each style crate builds
//! its own rules from these pieces and `pm-style` assembles them.

mod edges;
mod length;
mod rule;
mod step;

pub use edges::{Axis, Corner, CornerGroup, Corners, Edges, Side, Sides};
pub use length::Length;
pub use rule::Rule;
pub use step::{Number, STEP_PX};
