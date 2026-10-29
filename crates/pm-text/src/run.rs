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

/// Where the caret goes when it is moved one place along the screen in a line that holds right-to-left text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// To this byte of the line.
    To(usize),
    /// Off the end of the line the way it was moved.
    Edge,
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
    /// Whether some of the line is set right to left, so that its bytes and its glyphs do not run the same way.
    pub fn has_rtl(&self) -> bool {
        self.glyphs.iter().any(|glyph| glyph.level.is_rtl())
    }

    /// How far right of the start of the line the caret sits when it is at
    /// byte offset `byte` of the text, in logical pixels. In a line set only
    /// left to right that is the left edge of the character after it; where the
    /// line holds right-to-left text it is the place [`Run::step`] stops at for
    /// that byte, and a byte no stop is for stands at the edge of the character
    /// after it that it stands before, which is its right edge for a character
    /// set right to left.
    pub fn caret_x(&self, byte: usize) -> f32 {
        if !self.has_rtl() {
            return self
                .glyphs
                .iter()
                .find(|glyph| glyph.start >= byte)
                .map_or(self.width, |glyph| glyph.x);
        }
        if let Some(stop) = self.stops().into_iter().find(|stop| stop.1 == byte) {
            return stop.0;
        }
        if let Some(glyph) = self.glyphs.iter().find(|glyph| glyph.start == byte) {
            return if glyph.level.is_rtl() {
                glyph.x + glyph.w
            } else {
                glyph.x
            };
        }
        if let Some(glyph) = self.glyphs.iter().find(|glyph| glyph.end == byte) {
            return if glyph.level.is_rtl() {
                glyph.x
            } else {
                glyph.x + glyph.w
            };
        }
        self.glyphs
            .iter()
            .find(|glyph| glyph.start >= byte)
            .map_or(self.width, |glyph| glyph.x)
    }

    /// The stretches of the line, as left and right edges in logical pixels, that the bytes `range` are drawn over: one for text that runs one way, and one for each piece of a range that crosses between left to right and right to left.
    pub fn spans(&self, range: std::ops::Range<usize>) -> Vec<(f32, f32)> {
        if !self.has_rtl() || range.is_empty() {
            let (left, right) = (self.caret_x(range.start), self.caret_x(range.end));
            return vec![(left.min(right), left.max(right))];
        }
        let mut edges: Vec<(f32, f32)> = self
            .glyphs
            .iter()
            .filter(|glyph| range.contains(&glyph.start))
            .map(|glyph| (glyph.x, glyph.x + glyph.w))
            .collect();
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut joined: Vec<(f32, f32)> = Vec::new();
        for edge in edges {
            match joined.last_mut() {
                Some(last) if edge.0 <= last.1 + 0.5 => last.1 = last.1.max(edge.1),
                _ => joined.push(edge),
            }
        }
        joined
    }

    /// The places along the line a caret can stand when it is moved along the screen, left to right, each with the byte of the text it stands at: where two glyphs meet, the byte that text of the line's own direction typed there would land at, which is the one beside the glyph that is set left to right when only one is.
    fn stops(&self) -> Vec<(f32, usize)> {
        let mut edges: Vec<f32> = self
            .glyphs
            .iter()
            .filter(|glyph| glyph.w > 0.0)
            .flat_map(|glyph| [glyph.x, glyph.x + glyph.w])
            .collect();
        edges.sort_by(f32::total_cmp);
        edges.dedup_by(|later, earlier| (*later - *earlier).abs() < 0.01);
        let trailing = |glyph: &cosmic_text::LayoutGlyph| {
            if glyph.level.is_rtl() {
                glyph.start
            } else {
                glyph.end
            }
        };
        let leading = |glyph: &cosmic_text::LayoutGlyph| {
            if glyph.level.is_rtl() {
                glyph.end
            } else {
                glyph.start
            }
        };
        edges
            .into_iter()
            .filter_map(|edge| {
                let left = self
                    .glyphs
                    .iter()
                    .find(|glyph| glyph.w > 0.0 && (glyph.x + glyph.w - edge).abs() < 0.01);
                let right = self
                    .glyphs
                    .iter()
                    .find(|glyph| glyph.w > 0.0 && (glyph.x - edge).abs() < 0.01);
                let byte = match (left, right) {
                    (Some(left), Some(right)) => {
                        let (after, before) = (trailing(left), leading(right));
                        if after == before || !left.level.is_rtl() {
                            after
                        } else if !right.level.is_rtl() {
                            before
                        } else {
                            after
                        }
                    }
                    (Some(left), None) => trailing(left),
                    (None, Some(right)) => leading(right),
                    (None, None) => return None,
                };
                Some((edge, byte))
            })
            .collect()
    }

    /// Where the caret at byte `byte` goes when it is moved to the right on the screen, or to the left when `!right`, in a line that holds right-to-left text; nothing for a line set only left to right, where moving by characters is moving on the screen.
    pub fn step(&self, byte: usize, right: bool) -> Option<Step> {
        if !self.has_rtl() {
            return None;
        }
        let stops = self.stops();
        let here_x = self.caret_x(byte);
        let here = stops.iter().position(|stop| stop.1 == byte).or_else(|| {
            stops
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1.0 - here_x).abs().total_cmp(&(b.1.0 - here_x).abs()))
                .map(|(place, _)| place)
        })?;
        let next = if right {
            Some(here + 1)
        } else {
            here.checked_sub(1)
        };
        Some(
            next.and_then(|next| stops.get(next))
                .map_or(Step::Edge, |stop| Step::To(stop.1)),
        )
    }

    /// The glyphs of the line that stand for the bytes `range` of its text, placed where they are in the whole line.
    pub fn part(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            width: self.width,
            height: self.height,
            baseline: self.baseline,
            glyphs: self
                .glyphs
                .iter()
                .filter(|glyph| range.contains(&glyph.start))
                .cloned()
                .collect(),
        }
    }

    /// The byte offset of the character boundary nearest `x` logical pixels
    /// right of the start of the line, in a line of `length` bytes.
    pub fn byte_at(&self, x: f32, length: usize) -> usize {
        if self.has_rtl() {
            return self
                .stops()
                .into_iter()
                .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
                .map_or(length, |(_, byte)| byte.min(length));
        }
        self.glyphs
            .iter()
            .find(|glyph| x < glyph.x + glyph.w / 2.0)
            .map_or(length, |glyph| glyph.start)
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
