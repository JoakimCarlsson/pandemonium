//! Design tokens: the named colours, type scale, sizes and radii every element
//! reads.
//!
//! Tokens are values, not lookups into a global. A window owns one [`Theme`]
//! and hands it to the layout and paint passes, so swapping the theme is a
//! swap of one struct and the next frame is drawn in it. An element names the
//! token it wants — the body step of the scale, the height of a control, the
//! wash a selection is drawn at — and resolves it against the theme in hand,
//! never against a value baked in when the element was built.

mod colors;
mod ember;
mod emphasis;
mod fathom;
mod pandemonium;
mod radii;
mod sizes;
mod syntax;
mod terminal;
mod text;
mod verdant;

pub use colors::Colors;
pub use emphasis::Emphasis;
pub use radii::Radii;
pub use sizes::Sizes;
pub use syntax::Syntax;
pub use terminal::Terminal;
pub use text::{Font, TextScale, TextSize};

/// Whether a theme is a light or a dark one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appearance {
    /// Dark surfaces, light text.
    Dark,
    /// Light surfaces, dark text.
    Light,
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
    /// The heights controls and bars are drawn at.
    pub size: Sizes,
    /// The corner radii.
    pub radius: Radii,
    /// The alphas the translucent parts of the window are drawn at.
    pub emphasis: Emphasis,
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
