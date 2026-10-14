//! What a frame is made of: a list of rounded, bordered, transformed quads.

use std::sync::Arc;

use pm_color::Srgba;
use pm_text::Run;

use crate::geometry::{Affine, Rect};

/// One rounded rectangle, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// Where the rectangle is, before its transform.
    pub bounds: Rect,
    /// The fill, sRGB-encoded with straight alpha.
    pub fill: Srgba,
    /// The thickness of the border, drawn inside the bounds.
    pub border_width: f32,
    /// The colour of the border.
    pub border_color: Srgba,
    /// The corner radii, clockwise from the top-left corner.
    pub radii: [f32; 4],
    /// How far the edge is blurred, in logical pixels; zero is a crisp edge.
    pub softness: f32,
    /// The transform applied to the finished rectangle.
    pub transform: Affine,
}

impl Quad {
    /// A square-cornered, crisp rectangle filled with `fill`.
    pub const fn filled(bounds: Rect, fill: Srgba) -> Self {
        Self {
            bounds,
            fill,
            border_width: 0.0,
            border_color: Srgba::TRANSPARENT,
            radii: [0.0; 4],
            softness: 0.0,
            transform: Affine::IDENTITY,
        }
    }

    /// This quad with a border `width` thick in `color`.
    pub const fn border(mut self, width: f32, color: Srgba) -> Self {
        self.border_width = width;
        self.border_color = color;
        self
    }

    /// This quad with the given corner radii, clockwise from the top-left.
    pub const fn radii(mut self, radii: [f32; 4]) -> Self {
        self.radii = radii;
        self
    }

    /// This quad with the same radius at every corner.
    pub const fn radius(self, radius: f32) -> Self {
        self.radii([radius; 4])
    }

    /// This quad with its edge blurred over `softness` logical pixels.
    pub const fn softness(mut self, softness: f32) -> Self {
        self.softness = softness;
        self
    }

    /// This quad drawn through `transform`.
    pub const fn transformed(mut self, transform: Affine) -> Self {
        self.transform = transform;
        self
    }
}

/// One line of text, drawn in one colour.
#[derive(Clone, Debug)]
pub struct Text {
    /// The shaped line.
    pub run: Run,
    /// The left edge of the line, in logical pixels.
    pub x: f32,
    /// The top edge of the line, in logical pixels.
    pub y: f32,
    /// The colour, sRGB-encoded with straight alpha.
    pub color: Srgba,
}

/// An icon, tinted: its artwork drawn into a square.
#[derive(Clone, Debug)]
pub struct Icon {
    /// The artwork.
    pub svg: pm_icon::Svg,
    /// Left edge of the square, in logical pixels.
    pub x: f32,
    /// Top edge of the square, in logical pixels.
    pub y: f32,
    /// The side of the square, in logical pixels.
    pub size: f32,
    /// What the artwork is tinted.
    pub color: Srgba,
    /// How far the square is turned clockwise about its middle, in radians.
    pub rotation: f32,
}

/// The pixels of a picture, decoded and ready to be drawn.
#[derive(Clone, Debug)]
pub struct Picture {
    /// What names the picture; two pictures with one key are the same pixels.
    pub key: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Tightly packed sRGB-encoded RGBA bytes, row by row from the top.
    pub pixels: Arc<Vec<u8>>,
}

impl PartialEq for Picture {
    /// Two pictures are equal when they carry the same key.
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

/// A picture drawn to fill a rectangle, cropped to the rectangle's shape.
#[derive(Clone, Debug)]
pub struct Image {
    /// The pixels.
    pub picture: Picture,
    /// Left edge, in logical pixels.
    pub x: f32,
    /// Top edge, in logical pixels.
    pub y: f32,
    /// Width, in logical pixels.
    pub width: f32,
    /// Height, in logical pixels.
    pub height: f32,
    /// How round the corners are, in logical pixels.
    pub radius: f32,
}

/// One thing a frame draws.
#[derive(Clone, Debug)]
pub enum Primitive {
    /// A rounded rectangle.
    Quad(Quad),
    /// A line of text.
    Text(Text),
    /// A tinted icon.
    Icon(Icon),
    /// A picture in its own colours.
    Image(Image),
    /// Everything after this is drawn only inside the rectangle, or
    /// anywhere when there is none.
    Clip(Option<Rect>),
}

/// The primitives of one frame, in the order they are drawn: later ones cover
/// earlier ones.
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    /// The primitives, painter's order.
    primitives: Vec<Primitive>,
}

impl DrawList {
    /// An empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empties the list, keeping its allocation for the next frame.
    pub fn clear(&mut self) {
        self.primitives.clear();
    }

    /// Adds `quad` over everything already in the list.
    pub fn push(&mut self, quad: Quad) {
        self.primitives.push(Primitive::Quad(quad));
    }

    /// Adds `text` over everything already in the list.
    pub fn push_text(&mut self, text: Text) {
        self.primitives.push(Primitive::Text(text));
    }

    /// Limits what is added from now on to `clip`, or lifts the limit.
    pub fn set_clip(&mut self, clip: Option<Rect>) {
        self.primitives.push(Primitive::Clip(clip));
    }

    /// Adds `icon` over everything already in the list.
    pub fn push_icon(&mut self, icon: Icon) {
        self.primitives.push(Primitive::Icon(icon));
    }

    /// Adds `image` over everything already in the list.
    pub fn push_image(&mut self, image: Image) {
        self.primitives.push(Primitive::Image(image));
    }

    /// The primitives, in drawing order.
    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }
}
