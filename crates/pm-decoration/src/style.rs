//! The decoration slice of a style, assembled from its parts.

use crate::{Background, Borders, EffectsStyle, ShadowStyle};

/// Everything a painter reads about the box around one element's content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DecorationStyle {
    /// The fill behind the content.
    pub background: Background,
    /// Borders, corner radii and outline.
    pub borders: Borders,
    /// Shadows and ring.
    pub shadows: ShadowStyle,
    /// Opacity, filters, blending and pointer behaviour.
    pub effects: EffectsStyle,
}
