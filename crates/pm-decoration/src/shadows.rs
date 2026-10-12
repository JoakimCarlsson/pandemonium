//! Box shadows, rings, drop shadows and text shadows.

use pm_color::Color;
use pm_primitives::Rule;

/// One shadow: an offset, blurred, optionally grown copy of the shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    /// Horizontal offset in logical pixels.
    pub x: f32,
    /// Vertical offset in logical pixels.
    pub y: f32,
    /// Blur radius in logical pixels.
    pub blur: f32,
    /// How far the shape grows before blurring, in logical pixels; negative shrinks it.
    pub spread: f32,
    /// The opacity of the shadow colour, 0.0 to 1.0.
    pub alpha: f32,
    /// Whether the shadow is cast inside the box.
    pub inset: bool,
}

impl ShadowLayer {
    /// A layer from its parts.
    pub const fn new(x: f32, y: f32, blur: f32, spread: f32, alpha: f32, inset: bool) -> Self {
        Self {
            x,
            y,
            blur,
            spread,
            alpha,
            inset,
        }
    }
}

/// A solid band around the box, drawn as a shadow with no blur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    /// The band thickness in logical pixels.
    pub width: f32,
    /// The colour; the accent colour when unset.
    pub color: Option<Color>,
    /// Whether the band is drawn inside the box.
    pub inset: bool,
    /// A gap between the box and the band, filled with the offset colour.
    pub offset: f32,
    /// The colour of the gap.
    pub offset_color: Option<Color>,
}

/// The shadows of one element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShadowStyle {
    /// Layers cast behind the box.
    pub layers: &'static [ShadowLayer],
    /// Layers cast inside the box.
    pub inset_layers: &'static [ShadowLayer],
    /// Layers cast by whatever the box paints, including its content.
    pub drop_layers: &'static [ShadowLayer],
    /// Layers cast by glyphs.
    pub text_layers: &'static [ShadowLayer],
    /// The colour every layer is drawn in; black when unset.
    pub color: Option<Color>,
    /// The ring.
    pub ring: Option<Ring>,
}

/// One declaration about shadows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShadowRule {
    /// Layers cast behind the box.
    Layers(&'static [ShadowLayer]),
    /// Layers cast inside the box.
    InsetLayers(&'static [ShadowLayer]),
    /// Layers cast by whatever the box paints.
    DropLayers(&'static [ShadowLayer]),
    /// Layers cast by glyphs.
    TextLayers(&'static [ShadowLayer]),
    /// The colour of every layer.
    Color(Color),
    /// A ring of the given thickness in logical pixels.
    Ring(f32),
    /// A ring drawn inside the box.
    InsetRing(f32),
    /// The ring colour.
    RingColor(Color),
    /// The gap between box and ring in logical pixels.
    RingOffset(f32),
    /// The colour of that gap.
    RingOffsetColor(Color),
}

impl ShadowStyle {
    /// The ring being declared, starting a one-pixel one if none is.
    fn ring_mut(&mut self) -> &mut Ring {
        self.ring.get_or_insert(Ring {
            width: 1.0,
            color: None,
            inset: false,
            offset: 0.0,
            offset_color: None,
        })
    }
}

impl Rule for ShadowRule {
    type Target = ShadowStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut ShadowStyle) {
        match self {
            Self::Layers(layers) => target.layers = layers,
            Self::InsetLayers(layers) => target.inset_layers = layers,
            Self::DropLayers(layers) => target.drop_layers = layers,
            Self::TextLayers(layers) => target.text_layers = layers,
            Self::Color(color) => target.color = Some(color),
            Self::Ring(width) => {
                let ring = target.ring_mut();
                ring.width = width;
                ring.inset = false;
            }
            Self::InsetRing(width) => {
                let ring = target.ring_mut();
                ring.width = width;
                ring.inset = true;
            }
            Self::RingColor(color) => target.ring_mut().color = Some(color),
            Self::RingOffset(pixels) => target.ring_mut().offset = pixels,
            Self::RingOffsetColor(color) => target.ring_mut().offset_color = Some(color),
        }
    }
}
