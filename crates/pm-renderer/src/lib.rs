//! The GPU device, the target and the frame each redraw presents.
//!
//! This is the one crate that talks to wgpu. Callers build a [`DrawList`] of
//! quads in logical pixels and hand it over; they never see a queue, an
//! encoder or a bind group. A [`Renderer`] draws into a window, or into a
//! texture whose pixels can be read back.

mod atlas;
mod draw;
mod geometry;
mod pipeline;
mod renderer;

pub use draw::{DrawList, Icon, Primitive, Quad, Text};
pub use geometry::{Affine, Rect};
pub use renderer::{Renderer, RendererError};
