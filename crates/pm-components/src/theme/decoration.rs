//! The drawing values the theme gives boxes: the app's soft shadows as rustle
//! [`Shadow`] layers cast with [`Modifier::drop_shadow_with`], and the outline
//! and raised panel ([`Frame`]) that cards, lists and popups are set on.

use rustle::{Modifier, Offset, Shadow};

use super::{Radius, Role, color};

/// How much farther a shadow fades than its blur says, as a share of the blur:
/// a CSS box shadow fades over about twice its blur radius, which is softer
/// than a Gaussian of half the blur as its deviation.
pub const SHADOW_SOFTNESS: f32 = 1.3;

/// The black shadow a CSS box shadow of `x`, `y`, `blur`, `spread` and `alpha`
/// casts, as a rustle [`Shadow`] blurred over [`SHADOW_SOFTNESS`] times its blur.
pub const fn box_shadow(x: f32, y: f32, blur: f32, spread: f32, alpha: f32) -> Shadow {
    Shadow::new(blur * SHADOW_SOFTNESS)
        .with_offset(Offset::new(x, y))
        .with_spread(spread)
        .with_alpha(alpha)
}

/// The two layers of the short shadow under a compact surface.
pub const MEDIUM_SHADOW: [Shadow; 2] = [
    box_shadow(0.0, 4.0, 6.0, -1.0, 0.1),
    box_shadow(0.0, 2.0, 4.0, -2.0, 0.1),
];

/// The tall, soft shadow under a floating surface.
pub const FLOATING_SHADOW: [Shadow; 1] = [box_shadow(0.0, 12.0, 40.0, 0.0, 0.35)];

/// Casting a shadow of several layers behind a box.
pub trait ShadowLayers {
    /// This modifier with every one of `layers` cast behind the area, its
    /// corners rounded by `corner` logical pixels, the first layer lowest: one
    /// [`Modifier::drop_shadow_with`] per layer.
    fn shadow_layers(self, layers: &[Shadow], corner: f32) -> Self;
}

impl ShadowLayers for Modifier {
    /// Chains one [`Modifier::drop_shadow_with`] per layer of `layers`, with `corner` as their corner radius.
    fn shadow_layers(self, layers: &[Shadow], corner: f32) -> Self {
        layers.iter().fold(self, |modifier, layer| {
            modifier.drop_shadow_with(corner, *layer)
        })
    }
}

/// The app's frames for a box, chained onto a [`Modifier`] like its own
/// methods: a one pixel outline and the raised panel that cards and popups
/// sit on.
pub trait Frame {
    /// Outlines the box one pixel wide in the border role, its corners as
    /// round as `radius`, and keeps what it holds inside the outline.
    fn outlined(self, radius: Radius) -> Self;

    /// Fills the box with the surface role and outlines it, both with corners
    /// as round as `radius`: the panel cards, lists and popups are set on.
    fn panel(self, radius: Radius) -> Self;
}

impl Frame for Modifier {
    /// Adds a one pixel border-role outline and pads the box by one pixel.
    fn outlined(self, radius: Radius) -> Self {
        self.border(1.0, color(Role::Border), radius.px())
            .padding(1.0)
    }

    /// Adds a rounded surface background, then [`Frame::outlined`].
    fn panel(self, radius: Radius) -> Self {
        self.background_rounded(color(Role::Surface), radius.px())
            .outlined(radius)
    }
}
