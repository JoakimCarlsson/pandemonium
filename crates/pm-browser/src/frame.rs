//! What a page paints.

use std::sync::Arc;

/// A rectangle of whole device pixels of a picture, from its top left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Region {
    /// The left edge.
    pub x: u32,
    /// The top edge.
    pub y: u32,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
}

impl Region {
    /// The whole of a picture `width` by `height` pixels.
    pub fn whole(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// A rectangle from the left, top, width and height the helper reports, cut to a picture `width` by `height`.
    pub fn reported([x, y, wide, tall]: [u32; 4], width: u32, height: u32) -> Self {
        let x = x.min(width);
        let y = y.min(height);
        Self {
            x,
            y,
            width: wide.min(width - x),
            height: tall.min(height - y),
        }
    }

    /// Whether the rectangle covers no pixel.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The smallest rectangle holding both this one and `other`; an empty one adds nothing.
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

/// The pixels a page painted, at its device resolution.
#[derive(Clone, Debug)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// How many paints the page has made when this one was taken; a larger number is a newer picture.
    pub generation: u64,
    /// Tightly packed pixels, four bytes each in the order [`Frame::bgra`] says, row by row from the top, shared so a
    /// renderer can take them without a copy.
    pub pixels: Arc<[u8]>,
    /// Whether each pixel is stored blue, green, red, alpha, as the engine paints, rather than red, green, blue, alpha.
    pub bgra: bool,
    /// The generation of the picture handed out before this one and the part of this one that differs from it, when
    /// both are known; a renderer still holding that picture needs to take only this part of the new one.
    pub changed: Option<(u64, Region)>,
}
