//! Text: shaping runs of text in a typography style, measuring them and
//! rasterizing their glyphs.
//!
//! The styling of text is `pm-typography`'s; this crate turns a string and a
//! [`pm_typography::TypographyStyle`] into a [`Run`] of positioned glyphs,
//! and a glyph into a coverage bitmap a renderer can draw. [`word_at`] finds
//! the word around a place in a text.

mod field;
mod reach;
mod run;
mod shaper;
mod word;

pub use field::{Edit, Field, Motion};
pub use reach::{Clicks, Gesture, Reach, Surface};
pub use run::{Bitmap, GlyphKey, PlacedGlyph, Run, Step};
pub use shaper::{Extent, Shaper};
pub use word::{word_at, word_at_with};
