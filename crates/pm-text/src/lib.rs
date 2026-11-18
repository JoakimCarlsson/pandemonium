//! Text: shaping runs of text in a typography style, measuring them and
//! rasterizing their glyphs.
//!
//! The styling of text is `pm-typography`'s; this crate turns a string and a
//! [`pm_typography::TypographyStyle`] into a [`Run`] of positioned glyphs,
//! and a glyph into a coverage bitmap a renderer can draw. [`word_at`] finds
//! the word around a place in a text.

mod run;
mod shaper;

pub use pm_field::{Clicks, Edit, Field, Gesture, Motion, Reach, Surface, word_at, word_at_with};
pub use run::{Bitmap, GlyphKey, PlacedGlyph, Run, Step};
pub use shaper::{Extent, Shaper};
