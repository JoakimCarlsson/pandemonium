//! The modifier helpers that take the spacing scale, the colour roles and
//! the corner radii. Rows and columns are rustle's [`rustle::row_arranged`],
//! [`rustle::column_arranged`] and [`rustle::flow_row_with`], sized with
//! rustle's own `weight`, `fill_max_width`, `width_in` and intrinsic sizes.
//!
//! Modifier order matters: what comes first is outside. Write a box's
//! modifier in this order, from the outside in: outer space (`pad` before
//! anything else), size and weight, shadow, background, border, clip,
//! padding, and last the input modifiers that should cover the padded area.

use rustle::Modifier;

use crate::kit::theme::{self, Radius, Tone, sp};

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

/// Modifier helpers that count in the spacing scale and paint in roles and radii.
///
/// Each returns the rustle [`Modifier`] it was called on with one element
/// added, so they chain with rustle's own methods.
pub trait Styled: Sized {
    /// Padding of `steps` on every side: `p(steps)`.
    fn pad(self, steps: impl Into<f64>) -> Self;
    /// Padding of `steps` left and right: `px(steps)`.
    fn pad_x(self, steps: impl Into<f64>) -> Self;
    /// Padding of `steps` top and bottom: `py(steps)`.
    fn pad_y(self, steps: impl Into<f64>) -> Self;
    /// Padding of `steps` on one side: `pt`, `pr`, `pb`, `pl`.
    fn pad_side(self, side: Side, steps: impl Into<f64>) -> Self;
    /// `steps` wide: `w(steps)`.
    fn w(self, steps: impl Into<f64>) -> Self;
    /// `steps` tall: `h(steps)`.
    fn h(self, steps: impl Into<f64>) -> Self;
    /// `steps` wide and tall: `size(steps)`.
    fn square(self, steps: impl Into<f64>) -> Self;
    /// At least `steps` tall: `min_h(steps)`.
    fn min_h(self, steps: impl Into<f64>) -> Self;
    /// At most `pixels` wide: `max_w_px(pixels)`.
    fn max_w_px(self, pixels: f32) -> Self;
    /// At most `pixels` tall: `max_h_px(pixels)`.
    fn max_h_px(self, pixels: f32) -> Self;
    /// Filled with `tone`: `bg(tone)`.
    fn fill(self, tone: impl Into<Tone>) -> Self;
    /// Filled with `tone` inside corners rounded by `radius`: `bg(tone)` with `rounded(radius)`.
    fn fill_rounded(self, tone: impl Into<Tone>, radius: Radius) -> Self;
    /// A one pixel outline in `tone` inside corners rounded by `radius`, which takes a pixel of room on every side inside the box and is drawn under what the box holds, as a CSS border is: `border()`, `border_color(tone)`.
    fn stroke(self, tone: impl Into<Tone>, radius: Radius) -> Self;
    /// A line `pixels` thick along one side in `tone`, taking that much room on that side inside the box as a CSS border does: `border_t()`, `border_b()`, `border_l()`, `border_r()`.
    fn stroke_side(self, side: Side, pixels: f32, tone: impl Into<Tone>) -> Self;
    /// Everything inside kept inside corners rounded by `radius`: `masked()`; with [`Radius::None`] the rectangle: `overflow_hidden()`.
    fn clip_to(self, radius: Radius) -> Self;
    /// Drawn at `percent` of its opacity: `opacity(percent)`.
    fn opacity(self, percent: f32) -> Self;
    /// The short shadow of a compact surface: `shadow_md()`.
    fn shadow_md(self, radius: Radius) -> Self;
    /// The tall, soft shadow of a floating surface: `shadow_layers(&FLOATING_SHADOW)`.
    fn shadow_floating(self, radius: Radius) -> Self;
    /// Shadows drawn as CSS box shadows are: each `(x, y, blur, spread, alpha)` in black.
    fn shadows(self, layers: &'static [(f32, f32, f32, f32, f32)], radius: Radius) -> Self;
    /// Placed `x` and `y` logical pixels from the top left of the [`layer`] it is in: `absolute()`, `left_px(x)`, `top_px(y)`.
    fn at(self, x: f32, y: f32) -> Self;
}

impl Styled for Modifier {
    fn pad(self, steps: impl Into<f64>) -> Self {
        self.padding(sp(steps))
    }

    fn pad_x(self, steps: impl Into<f64>) -> Self {
        self.padding_xy(sp(steps), 0.0)
    }

    fn pad_y(self, steps: impl Into<f64>) -> Self {
        self.padding_xy(0.0, sp(steps))
    }

    fn pad_side(self, side: Side, steps: impl Into<f64>) -> Self {
        let pixels = sp(steps);
        match side {
            Side::Top => self.padding_each(0.0, pixels, 0.0, 0.0),
            Side::Right => self.padding_each(0.0, 0.0, pixels, 0.0),
            Side::Bottom => self.padding_each(0.0, 0.0, 0.0, pixels),
            Side::Left => self.padding_each(pixels, 0.0, 0.0, 0.0),
        }
    }

    fn w(self, steps: impl Into<f64>) -> Self {
        self.width(sp(steps))
    }

    fn h(self, steps: impl Into<f64>) -> Self {
        self.height(sp(steps))
    }

    fn square(self, steps: impl Into<f64>) -> Self {
        let pixels = sp(steps);
        self.size(pixels, pixels)
    }

    fn min_h(self, steps: impl Into<f64>) -> Self {
        self.height_in(sp(steps), f32::INFINITY)
    }

    fn max_w_px(self, pixels: f32) -> Self {
        self.width_in(0.0, pixels)
    }

    fn max_h_px(self, pixels: f32) -> Self {
        self.height_in(0.0, pixels)
    }

    fn fill(self, tone: impl Into<Tone>) -> Self {
        self.background(theme::color(tone))
    }

    fn fill_rounded(self, tone: impl Into<Tone>, radius: Radius) -> Self {
        self.background_rounded(theme::color(tone), radius.px())
    }

    fn stroke(self, tone: impl Into<Tone>, radius: Radius) -> Self {
        let color = theme::color(tone);
        let corner = radius.px();
        self.draw_behind(move |canvas| {
            let (width, height) = (canvas.width(), canvas.height());
            let corner = corner.min(width.min(height) / 2.0);
            canvas.stroke_rect(0.0, 0.0, width, height, corner, 1.0, color);
        })
        .padding(1.0)
    }

    fn stroke_side(self, side: Side, pixels: f32, tone: impl Into<Tone>) -> Self {
        let color = theme::color(tone);
        let drawn = self.draw_behind(move |canvas| {
            let (width, height) = (canvas.width(), canvas.height());
            match side {
                Side::Top => canvas.rect(0.0, 0.0, width, pixels, color),
                Side::Right => canvas.rect(width - pixels, 0.0, pixels, height, color),
                Side::Bottom => canvas.rect(0.0, height - pixels, width, pixels, color),
                Side::Left => canvas.rect(0.0, 0.0, pixels, height, color),
            }
        });
        match side {
            Side::Top => drawn.padding_each(0.0, pixels, 0.0, 0.0),
            Side::Right => drawn.padding_each(0.0, 0.0, pixels, 0.0),
            Side::Bottom => drawn.padding_each(0.0, 0.0, 0.0, pixels),
            Side::Left => drawn.padding_each(pixels, 0.0, 0.0, 0.0),
        }
    }

    fn clip_to(self, radius: Radius) -> Self {
        match radius {
            Radius::None => self.clip(),
            radius => self.clip_rounded(radius.px()),
        }
    }

    fn opacity(self, percent: f32) -> Self {
        self.alpha(percent / 100.0)
    }

    fn shadow_md(self, radius: Radius) -> Self {
        self.shadows(&MEDIUM_SHADOW, radius)
    }

    fn shadow_floating(self, radius: Radius) -> Self {
        self.shadows(&FLOATING_SHADOW, radius)
    }

    fn shadows(self, layers: &'static [(f32, f32, f32, f32, f32)], radius: Radius) -> Self {
        let corner = radius.px();
        self.draw_behind(move |canvas| {
            let (width, height) = (canvas.width(), canvas.height());
            for (x, y, blur, spread, alpha) in layers {
                let shade = rustle::Color::rgba(0, 0, 0, (alpha * 255.0).round() as u8);
                canvas.shadow(
                    x - spread,
                    y - spread,
                    width + 2.0 * spread,
                    height + 2.0 * spread,
                    (corner + spread).max(0.0),
                    blur * SHADOW_SOFTNESS,
                    shade,
                );
            }
        })
    }

    fn at(self, x: f32, y: f32) -> Self {
        self.offset(x, y)
    }
}

/// How much farther a shadow fades than its blur says, as a share of the blur:
/// a CSS box shadow fades over about twice its blur radius, which is softer
/// than a Gaussian of half the blur as its deviation.
pub const SHADOW_SOFTNESS: f32 = 1.3;

/// The two layers of the short shadow under a compact surface, as `(x, y, blur, spread, alpha)`.
const MEDIUM_SHADOW: [(f32, f32, f32, f32, f32); 2] =
    [(0.0, 4.0, 6.0, -1.0, 0.1), (0.0, 2.0, 4.0, -2.0, 0.1)];

/// The tall, soft shadow under a floating surface, as `(x, y, blur, spread, alpha)`.
const FLOATING_SHADOW: [(f32, f32, f32, f32, f32); 1] = [(0.0, 12.0, 40.0, 0.0, 0.35)];
