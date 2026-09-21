//! GPU device, glyph atlas, text shaping and draw-list submission.
//!
//! Callers build a [`DrawList`] of quads and shaped text in logical pixels and
//! hand it to [`Renderer::render`]. Queues, encoders, pipelines and bind groups
//! stay behind this boundary.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod atlas;
mod color;
mod draw;
mod geometry;
mod pipeline;
mod renderer;
mod text;

pub use color::Rgba;
pub use draw::{DrawList, Quad, TextRun};
pub use geometry::{Point, Rect, Size};
pub use renderer::Renderer;
pub use text::{FontStyle, ShapedRun, TextSystem};
