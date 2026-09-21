//! How far a scrollable area is scrolled, and what that does to its layout.
//!
//! Scrolling is a property of the area being scrolled, not of the window: the
//! content is laid out in a space as tall as it needs and painted at a
//! negative offset, and the offset is clamped against what the last frame
//! actually painted. One of these belongs to each area that scrolls, so two
//! of them side by side scroll independently.

use pm_gfx::{Point, Size};

/// The scroll offset of one area, and the extents it is clamped against.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scroll {
    /// How far the content is shifted up, in logical pixels.
    offset: f32,
    /// The height the last frame painted, in logical pixels.
    content_height: f32,
    /// The space the content is seen through, in logical pixels.
    viewport: Size,
}

impl Scroll {
    /// Records the space the content is seen through.
    pub fn set_viewport(&mut self, viewport: Size) {
        self.viewport = viewport;
        self.clamp();
    }

    /// Records how tall the content turned out, once a frame has painted it.
    pub fn set_content_height(&mut self, height: f32) {
        self.content_height = height;
        self.clamp();
    }

    /// Scrolls by `delta` logical pixels, positive being towards the top.
    pub fn by(&mut self, delta: f32) {
        self.offset -= delta;
        self.clamp();
    }

    /// The space to lay the content out in: as tall as the offset reaches.
    pub fn content_space(&self) -> Size {
        Size::new(self.viewport.width, self.viewport.height + self.offset)
    }

    /// Where the content's top-left corner lands at this offset.
    pub fn origin(&self) -> Point {
        Point::new(0.0, -self.offset)
    }

    /// Holds the offset between the top and the end of the content.
    fn clamp(&mut self) {
        let limit = (self.content_height - self.viewport.height).max(0.0);
        self.offset = self.offset.clamp(0.0, limit);
    }
}
