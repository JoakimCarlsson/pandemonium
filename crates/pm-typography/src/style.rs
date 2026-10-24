//! The typography slice of a style.

use pm_color::Color;

use crate::{
    FontFamily, FontSlant, TextAlign, TextDecoration, TextOverflow, TextTransform, TextWrap,
    WhiteSpace,
};

/// The height of a line of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// A multiple of the font size.
    Ratio(f32),
    /// An exact number of logical pixels.
    Px(f32),
}

impl LineHeight {
    /// The height in logical pixels for text of `font_size`.
    pub fn resolve(self, font_size: f32) -> f32 {
        match self {
            Self::Ratio(ratio) => ratio * font_size,
            Self::Px(pixels) => pixels,
        }
    }
}

/// Everything a text shaper reads about one run of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypographyStyle {
    /// The typeface family.
    pub family: FontFamily,
    /// The font size in logical pixels.
    pub size: f32,
    /// The line height.
    pub line_height: LineHeight,
    /// The weight from 100 to 900.
    pub weight: u16,
    /// Upright or leaning.
    pub slant: FontSlant,
    /// Extra space between letters, in multiples of the font size.
    pub tracking: f32,
    /// Horizontal alignment of lines.
    pub align: TextAlign,
    /// The text colour; inherited from the parent when unset.
    pub color: Option<Color>,
    /// Lines drawn along the text.
    pub decoration: TextDecoration,
    /// Case mapping.
    pub transform: TextTransform,
    /// White-space handling.
    pub white_space: WhiteSpace,
    /// What stands in for text that does not fit.
    pub overflow: TextOverflow,
    /// Line breaking strategy.
    pub wrap: TextWrap,
    /// Cut the text after this many lines.
    pub line_clamp: Option<u16>,
    /// Indent of the first line in logical pixels.
    pub indent: f32,
}

impl TypographyStyle {
    /// Sixteen-pixel upright sans-serif at regular weight on a line one and a
    /// half times as tall.
    pub const BASE: Self = Self {
        family: FontFamily::Sans,
        size: 16.0,
        line_height: LineHeight::Ratio(1.5),
        weight: 400,
        slant: FontSlant::Normal,
        tracking: 0.0,
        align: TextAlign::Start,
        color: None,
        decoration: TextDecoration::NONE,
        transform: TextTransform::None,
        white_space: WhiteSpace::Normal,
        overflow: TextOverflow::Clip,
        wrap: TextWrap::Wrap,
        line_clamp: None,
        indent: 0.0,
    };

    /// Regular upright sans-serif at `size` logical pixels, on a line 1.4
    /// times as tall.
    pub const fn sized(size: f32) -> Self {
        Self {
            size,
            line_height: LineHeight::Ratio(1.4),
            ..Self::BASE
        }
    }

    /// This style shaped in the fixed-width family.
    pub const fn mono(mut self) -> Self {
        self.family = FontFamily::Mono;
        self
    }

    /// This style at `weight` on the 100 to 900 scale.
    pub const fn weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    /// This style leaning.
    pub const fn italic(mut self) -> Self {
        self.slant = FontSlant::Italic;
        self
    }

    /// This style with `pixels` between baselines.
    pub const fn leading_px(mut self, pixels: f32) -> Self {
        self.line_height = LineHeight::Px(pixels);
        self
    }

    /// The distance between baselines in logical pixels.
    pub fn line_height_px(&self) -> f32 {
        self.line_height.resolve(self.size)
    }
}

impl Default for TypographyStyle {
    /// [`TypographyStyle::BASE`].
    fn default() -> Self {
        Self::BASE
    }
}
