//! The GPU device, the surface and the frame each redraw presents.
//!
//! This is the one crate that talks to wgpu. Callers hand it a window and ask
//! for frames; they never see a queue, an encoder or a bind group.

mod renderer;

pub use renderer::{Renderer, RendererError};
