//! Rows, columns and layers that lay out as the old flex boxes did, and the
//! modifier helpers that take the spacing scale, the colour roles and the
//! corner radii.
//!
//! The old boxes were CSS flex boxes: children stretch across the cross axis
//! unless the box centres them, `flex_1` shares the room left, `gap` puts
//! space between children and `justify_*` spreads what is left. [`Flex`]
//! names those choices and [`row`], [`column`] and [`block`] lay children
//! out by them; [`layer`] stacks children on top of each other, as an
//! absolutely placed child sat over its relative parent.
//!
//! Modifier order matters as it does in Compose: what comes first is
//! outside. Write a box's modifier in this order, which is the CSS box model
//! from the outside in: margin (`pad` before anything else), size,
//! [`Styled::grow`], shadow, background, border, clip, padding, and last the
//! input modifiers that should cover the padded area.

use rustle::{Alignment, Arrangement, BoxAlignment, Modifier};

use crate::kit::theme::{self, Radius, Tone, sp};

/// How a row or column places its children, as a CSS flex box: the gap
/// between them, how the room left along the main axis is spread, and where
/// children sit across it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flex {
    /// The space between neighbours, in logical pixels.
    gap: f32,
    /// How the room left along the main axis is spread.
    main: Arrangement,
    /// Where children sit across the main axis.
    cross: Alignment,
}

impl Default for Flex {
    /// Children packed at the start, no gap, stretched across: `flex()`.
    fn default() -> Self {
        Self {
            gap: 0.0,
            main: Arrangement::Start,
            cross: Alignment::Stretch,
        }
    }
}

impl Flex {
    /// Children packed at the start, no gap, stretched across: `flex()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `steps` of the spacing scale between neighbours: `gap(steps)`.
    pub fn gap(mut self, steps: impl Into<f64>) -> Self {
        self.gap = sp(steps);
        self
    }

    /// `pixels` logical pixels between neighbours.
    pub fn gap_px(mut self, pixels: f32) -> Self {
        self.gap = pixels;
        self
    }

    /// Children centred across the main axis: `items_center()`.
    pub fn center(mut self) -> Self {
        self.cross = Alignment::Center;
        self
    }

    /// Children at the start of the cross axis, as tall or wide as they are: `items_start()`.
    pub fn start(mut self) -> Self {
        self.cross = Alignment::Start;
        self
    }

    /// Children at the end of the cross axis: `items_end()`.
    pub fn end(mut self) -> Self {
        self.cross = Alignment::End;
        self
    }

    /// Children stretched across the main axis, the default: `items_stretch()`.
    pub fn stretch(mut self) -> Self {
        self.cross = Alignment::Stretch;
        self
    }

    /// The room left along the main axis between the children, the first at the start and the last at the end: `justify_between()`.
    pub fn between(mut self) -> Self {
        self.main = Arrangement::SpaceBetween;
        self
    }

    /// The children centred along the main axis: `justify_center()`.
    pub fn middle(mut self) -> Self {
        self.main = Arrangement::Center;
        self
    }

    /// The children at the end of the main axis: `justify_end()`.
    pub fn trailing(mut self) -> Self {
        self.main = Arrangement::End;
        self
    }

    /// The room left spread around the children: `justify_around()`.
    pub fn around(mut self) -> Self {
        self.main = Arrangement::SpaceAround;
        self
    }

    /// The room left spread evenly between and around the children: `justify_evenly()`.
    pub fn evenly(mut self) -> Self {
        self.main = Arrangement::SpaceEvenly;
        self
    }
}

/// Emits a row laid out by `flex`: `node().styles([flex(), ...])`.
#[track_caller]
pub fn row(modifier: Modifier, flex: Flex, content: impl FnOnce()) {
    rustle::row_spaced(modifier, flex.gap, flex.main, flex.cross, content);
}

/// Emits a column laid out by `flex`: `node().styles([flex(), flex_col(), ...])`.
#[track_caller]
pub fn column(modifier: Modifier, flex: Flex, content: impl FnOnce()) {
    rustle::column_spaced(modifier, flex.gap, flex.main, flex.cross, content);
}

/// Emits a box whose children are stacked down it, each as wide as the box:
/// a `node()` with no `flex()`, which was a CSS block.
#[track_caller]
pub fn block(modifier: Modifier, content: impl FnOnce()) {
    rustle::column_spaced(
        modifier,
        0.0,
        Arrangement::Start,
        Alignment::Stretch,
        content,
    );
}

/// Emits a box whose children lie on top of each other, each at the top
/// left unless its modifier aligns it: a `relative()` box with `absolute()`
/// children. The box is as large as its largest child.
#[track_caller]
pub fn layer(modifier: Modifier, content: impl FnOnce()) {
    rustle::stack_with(modifier, content);
}

/// Emits a row whose children wrap onto further lines when they do not fit:
/// `flex_wrap()`, with `gap` steps between neighbours and between lines.
#[track_caller]
pub fn wrapping(modifier: Modifier, gap: impl Into<f64> + Copy, content: impl FnOnce()) {
    rustle::flow_row_with(
        modifier,
        rustle::FlowOptions::new(sp(gap), sp(gap)),
        content,
    );
}

/// Emits an empty box that takes the room left in its row or column: `spacer()` with `flex_1()`.
#[track_caller]
pub fn grow() {
    rustle::spacer(Modifier::new().weight(1.0));
}

/// Emits an empty box sized by `modifier`, such as a fixed gap.
#[track_caller]
pub fn gap_box(modifier: Modifier) {
    rustle::spacer(modifier);
}

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
    /// Takes the room left in its row or column: `flex_1()`. Text inside it is
    /// cut short at its edge, which `min_w_px(0)` was needed for before.
    fn grow(self) -> Self;
    /// Filled with `tone`: `bg(tone)`.
    fn fill(self, tone: impl Into<Tone>) -> Self;
    /// Filled with `tone` inside corners rounded by `radius`: `bg(tone)` with `rounded(radius)`.
    fn fill_rounded(self, tone: impl Into<Tone>, radius: Radius) -> Self;
    /// A one pixel outline in `tone` inside corners rounded by `radius`: `border()`, `border_color(tone)`.
    fn stroke(self, tone: impl Into<Tone>, radius: Radius) -> Self;
    /// A line `pixels` thick along one side in `tone`: `border_t()`, `border_b()`, `border_l()`, `border_r()`.
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
    /// Placed at `alignment` in the [`layer`] it is in, or across its row or column: `absolute()` with insets of 0, `self_start()`, `self_end()`.
    fn placed(self, alignment: BoxAlignment) -> Self;
    /// Exactly as large as the [`layer`] it is in, which its other children
    /// size, without counting toward that size: `absolute()` with `inset(0)`.
    fn cover(self) -> Self;
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

    fn grow(self) -> Self {
        self.weight(1.0)
    }

    fn fill(self, tone: impl Into<Tone>) -> Self {
        self.background(theme::color(tone))
    }

    fn fill_rounded(self, tone: impl Into<Tone>, radius: Radius) -> Self {
        self.background_rounded(theme::color(tone), radius.px())
    }

    fn stroke(self, tone: impl Into<Tone>, radius: Radius) -> Self {
        self.border(1.0, theme::color(tone), radius.px())
    }

    fn stroke_side(self, side: Side, pixels: f32, tone: impl Into<Tone>) -> Self {
        let color = theme::color(tone);
        self.draw_behind(move |canvas| {
            let (width, height) = (canvas.width(), canvas.height());
            match side {
                Side::Top => canvas.rect(0.0, 0.0, width, pixels, color),
                Side::Right => canvas.rect(width - pixels, 0.0, pixels, height, color),
                Side::Bottom => canvas.rect(0.0, height - pixels, width, pixels, color),
                Side::Left => canvas.rect(0.0, 0.0, pixels, height, color),
            }
        })
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
                    *blur,
                    shade,
                );
            }
        })
    }

    fn at(self, x: f32, y: f32) -> Self {
        self.offset(x, y)
    }

    fn placed(self, alignment: BoxAlignment) -> Self {
        self.align(alignment)
    }

    fn cover(self) -> Self {
        self.match_parent_size()
    }
}

/// The two layers of the short shadow under a compact surface, as `(x, y, blur, spread, alpha)`.
const MEDIUM_SHADOW: [(f32, f32, f32, f32, f32); 2] =
    [(0.0, 4.0, 6.0, -1.0, 0.1), (0.0, 2.0, 4.0, -2.0, 0.1)];

/// The tall, soft shadow under a floating surface, as `(x, y, blur, spread, alpha)`.
const FLOATING_SHADOW: [(f32, f32, f32, f32, f32); 1] = [(0.0, 12.0, 40.0, 0.0, 0.35)];
