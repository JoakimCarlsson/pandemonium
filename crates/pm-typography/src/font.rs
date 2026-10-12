//! The way an element names a step of the type scale, and the scale a theme sets.
//!
//! An element never holds a resolved style: it holds a [`Font`], which is a
//! step of the scale plus the variations asked for on top of it, and the step
//! is looked up in the theme's own [`TextScale`] during layout and paint. A
//! theme that sets a denser scale is therefore drawn in it, rather than being
//! overruled by a size baked in when the element was built.

use crate::{TextSize, TypographyStyle};

/// A step of the type scale, with the variations an element asks for on top.
#[derive(Clone, Copy, Debug)]
pub struct Font {
    /// Which step of the scale the run is drawn at.
    size: TextSize,
    /// Whether the run is shaped in the fixed-width family.
    mono: bool,
    /// The weight to override the step's with, if any.
    weight: Option<u16>,
    /// Whether the run is leaning.
    italic: bool,
    /// The leading to override the step's with, in logical pixels.
    leading: Option<f32>,
}

impl Font {
    /// A run at `size`, with the step's own weight, leading and family.
    pub const fn new(size: TextSize) -> Self {
        Self {
            size,
            mono: false,
            weight: None,
            italic: false,
            leading: None,
        }
    }

    /// This font at `size` instead, keeping every variation.
    pub const fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// This font shaped in the fixed-width family.
    pub const fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// This font at `weight` on the 100 to 900 scale.
    pub const fn weight(mut self, weight: u16) -> Self {
        self.weight = Some(weight);
        self
    }

    /// This font leaning.
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// This font with `pixels` between baselines.
    pub const fn leading(mut self, pixels: f32) -> Self {
        self.leading = Some(pixels);
        self
    }

    /// The style this font resolves to against `scale`.
    pub fn resolve(&self, scale: &TextScale) -> TypographyStyle {
        let mut style = scale.step(self.size);
        if self.mono {
            style = style.mono();
        }
        if let Some(weight) = self.weight {
            style = style.weight(weight);
        }
        if self.italic {
            style = style.italic();
        }
        if let Some(leading) = self.leading {
            style = style.leading_px(leading);
        }
        style
    }
}

impl Default for Font {
    /// The body step, unvaried.
    fn default() -> Self {
        Self::new(TextSize::Base)
    }
}

/// The type scale a theme sets: the steps of [`TextSize`] the interface uses,
/// at the sizes the theme gives them, plus the two fixed-pitch faces the grids
/// are drawn in.
#[derive(Clone, Copy, Debug)]
pub struct TextScale {
    /// 11px: badges and the smallest captions.
    pub xs: TypographyStyle,
    /// 12px: secondary labels.
    pub sm: TypographyStyle,
    /// 14px: body text and the default for anything unstated.
    pub base: TypographyStyle,
    /// 16px: section titles.
    pub lg: TypographyStyle,
    /// 20px: page headings.
    pub xl: TypographyStyle,
    /// 26px: the one heading at the top of a screen.
    pub xl2: TypographyStyle,
    /// The grid a file is edited in, leaded loosely enough to read prose in.
    pub code: TypographyStyle,
    /// The grid a terminal is drawn in, leaded as tightly as a shell expects.
    pub terminal: TypographyStyle,
}

impl TextScale {
    /// The scale every theme uses, tuned for a 1.4 line height.
    pub const DEFAULT: Self = Self {
        xs: TypographyStyle::sized(11.0),
        sm: TypographyStyle::sized(12.0),
        base: TypographyStyle::sized(14.0),
        lg: TypographyStyle::sized(16.0),
        xl: TypographyStyle::sized(20.0),
        xl2: TypographyStyle::sized(26.0),
        code: TypographyStyle::sized(14.0).mono().leading_px(21.0),
        terminal: TypographyStyle::sized(14.0).mono(),
    };

    /// The style at `size`, before any of a font's own variations.
    ///
    /// The steps the interface uses carry the theme's own sizes; any other
    /// step of the scale is drawn at its own size.
    pub fn step(&self, size: TextSize) -> TypographyStyle {
        match size {
            TextSize::Xs => self.xs,
            TextSize::Sm => self.sm,
            TextSize::Base => self.base,
            TextSize::Lg => self.lg,
            TextSize::Xl => self.xl,
            TextSize::Xl2 => self.xl2,
            other => TypographyStyle::sized(other.pixels()),
        }
    }
}
