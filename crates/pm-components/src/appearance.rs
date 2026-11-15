//! The appearance settings components read while they draw: the interface's
//! font and size, the code font, its size and line height, and how closely
//! rows, padding and gaps are set, each provided through its own composition
//! local by [`crate::app_theme`].

use rustle::{CompositionLocal, FontFamily, TextStyle, Typography};

/// The size the interface was drawn for, in logical pixels.
pub const INTERFACE_BASE: f32 = 13.0;

/// How closely rows, padding and gaps are set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    /// The room the window was drawn for.
    #[default]
    Comfortable,
    /// Less room around everything, so more fits.
    Compact,
}

impl Density {
    /// How much of the comfortable room each step of spacing takes.
    pub fn scale(self) -> f32 {
        match self {
            Self::Comfortable => 1.0,
            Self::Compact => 0.75,
        }
    }

    /// How much of its comfortable height a row takes: half way between the full height and the spacing's [`Self::scale`], so text still fits.
    pub fn row_scale(self) -> f32 {
        (1.0 + self.scale()) / 2.0
    }

    /// `space`, a padding or gap drawn for comfortable density, at this density.
    pub fn space(self, space: f32) -> f32 {
        (space * self.scale()).round()
    }
}

/// The font the interface is set in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterfaceFont {
    /// The family, sans serif unless the settings name one.
    pub family: FontFamily,
    /// How big its text is, in logical pixels.
    pub size: f32,
}

impl Default for InterfaceFont {
    /// The interface's own sans serif at the size it was drawn for.
    fn default() -> Self {
        Self {
            family: FontFamily::SansSerif,
            size: INTERFACE_BASE,
        }
    }
}

impl InterfaceFont {
    /// The interface font in the family named `name`, the shipped one when empty, at `size` pixels.
    pub fn named(name: &str, size: f32) -> Self {
        Self {
            family: family_named(name, FontFamily::SansSerif),
            size,
        }
    }

    /// How much bigger than drawn for the interface's text is.
    pub fn scale(&self) -> f32 {
        self.size / INTERFACE_BASE
    }

    /// `size`, a text size drawn for the base interface size, at this font's size.
    pub fn sized(&self, size: f32) -> f32 {
        (size * self.scale()).round()
    }

    /// The style plain text takes: Rustle's default text style in this family, grown or shrunk by [`Self::scale`].
    pub fn plain(&self) -> TextStyle {
        let base = TextStyle::default();
        TextStyle {
            font_size: (base.font_size * self.scale()).round(),
            line_height: (base.line_height * self.scale()).round(),
            font_family: self.family,
            ..base
        }
    }

    /// The Material type scale rebuilt in this family, each style grown or shrunk by [`Self::scale`].
    pub fn typography(&self) -> Typography {
        let base = Typography::default();
        let set = |style: TextStyle| TextStyle {
            font_size: (style.font_size * self.scale()).round(),
            line_height: (style.line_height * self.scale()).round(),
            font_family: self.family,
            ..style
        };
        Typography {
            display_large: set(base.display_large),
            display_medium: set(base.display_medium),
            display_small: set(base.display_small),
            headline_large: set(base.headline_large),
            headline_medium: set(base.headline_medium),
            headline_small: set(base.headline_small),
            title_large: set(base.title_large),
            title_medium: set(base.title_medium),
            title_small: set(base.title_small),
            body_large: set(base.body_large),
            body_medium: set(base.body_medium),
            body_small: set(base.body_small),
            label_large: set(base.label_large),
            label_medium: set(base.label_medium),
            label_small: set(base.label_small),
        }
    }
}

/// The code size the views were drawn for, in logical pixels.
const CODE_BASE: f32 = 12.0;

/// The line height the views were drawn for, as a multiple of the code size.
const LINE_BASE: f32 = 1.67;

/// The font code is set in: the editor, diffs, the terminal and code blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodeFont {
    /// The family, monospace unless the settings name one.
    pub family: FontFamily,
    /// How big code is, in logical pixels.
    pub size: f32,
    /// How tall each line is, as a multiple of `size`.
    pub line_height: f32,
    /// How big the terminal's cells are, in logical pixels.
    pub terminal_size: f32,
}

impl Default for CodeFont {
    /// Monospace at the size and line height the views were drawn for, with the terminal at 12 pixels.
    fn default() -> Self {
        Self {
            family: FontFamily::Monospace,
            size: CODE_BASE,
            line_height: LINE_BASE,
            terminal_size: 12.0,
        }
    }
}

impl CodeFont {
    /// The code font in the family named `name`, monospace when empty, `size` pixels big with lines `line_height` times as tall, and the terminal's cells `terminal_size` pixels big.
    pub fn named(name: &str, size: f32, line_height: f32, terminal_size: f32) -> Self {
        Self {
            family: family_named(name, FontFamily::Monospace),
            size,
            line_height,
            terminal_size,
        }
    }

    /// The code size `size`, drawn for the default code size, at this font's size.
    pub fn sized(&self, size: f32) -> f32 {
        size * self.size / CODE_BASE
    }

    /// How tall a line of text `size` pixels big is, in whole pixels, for a view drawn with lines `drawn` times as tall at the default line height.
    pub fn line_for(&self, size: f32, drawn: f32) -> f32 {
        (size * drawn * self.line_height / LINE_BASE)
            .round()
            .max(1.0)
    }
}

/// The family named `name`, or `fallback` when the name is empty.
fn family_named(name: &str, fallback: FontFamily) -> FontFamily {
    if name.trim().is_empty() {
        fallback
    } else {
        FontFamily::named(name.trim())
    }
}

/// The density in effect, comfortable until [`crate::app_theme`] provides another.
pub(crate) static LOCAL_DENSITY: CompositionLocal<Density> =
    CompositionLocal::new(Density::default);

/// The interface font in effect.
pub(crate) static LOCAL_INTERFACE_FONT: CompositionLocal<InterfaceFont> =
    CompositionLocal::new(InterfaceFont::default);

/// The code font in effect.
pub(crate) static LOCAL_CODE_FONT: CompositionLocal<CodeFont> =
    CompositionLocal::new(CodeFont::default);

/// The density in effect at this point of the composition.
pub fn density() -> Density {
    LOCAL_DENSITY.current()
}

/// The interface font in effect at this point of the composition.
pub fn interface_font() -> InterfaceFont {
    LOCAL_INTERFACE_FONT.current()
}

/// The code font in effect at this point of the composition.
pub fn code_font() -> CodeFont {
    LOCAL_CODE_FONT.current()
}

/// `height`, a row height drawn for comfortable density at the base interface size, grown with the interface font and shrunk with the density.
pub fn row_height(height: f32) -> f32 {
    (height * interface_font().scale() * density().row_scale()).round()
}

/// `space`, a padding or gap drawn for comfortable density, at the density in effect.
pub fn spacing(space: f32) -> f32 {
    density().space(space)
}
