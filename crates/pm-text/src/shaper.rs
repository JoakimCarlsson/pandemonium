//! The shaper: fonts loaded once, and runs shaped and rasterized against them.

use std::sync::Arc;

use cosmic_text::fontdb::Weight;
use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent,
};
use pm_typography::{FontFamily, FontSlant, TypographyStyle};

use crate::run::{Bitmap, GlyphKey, Run};

/// The room one run of text takes, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Extent {
    /// The width of the run.
    pub width: f32,
    /// The height of its line.
    pub height: f32,
}

/// Owns the system fonts and shapes and rasterizes text with them.
pub struct Shaper {
    /// The font database and shaping engine.
    fonts: FontSystem,
    /// The rasterizer for glyph outlines.
    swash: SwashCache,
}

impl Shaper {
    /// Loads the system fonts.
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            swash: SwashCache::new(),
        }
    }

    /// Shapes `text` on one line in `style`.
    pub fn shape(&mut self, text: &str, style: &TypographyStyle) -> Run {
        let line_height = style.line_height_px();
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(style.size, line_height));
        let family = match style.family {
            FontFamily::Sans => Family::SansSerif,
            FontFamily::Serif => Family::Serif,
            FontFamily::Mono => Family::Monospace,
        };
        let attrs = Attrs::new()
            .family(family)
            .weight(Weight(style.weight))
            .style(match style.slant {
                FontSlant::Normal => Style::Normal,
                FontSlant::Italic => Style::Italic,
            });
        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_size(None, None);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        match buffer.layout_runs().next() {
            Some(line) => Run {
                width: line.line_w,
                height: line_height,
                baseline: line.line_y - line.line_top,
                glyphs: Arc::from(line.glyphs),
            },
            None => Run {
                width: 0.0,
                height: line_height,
                baseline: line_height,
                glyphs: Arc::from([]),
            },
        }
    }

    /// The extent `text` occupies on one line in `style`.
    pub fn measure(&mut self, text: &str, style: &TypographyStyle) -> Extent {
        let run = self.shape(text, style);
        Extent {
            width: run.width,
            height: run.height,
        }
    }

    /// The coverage bitmap of one glyph, or nothing for a glyph with no ink
    /// or one that is not a plain mask.
    pub fn rasterize(&mut self, key: GlyphKey) -> Option<Bitmap> {
        let image = self.swash.get_image_uncached(&mut self.fonts, key.0)?;
        if image.content != SwashContent::Mask || image.placement.width == 0 {
            return None;
        }
        Some(Bitmap {
            width: image.placement.width,
            height: image.placement.height,
            left: image.placement.left,
            top: image.placement.top,
            coverage: image.data,
        })
    }
}

impl Default for Shaper {
    /// [`Shaper::new`].
    fn default() -> Self {
        Self::new()
    }
}
