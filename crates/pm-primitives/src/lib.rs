//! The vocabulary every style crate shares: the spacing step and how big it
//! and interface text are built, lengths, the four-sided and four-cornered
//! containers, axes and the rule trait.
//!
//! Nothing here knows about colour, text or painting; each style crate builds
//! its own rules from these pieces and `pm-style` assembles them.

mod edges;
mod length;
mod rule;
mod scale;
mod step;

pub use edges::{Axis, Corner, CornerGroup, Corners, Edges, Side, Sides};
pub use length::Length;
pub use rule::Rule;
pub use scale::{set_scales, step_scale, text_scale};
pub use step::{Number, STEP_PX};
