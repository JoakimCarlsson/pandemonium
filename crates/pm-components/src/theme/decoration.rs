//! The drawing values the theme gives boxes: a line along one side, a closure
//! for [`Modifier::draw_behind`], and the app's soft shadows as rustle
//! [`Shadow`] layers cast with [`Modifier::drop_shadow_with`].

use rustle::{Canvas, Color, Modifier, Offset, Shadow};

/// One side of a box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    /// The top edge.
    Top,
    /// The right edge.
    Right,
    /// The bottom edge.
    Bottom,
    /// The left edge.
    Left,
}

impl Side {
    /// The padding, left, top, right and bottom, that keeps what a box holds
    /// clear of a line `width` thick along this side.
    pub fn padding(self, width: f32) -> (f32, f32, f32, f32) {
        match self {
            Self::Top => (0.0, width, 0.0, 0.0),
            Self::Right => (0.0, 0.0, width, 0.0),
            Self::Bottom => (0.0, 0.0, 0.0, width),
            Self::Left => (width, 0.0, 0.0, 0.0),
        }
    }
}

/// Draws a line `width` thick in `color` along `side` of the area, as a CSS
/// border on one side is drawn; follow it with the padding [`Side::padding`] gives.
pub fn edge(side: Side, width: f32, color: Color) -> impl Fn(&mut Canvas) + 'static {
    move |canvas| {
        let (across, down) = (canvas.width(), canvas.height());
        match side {
            Side::Top => canvas.rect(0.0, 0.0, across, width, color),
            Side::Right => canvas.rect(across - width, 0.0, width, down, color),
            Side::Bottom => canvas.rect(0.0, down - width, across, width, color),
            Side::Left => canvas.rect(0.0, 0.0, width, down, color),
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The padding of a side keeps content clear of a line along that side only.
    #[test]
    fn a_side_pads_only_itself() {
        assert_eq!(Side::Top.padding(2.0), (0.0, 2.0, 0.0, 0.0));
        assert_eq!(Side::Right.padding(2.0), (0.0, 0.0, 2.0, 0.0));
        assert_eq!(Side::Bottom.padding(2.0), (0.0, 0.0, 0.0, 2.0));
        assert_eq!(Side::Left.padding(2.0), (2.0, 0.0, 0.0, 0.0));
    }
}
