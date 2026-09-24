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
mod emphasis;
mod radii;
mod sizes;
mod syntax;
mod terminal;
mod text;

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

impl Theme {
    /// A theme with every colour left black and every other token at its
    /// default: what a theme file is painted over when it builds on nothing.
    pub fn unpainted(appearance: Appearance) -> Self {
        Self {
            name: "",
            appearance,
            colors: Colors::default(),
            syntax: Syntax::default(),
            terminal: Terminal::default(),
            text: TextScale::DEFAULT,
            size: Sizes::DEFAULT,
            radius: Radii::DEFAULT,
            emphasis: Emphasis::DEFAULT,
        }
    }

    /// This theme with the grid a file is edited in scaled by `factor`.
    ///
    /// Zooming an editor is zooming its text and nothing else: the bars, the
    /// tabs and the sidebars are the window's furniture and stay the size
    /// the reader set them at.
    pub fn zoomed(mut self, factor: f32) -> Self {
        let ratio = self.text.code.line_height / self.text.code.size;
        self.text.code.size *= factor;
        self.text.code.line_height = (self.text.code.size * ratio).round();
        self
    }
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
