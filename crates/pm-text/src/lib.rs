//! Text: shaping runs of text in a typography style, measuring them and
//! rasterizing their glyphs.
//!
//! The styling of text is `pm-typography`'s; this crate turns a string and a
//! [`pm_typography::TypographyStyle`] into a [`Run`] of positioned glyphs,
//! and a glyph into a coverage bitmap a renderer can draw.

mod field;
mod run;
mod shaper;

pub use field::{Edit, Field, Motion};
pub use run::{Bitmap, GlyphKey, PlacedGlyph, Run};
pub use shaper::{Extent, Shaper};
