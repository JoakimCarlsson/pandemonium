//! Design tokens: the named colours, type scale and radii every element reads.
//!
//! Tokens are values, not lookups into a global. A window owns one [`Theme`]
//! and hands it to the layout and paint passes, so swapping the theme is a
//! swap of one struct and the next frame is drawn in it.

mod ember;
mod fathom;
mod pandemonium;
mod terminal;
mod verdant;

use pm_gfx::{FontStyle, Rgba};

pub use terminal::Terminal;

/// Whether a theme is a light or a dark one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appearance {
    /// Dark surfaces, light text.
    Dark,
    /// Light surfaces, dark text.
    Light,
}

/// The semantic colours elements are painted in.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    /// The window behind everything.
    pub background: Rgba,
    /// A panel or card sitting on the background.
    pub surface: Rgba,
    /// A surface under the pointer.
    pub surface_hover: Rgba,
    /// A surface being pressed.
    pub surface_active: Rgba,
    /// A surface that is the selected one of a set.
    pub surface_selected: Rgba,
    /// The ordinary divider and outline colour.
    pub border: Rgba,
    /// A quieter divider, for rules inside a panel.
    pub border_variant: Rgba,
    /// The outline of the element holding keyboard focus.
    pub border_focused: Rgba,
    /// The outline of a selected element.
    pub border_selected: Rgba,
    /// Body text.
    pub text: Rgba,
    /// Secondary text: descriptions and captions.
    pub text_muted: Rgba,
    /// Text that is barely there: hints and disabled labels.
    pub text_subtle: Rgba,
    /// Text drawn on top of `accent`.
    pub text_on_accent: Rgba,
    /// The one colour that means "this is the action".
    pub accent: Rgba,
    /// The accent under the pointer.
    pub accent_hover: Rgba,
    /// The accent being pressed.
    pub accent_active: Rgba,
    /// Something finished or is healthy.
    pub success: Rgba,
    /// Something needs attention.
    pub warning: Rgba,
    /// Something failed or is destructive.
    pub danger: Rgba,
}

/// The type scale, in the Tailwind naming: `xs` through `xxl`.
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
    };
}

/// Corner radii, in the Tailwind naming.
#[derive(Clone, Copy, Debug)]
pub struct Radii {
    /// 2px.
    pub sm: f32,
    /// 4px.
    pub md: f32,
    /// 6px.
    pub lg: f32,
    /// 10px.
    pub xl: f32,
    /// Large enough to round any bar into a pill.
    pub full: f32,
}

impl Radii {
    /// The radii every theme uses.
    pub const DEFAULT: Self = Self {
        sm: 2.0,
        md: 4.0,
        lg: 6.0,
        xl: 10.0,
        full: 9999.0,
    };
}

/// The colours code is highlighted in.
#[derive(Clone, Copy, Debug)]
pub struct Syntax {
    /// Keywords and operators.
    pub keyword: Rgba,
    /// String and character literals.
    pub string: Rgba,
    /// Function and method names.
    pub function: Rgba,
    /// Comments and documentation.
    pub comment: Rgba,
    /// Numeric literals.
    pub number: Rgba,
    /// Types, traits and named constants.
    pub type_name: Rgba,
    /// Brackets, delimiters and other punctuation.
    pub punctuation: Rgba,
}

/// One resolved theme: the tokens a frame is drawn from.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// The theme's name, as the theme picker shows it.
    pub name: &'static str,
    /// Whether this is the light or the dark variant.
    pub appearance: Appearance,
    /// The semantic colours.
    pub colors: Colors,
    /// The colours code is highlighted in.
    pub syntax: Syntax,
    /// The colours a terminal grid is drawn in.
    pub terminal: Terminal,
    /// The type scale.
    pub text: TextScale,
    /// The corner radii.
    pub radius: Radii,
}

/// A theme in both appearances: what the theme picker offers as one choice.
///
/// Families are how a light and a dark theme stay one decision. Picking the
/// family is the user's choice; picking the variant is the appearance the
/// desktop or the theme mode asks for.
#[derive(Clone, Copy, Debug)]
pub struct ThemeFamily {
    /// The family's name, as the theme picker shows it.
    pub name: &'static str,
    /// The variant for dark surfaces.
    pub dark: Theme,
    /// The variant for light surfaces.
    pub light: Theme,
}

impl ThemeFamily {
    /// The variant of this family for `appearance`.
    pub const fn variant(&self, appearance: Appearance) -> Theme {
        match appearance {
            Appearance::Dark => self.dark,
            Appearance::Light => self.light,
        }
    }
}

/// Every family the editor ships with, in the order the picker offers them.
pub const FAMILIES: [ThemeFamily; 4] = [
    pandemonium::family(),
    fathom::family(),
    ember::family(),
    verdant::family(),
];

/// The family a first launch starts in, as an index into [`FAMILIES`].
pub const DEFAULT_FAMILY: usize = 0;

/// The family at `index`, or the default one when the index is out of range.
pub const fn family(index: usize) -> ThemeFamily {
    if index < FAMILIES.len() {
        FAMILIES[index]
    } else {
        FAMILIES[DEFAULT_FAMILY]
    }
}
