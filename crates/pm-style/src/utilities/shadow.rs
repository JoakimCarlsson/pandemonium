//! Box shadows, rings, drop shadows and text shadows.

use pm_color::Color;
use pm_decoration::{DropShadow, InsetShadow, Shadow, ShadowRule, TextShadow};
use pm_primitives::Number;

use crate::Utility;

/// A box shadow from a named preset.
pub fn shadow(preset: Shadow) -> Utility {
    ShadowRule::Layers(preset.layers()).into()
}

/// An inset box shadow from a named preset.
pub fn inset_shadow(preset: InsetShadow) -> Utility {
    ShadowRule::InsetLayers(preset.layers()).into()
}

/// A shadow cast by whatever the element paints.
pub fn drop_shadow(preset: DropShadow) -> Utility {
    ShadowRule::DropLayers(preset.layers()).into()
}

/// A shadow cast by glyphs.
pub fn text_shadow(preset: TextShadow) -> Utility {
    ShadowRule::TextLayers(preset.layers()).into()
}

/// The colour every shadow layer is drawn in.
pub fn shadow_color(color: impl Into<Color>) -> Utility {
    ShadowRule::Color(color.into()).into()
}

/// A ring of `pixels` thickness around the box.
pub fn ring(pixels: impl Number) -> Utility {
    ShadowRule::Ring(pixels.value()).into()
}

/// A ring of `pixels` thickness inside the box.
pub fn inset_ring(pixels: impl Number) -> Utility {
    ShadowRule::InsetRing(pixels.value()).into()
}

/// The ring colour.
pub fn ring_color(color: impl Into<Color>) -> Utility {
    ShadowRule::RingColor(color.into()).into()
}

/// The gap between box and ring, in raw logical pixels.
pub fn ring_offset(pixels: impl Number) -> Utility {
    ShadowRule::RingOffset(pixels.value()).into()
}

/// The colour of the gap between box and ring.
pub fn ring_offset_color(color: impl Into<Color>) -> Utility {
    ShadowRule::RingOffsetColor(color.into()).into()
}

constants! {
    /// The faintest box shadow.
    shadow_2xs = ShadowRule::Layers(Shadow::Xs2.layers());
    /// An extra-small box shadow.
    shadow_xs = ShadowRule::Layers(Shadow::Xs.layers());
    /// A small box shadow.
    shadow_sm = ShadowRule::Layers(Shadow::Sm.layers());
    /// A medium box shadow.
    shadow_md = ShadowRule::Layers(Shadow::Md.layers());
    /// A large box shadow.
    shadow_lg = ShadowRule::Layers(Shadow::Lg.layers());
    /// An extra-large box shadow.
    shadow_xl = ShadowRule::Layers(Shadow::Xl.layers());
    /// The deepest box shadow.
    shadow_2xl = ShadowRule::Layers(Shadow::Xl2.layers());
    /// No box shadow.
    shadow_none = ShadowRule::Layers(&[]);
    /// A one-pixel ring.
    ring_1 = ShadowRule::Ring(1.0);
}
