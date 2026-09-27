//! The draw list: what a caller submits for one frame.

use std::sync::Arc;

use crate::color::Rgba;
use crate::geometry::{Point, Rect, Size};
use crate::image::Image;
use crate::svg::Svg;
use crate::text::ShapedRun;

/// A rounded, optionally bordered rectangle.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Where the quad sits, in logical pixels.
    pub bounds: Rect,
    /// Fill colour inside the border.
    pub background: Rgba,
    /// Corner radii, clockwise from the top-left corner.
    pub corner_radii: [f32; 4],
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
            corner_radii: [0.0; 4],
            border_width: 0.0,
            border_color: Rgba::TRANSPARENT,
        }
    }

    /// Returns this quad with every corner rounded by `radius`.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radii = [radius; 4];
        self
    }

    /// Returns this quad with each corner rounded on its own.
    ///
    /// The radii run clockwise from the top-left corner, the way CSS writes
    /// them, so a shape can be flat where it meets its neighbour.
    pub fn corner_radii(mut self, radii: [f32; 4]) -> Self {
        self.corner_radii = radii;
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

/// One icon placed on the screen in one colour.
#[derive(Clone, Copy)]
pub struct IconRun {
    /// The square the icon is drawn inside.
    pub bounds: Rect,
    /// The artwork to draw.
    pub svg: Svg,
    /// Colour the artwork's coverage is tinted with.
    pub color: Rgba,
    /// Clockwise rotation around the icon's centre, in radians.
    pub rotation: f32,
}

/// One picture placed on the screen, in its own colours.
#[derive(Clone)]
pub struct ImageRun {
    /// The rectangle the picture is stretched over.
    pub bounds: Rect,
    /// The picture to draw.
    pub image: Image,
}

/// Where a primitive sits in the stack of things drawn over each other.
///
/// Within one layer quads are drawn before pictures and pictures before text,
/// which is what a background behind a label wants. Between layers nothing of a lower one is drawn over
/// anything of a higher one, which is what a menu over a screen wants.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Layer(pub u32);

/// One frame's worth of primitives, in submission order.
///
/// A primitive is drawn over the ones pushed before it in its own layer, and
/// over everything in every layer below it. Within a layer quads come before
/// pictures and pictures before text; drawing a quad over text means opening
/// a layer for it.
pub struct DrawList {
    /// Quads with the clip and layer in force when each was pushed.
    quads: Vec<(Quad, Rect, Layer)>,
    /// Text runs with the clip and layer in force when each was pushed.
    texts: Vec<(TextRun, Rect, Layer)>,
    /// Icons with the clip and layer in force when each was pushed.
    icons: Vec<(IconRun, Rect, Layer)>,
    /// Pictures with the clip and layer in force when each was pushed.
    images: Vec<(ImageRun, Rect, Layer)>,
    /// The clip stack, never empty; the last entry is in force.
    clips: Vec<Rect>,
    /// The layer primitives are going into.
    layer: Layer,
    /// The layers to come back to, innermost last.
    layers: Vec<Layer>,
    /// The highest layer opened this frame.
    opened: Layer,
}

impl DrawList {
    /// Creates an empty list covering a window of `size`.
    pub fn new(size: Size) -> Self {
        Self {
            quads: Vec::new(),
            texts: Vec::new(),
            icons: Vec::new(),
            images: Vec::new(),
            clips: vec![Rect::new(Point::default(), size)],
            layer: Layer::default(),
            layers: Vec::new(),
            opened: Layer::default(),
        }
    }

    /// Drops every primitive and resets the clip to a window of `size`.
    pub fn reset(&mut self, size: Size) {
        self.quads.clear();
        self.texts.clear();
        self.icons.clear();
        self.images.clear();
        self.clips.clear();
        self.clips.push(Rect::new(Point::default(), size));
        self.layer = Layer::default();
        self.layers.clear();
        self.opened = Layer::default();
    }

    /// The window the list is drawn for.
    pub fn viewport(&self) -> Rect {
        *self.clips.first().expect("clip stack is never empty")
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

    /// Opens a layer over everything drawn so far, for an overlay.
    pub fn push_layer(&mut self) {
        self.opened = Layer(self.opened.0 + 1);
        self.layers.push(self.layer);
        self.layer = self.opened;
    }

    /// Returns to the layer in force before the matching [`Self::push_layer`].
    pub fn pop_layer(&mut self) {
        self.layer = self.layers.pop().unwrap_or_default();
    }

    /// Whether anything drawn inside `bounds` would land inside the clip.
    ///
    /// A list that is not virtualised, a file tree or a long document,
    /// pushes every row whether it is scrolled into view or not; the rows the
    /// clip hides entirely are dropped here, before they cost an instance.
    fn is_visible(&self, bounds: Rect) -> bool {
        bounds.overlaps(&self.clip())
    }

    /// Adds a quad, skipping it when it would draw nothing.
    pub fn quad(&mut self, quad: Quad) {
        if quad.is_invisible() || !self.is_visible(quad.bounds) {
            return;
        }
        let clip = self.clip();
        self.quads.push((quad, clip, self.layer));
    }

    /// Adds a shaped run at `origin` in `color`.
    ///
    /// The run is tested against the clip with a line's height of slack on
    /// every side, because a glyph's ink may reach past its line box: a slanted
    /// letter past the advance, an accent above the ascent.
    pub fn text(&mut self, origin: Point, run: Arc<ShapedRun>, color: Rgba) {
        if color.is_transparent() || run.glyphs.is_empty() {
            return;
        }
        let ink = Rect::new(origin, run.size()).outset(run.height);
        if !self.is_visible(ink) {
            return;
        }
        let clip = self.clip();
        self.texts
            .push((TextRun { origin, run, color }, clip, self.layer));
    }

    /// Adds `svg` drawn inside `bounds` in `color`.
    pub fn icon(&mut self, bounds: Rect, svg: Svg, color: Rgba) {
        self.rotated_icon(bounds, svg, color, 0.0);
    }

    /// Adds `svg` rotated around its centre inside `bounds`.
    ///
    /// The clip test allows for the corners a turn swings out past `bounds`.
    pub fn rotated_icon(&mut self, bounds: Rect, svg: Svg, color: Rgba, rotation: f32) {
        if color.is_transparent() || bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return;
        }
        let turned = bounds.outset(bounds.size.width.max(bounds.size.height) / 2.0);
        if !self.is_visible(turned) {
            return;
        }
        let clip = self.clip();
        self.icons.push((
            IconRun {
                bounds,
                svg,
                color,
                rotation,
            },
            clip,
            self.layer,
        ));
    }

    /// Adds `image` stretched over `bounds`.
    pub fn image(&mut self, bounds: Rect, image: Image) {
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 || !self.is_visible(bounds) {
            return;
        }
        let clip = self.clip();
        self.images
            .push((ImageRun { bounds, image }, clip, self.layer));
    }

    /// The quads to draw, each with its clip rectangle and layer.
    pub(crate) fn quads(&self) -> &[(Quad, Rect, Layer)] {
        &self.quads
    }

    /// The text runs to draw, each with its clip rectangle and layer.
    pub(crate) fn texts(&self) -> &[(TextRun, Rect, Layer)] {
        &self.texts
    }

    /// The layers this frame has anything in, lowest first.
    pub(crate) fn layers(&self) -> std::ops::RangeInclusive<u32> {
        0..=self.opened.0
    }

    /// The icons to draw, each with its clip rectangle.
    pub(crate) fn icons(&self) -> &[(IconRun, Rect, Layer)] {
        &self.icons
    }

    /// The pictures to draw, each with its clip rectangle and layer.
    pub(crate) fn images(&self) -> &[(ImageRun, Rect, Layer)] {
        &self.images
    }
}
