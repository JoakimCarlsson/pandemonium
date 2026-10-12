//! A shaped line of text and the glyphs it is made of.

use std::sync::Arc;

use cosmic_text::{CacheKey, LayoutGlyph};

/// Identifies one glyph at one size and sub-pixel position, for caching its bitmap.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GlyphKey(pub(crate) CacheKey);

/// One glyph of a run, placed on the physical pixel grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedGlyph {
    /// What to rasterize.
    pub key: GlyphKey,
    /// The pen position along x, in physical pixels.
    pub x: i32,
    /// The baseline position along y, in physical pixels.
    pub y: i32,
}

/// The coverage of one glyph: how much of each pixel it fills.
#[derive(Clone, Debug, Default)]
pub struct Bitmap {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// How far right of the pen the left edge is.
    pub left: i32,
    /// How far above the baseline the top edge is.
    pub top: i32,
    /// One coverage byte per pixel, row by row from the top.
    pub coverage: Vec<u8>,
}

/// One shaped line: its extent and the glyphs in it.
#[derive(Clone, Debug)]
pub struct Run {
    /// The width of the line in logical pixels.
    pub width: f32,
    /// The height of the line in logical pixels.
    pub height: f32,
    /// How far below the top of the line the baseline is, in logical pixels.
    pub baseline: f32,
    /// The glyphs, in visual order.
    pub(crate) glyphs: Arc<[LayoutGlyph]>,
}

impl Run {
    /// How far right of the start of the line the caret sits when it is at
    /// byte offset `byte` of the text, in logical pixels.
    pub fn caret_x(&self, byte: usize) -> f32 {
        self.glyphs
            .iter()
            .find(|glyph| glyph.start >= byte)
            .map_or(self.width, |glyph| glyph.x)
    }

    /// The glyphs placed for a line whose top-left corner is at `x`, `y`
    /// logical pixels, drawn at `scale` physical pixels per logical pixel.
    pub fn place(&self, x: f32, y: f32, scale: f32) -> impl Iterator<Item = PlacedGlyph> + '_ {
        let offset = (x * scale, (y + self.baseline) * scale);
        self.glyphs.iter().map(move |glyph| {
            let physical = glyph.physical(offset, scale);
            PlacedGlyph {
                key: GlyphKey(physical.cache_key),
                x: physical.x,
                y: physical.y,
            }
        })
    }
}
