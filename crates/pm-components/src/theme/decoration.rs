//! The drawing values the theme gives boxes that rustle has no modifier for:
//! a line along one side and the app's soft shadows, each a closure for
//! [`Modifier::draw_behind`](rustle::Modifier::draw_behind), as Compose draws
//! them in `drawBehind`.

use rustle::{Canvas, Color, ShadowFalloff};

use super::Radius;

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

/// One layer of a shadow as `(x, y, blur, spread, alpha)` in black, as a CSS box shadow says it.
pub type ShadowLayer = (f32, f32, f32, f32, f32);

/// The two layers of the short shadow under a compact surface.
pub const MEDIUM_SHADOW: [ShadowLayer; 2] =
    [(0.0, 4.0, 6.0, -1.0, 0.1), (0.0, 2.0, 4.0, -2.0, 0.1)];

/// The tall, soft shadow under a floating surface.
pub const FLOATING_SHADOW: [ShadowLayer; 1] = [(0.0, 12.0, 40.0, 0.0, 0.35)];

/// Draws `layers` of shadow behind an area with corners rounded by `radius`,
/// each fading smoothly over [`SHADOW_SOFTNESS`] times its blur.
pub fn shadows(layers: &'static [ShadowLayer], radius: Radius) -> impl Fn(&mut Canvas) + 'static {
    let corner = radius.px();
    move |canvas| {
        let (width, height) = (canvas.width(), canvas.height());
        for (x, y, blur, spread, alpha) in layers {
            let shade = Color::rgba(0, 0, 0, (alpha * 255.0).round() as u8);
            canvas.shadow_with_falloff(
                x - spread,
                y - spread,
                width + 2.0 * spread,
                height + 2.0 * spread,
                (corner + spread).max(0.0),
                blur * SHADOW_SOFTNESS,
                shade,
                ShadowFalloff::SmoothStep,
            );
        }
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
