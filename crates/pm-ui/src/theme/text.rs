//! The type scale, and the way an element names a step of it.
//!
//! An element never holds a resolved [`FontStyle`]: it holds a [`Font`], which
//! is a step of the scale plus the variations asked for on top of it, and the
//! step is looked up in the theme's own [`TextScale`] during layout and paint.
//! A theme that sets a denser scale is therefore drawn in it, rather than
//! being overruled by a size baked in when the element was built.

use pm_gfx::FontStyle;

/// The proportional steps of the scale, in the Tailwind naming.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextSize {
    /// Badges and the smallest captions.
    Xs,
    /// Secondary labels.
    Sm,
    /// Body text, and the default for anything unstated.
    Base,
    /// Section titles.
    Lg,
    /// Page headings.
    Xl,
    /// The one heading at the top of a screen.
    Xxl,
}

/// A step of the type scale, with the variations an element asks for on top.
#[derive(Clone, Copy, Debug)]
pub struct Font {
    /// Which step of the scale the run is drawn at.
    size: TextSize,
    /// Whether the run is shaped in the monospaced family.
    mono: bool,
    /// The weight to override the step's with, if any.
    weight: Option<u16>,
    /// Whether the run is slanted.
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

    /// Returns this font at `size` instead, keeping every variation.
    pub const fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// Returns this font shaped in the monospaced family.
    pub const fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Returns this font at `weight` on the usual 100..=900 scale.
    pub const fn weight(mut self, weight: u16) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Returns this font slanted.
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Returns this font with `pixels` between baselines.
    pub const fn leading(mut self, pixels: f32) -> Self {
        self.leading = Some(pixels);
        self
    }

    /// The style this font resolves to against `scale`.
    pub fn resolve(&self, scale: &TextScale) -> FontStyle {
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
            style = style.line_height(leading);
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

/// The type scale, in the Tailwind naming: `xs` through `xxl`, plus the two
/// fixed-pitch steps the grids are drawn in.
#[derive(Clone, Copy, Debug)]
pub struct TextScale {
    /// 11px: badges and the smallest captions.
    pub xs: FontStyle,
    /// 12px: secondary labels.
    pub sm: FontStyle,
    /// 14px: body text and the default for anything unstated.
    pub base: FontStyle,
    /// 16px: section titles.
    pub lg: FontStyle,
    /// 20px: page headings.
    pub xl: FontStyle,
    /// 26px: the one heading at the top of a screen.
    pub xxl: FontStyle,
    /// The grid a file is edited in, leaded loosely enough to read prose in.
    pub code: FontStyle,
    /// The grid a terminal is drawn in, leaded as tightly as a shell expects.
    pub terminal: FontStyle,
}

impl TextScale {
    /// The scale every theme uses, tuned for a 1.4 line height.
    pub const DEFAULT: Self = Self {
        xs: FontStyle::new(11.0),
        sm: FontStyle::new(12.0),
        base: FontStyle::new(14.0),
        lg: FontStyle::new(16.0),
        xl: FontStyle::new(20.0),
        xxl: FontStyle::new(26.0),
        code: FontStyle::new(14.0).mono().line_height(21.0),
        terminal: FontStyle::new(14.0).mono(),
    };

    /// The style at `size`, before any of a font's own variations.
    pub const fn step(&self, size: TextSize) -> FontStyle {
        match size {
            TextSize::Xs => self.xs,
            TextSize::Sm => self.sm,
            TextSize::Base => self.base,
            TextSize::Lg => self.lg,
            TextSize::Xl => self.xl,
            TextSize::Xxl => self.xxl,
        }
    }
}
