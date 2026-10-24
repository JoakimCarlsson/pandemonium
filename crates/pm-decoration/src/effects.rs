//! Opacity, filters, blending and the pointer behaviour of an element.

use pm_primitives::{Number, Rule};

use crate::Blur;

/// How an element's pixels combine with what is beneath them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BlendMode {
    /// The element covers what is beneath.
    #[default]
    Normal,
    /// Darkens by multiplying.
    Multiply,
    /// Lightens by inverting, multiplying and inverting again.
    Screen,
    /// Multiplies or screens depending on the backdrop.
    Overlay,
    /// Keeps the darker channel.
    Darken,
    /// Keeps the lighter channel.
    Lighten,
    /// Absolute difference of the two.
    Difference,
}

/// The pointer shape shown over an element.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Cursor {
    /// The platform's default arrow.
    #[default]
    Default,
    /// A pointing hand.
    Pointer,
    /// An I-beam.
    Text,
    /// Four arrows.
    Move,
    /// A crossed-out circle.
    NotAllowed,
    /// An open hand.
    Grab,
    /// A closed hand.
    Grabbing,
    /// A busy indicator.
    Wait,
    /// A crosshair.
    Crosshair,
    /// A horizontal resize arrow.
    ColResize,
    /// A vertical resize arrow.
    RowResize,
}

/// Whether an element takes pointer input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PointerEvents {
    /// Hit testing sees the element.
    #[default]
    Auto,
    /// Hit testing passes through to what is beneath.
    None,
}

/// Whether the user can select the element's text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UserSelect {
    /// The default for the element's kind.
    #[default]
    Auto,
    /// Not selectable.
    None,
    /// Selectable.
    Text,
    /// Selected as a whole by one click.
    All,
}

/// How an element responds to the pointer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Interactivity {
    /// The pointer shape.
    pub cursor: Cursor,
    /// Whether hit testing sees the element.
    pub pointer_events: PointerEvents,
    /// Whether text inside is selectable.
    pub user_select: UserSelect,
}

/// Post-processing applied to the painted element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectsStyle {
    /// Opacity of the element and its content, 0.0 to 1.0.
    pub opacity: f32,
    /// Gaussian blur radius of the element, in logical pixels.
    pub blur: f32,
    /// Gaussian blur radius of what is behind the element, in logical pixels.
    pub backdrop_blur: f32,
    /// Brightness multiplier; 1.0 leaves it unchanged.
    pub brightness: f32,
    /// Contrast multiplier; 1.0 leaves it unchanged.
    pub contrast: f32,
    /// Saturation multiplier; 1.0 leaves it unchanged.
    pub saturate: f32,
    /// How far towards grey, 0.0 to 1.0.
    pub grayscale: f32,
    /// Hue rotation in degrees.
    pub hue_rotate: f32,
    /// How the element blends with what is beneath.
    pub blend_mode: BlendMode,
    /// Pointer behaviour.
    pub interactivity: Interactivity,
}

impl Default for EffectsStyle {
    /// Opaque, unfiltered, normally blended.
    fn default() -> Self {
        Self {
            opacity: 1.0,
            blur: 0.0,
            backdrop_blur: 0.0,
            brightness: 1.0,
            contrast: 1.0,
            saturate: 1.0,
            grayscale: 0.0,
            hue_rotate: 0.0,
            blend_mode: BlendMode::Normal,
            interactivity: Interactivity::default(),
        }
    }
}

/// One declaration about effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectsRule {
    /// Opacity from 0.0 to 1.0.
    Opacity(f32),
    /// Blur radius in logical pixels.
    Blur(f32),
    /// Backdrop blur radius in logical pixels.
    BackdropBlur(f32),
    /// Brightness multiplier.
    Brightness(f32),
    /// Contrast multiplier.
    Contrast(f32),
    /// Saturation multiplier.
    Saturate(f32),
    /// Grayscale amount from 0.0 to 1.0.
    Grayscale(f32),
    /// Hue rotation in degrees.
    HueRotate(f32),
    /// Blend mode.
    Blend(BlendMode),
    /// Pointer shape.
    Cursor(Cursor),
    /// Hit-test behaviour.
    PointerEvents(PointerEvents),
    /// Text selection behaviour.
    UserSelect(UserSelect),
}

impl Rule for EffectsRule {
    type Target = EffectsStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut EffectsStyle) {
        match self {
            Self::Opacity(value) => target.opacity = value,
            Self::Blur(pixels) => target.blur = pixels,
            Self::BackdropBlur(pixels) => target.backdrop_blur = pixels,
            Self::Brightness(value) => target.brightness = value,
            Self::Contrast(value) => target.contrast = value,
            Self::Saturate(value) => target.saturate = value,
            Self::Grayscale(value) => target.grayscale = value,
            Self::HueRotate(degrees) => target.hue_rotate = degrees,
            Self::Blend(mode) => target.blend_mode = mode,
            Self::Cursor(cursor) => target.interactivity.cursor = cursor,
            Self::PointerEvents(events) => target.interactivity.pointer_events = events,
            Self::UserSelect(select) => target.interactivity.user_select = select,
        }
    }
}

impl Number for Blur {
    /// The blur radius in logical pixels.
    fn value(self) -> f32 {
        self.pixels()
    }
}
