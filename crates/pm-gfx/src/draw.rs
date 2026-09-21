//! The draw list: what a caller submits for one frame.

use std::sync::Arc;

use crate::color::Rgba;
use crate::geometry::{Point, Rect, Size};
use crate::text::ShapedRun;

/// A rounded, optionally bordered rectangle.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Where the quad sits, in logical pixels.
    pub bounds: Rect,
    /// Fill colour inside the border.
    pub background: Rgba,
    /// Radius of all four corners.
    pub corner_radius: f32,
    /// Thickness of the border, drawn inside `bounds`.
    pub border_width: f32,
    /// Colour of the border.
    pub border_color: Rgba,
}

impl Quad {
    /// Creates a filled quad with square corners and no border.
    pub fn filled(bounds: Rect, background: Rgba) -> Self {
        Self {
            bounds,
            background,
            corner_radius: 0.0,
            border_width: 0.0,
            border_color: Rgba::TRANSPARENT,
        }
    }

    /// Returns this quad with rounded corners.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Returns this quad with a border of `width` in `color`.
    pub fn border(mut self, width: f32, color: Rgba) -> Self {
        self.border_width = width;
        self.border_color = color;
        self
    }

    /// Whether this quad would put no pixels on the screen.
    pub fn is_invisible(&self) -> bool {
        let no_fill = self.background.is_transparent();
        let no_border = self.border_width <= 0.0 || self.border_color.is_transparent();
        no_fill && no_border
    }
}

/// A shaped run placed on the screen in one colour.
#[derive(Clone)]
pub struct TextRun {
    /// Top-left corner of the run's line box.
    pub origin: Point,
    /// The shaped glyphs.
    pub run: Arc<ShapedRun>,
    /// Colour every glyph is drawn in.
    pub color: Rgba,
}

/// One frame's worth of primitives, in submission order.
///
/// Quads are drawn before text, so a quad pushed after a run still sits behind
/// it. Backgrounds are therefore free to be pushed in any order; overlaying a
/// quad *on top of* text is not expressible and is a second pass if it is ever
/// wanted.
pub struct DrawList {
    /// Quads with the clip rectangle in force when each was pushed.
    quads: Vec<(Quad, Rect)>,
    /// Text runs with the clip rectangle in force when each was pushed.
    texts: Vec<(TextRun, Rect)>,
    /// The clip stack, never empty; the last entry is in force.
    clips: Vec<Rect>,
}

impl DrawList {
    /// Creates an empty list covering a window of `size`.
    pub fn new(size: Size) -> Self {
        Self {
            quads: Vec::new(),
            texts: Vec::new(),
            clips: vec![Rect::new(Point::default(), size)],
        }
    }

    /// Drops every primitive and resets the clip to a window of `size`.
    pub fn reset(&mut self, size: Size) {
        self.quads.clear();
        self.texts.clear();
        self.clips.clear();
        self.clips.push(Rect::new(Point::default(), size));
    }

    /// The clip rectangle primitives are currently confined to.
    pub fn clip(&self) -> Rect {
        *self.clips.last().expect("clip stack is never empty")
    }

    /// Confines later primitives to `rect` as well as the current clip.
    pub fn push_clip(&mut self, rect: Rect) {
        let clip = self.clip().intersect(rect);
        self.clips.push(clip);
    }

    /// Restores the clip in force before the matching [`Self::push_clip`].
    pub fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    /// Adds a quad, skipping it when it would draw nothing.
    pub fn quad(&mut self, quad: Quad) {
        if quad.is_invisible() {
            return;
        }
        let clip = self.clip();
        self.quads.push((quad, clip));
    }

    /// Adds a shaped run at `origin` in `color`.
    pub fn text(&mut self, origin: Point, run: Arc<ShapedRun>, color: Rgba) {
        if color.is_transparent() || run.glyphs.is_empty() {
            return;
        }
        let clip = self.clip();
        self.texts.push((TextRun { origin, run, color }, clip));
    }

    /// The quads to draw, each with its clip rectangle.
    pub(crate) fn quads(&self) -> &[(Quad, Rect)] {
        &self.quads
    }

    /// The text runs to draw, each with its clip rectangle.
    pub(crate) fn texts(&self) -> &[(TextRun, Rect)] {
        &self.texts
    }
}
