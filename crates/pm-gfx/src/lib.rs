//! GPU device, glyph atlas, text shaping and draw-list submission.
//!
//! Callers build a [`DrawList`] of quads, pictures and shaped text in logical pixels and
//! hand it to [`Renderer::render`]. Queues, encoders, pipelines and bind groups
//! stay behind this boundary.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod atlas;
mod color;
mod draw;
mod geometry;
mod image;
mod pipeline;
mod renderer;
mod svg;
mod text;
mod textures;

pub use color::Rgba;
pub use draw::{DrawList, IconRun, ImageRun, Layer, Quad, TextRun};
pub use geometry::{Point, Rect, Size};
pub use image::Image;
pub use renderer::Renderer;
pub use svg::Svg;
pub use text::{FontFamily, FontStyle, ShapedRun, TextSystem};
