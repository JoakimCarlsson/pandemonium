//! Design tokens: the named colours, type scale and radii every element reads.
//!
//! Tokens are values, not lookups into a global. A window owns one [`Theme`]
//! and hands it to the layout and paint passes, so swapping the theme is a
//! swap of one struct and the next frame is drawn in it.

use pm_gfx::{FontStyle, Rgba};

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

/// One resolved theme: the tokens a frame is drawn from.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Whether this is the light or the dark theme.
    pub appearance: Appearance,
    /// The semantic colours.
    pub colors: Colors,
    /// The type scale.
    pub text: TextScale,
    /// The corner radii.
    pub radius: Radii,
}

impl Theme {
    /// The dark theme, which is the one the editor starts in.
    pub fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            colors: Colors {
                background: Rgba::hex(0x0b0b0d),
                surface: Rgba::hex(0x14141a),
                surface_hover: Rgba::hex(0x1d1d25),
                surface_active: Rgba::hex(0x24242e),
                surface_selected: Rgba::hex(0x1f2130),
                border: Rgba::hex(0x2a2a33),
                border_variant: Rgba::hex(0x1e1e26),
                border_focused: Rgba::hex(0x6b7cff),
                border_selected: Rgba::hex(0x8a7cff),
                text: Rgba::hex(0xe8e8ef),
                text_muted: Rgba::hex(0x9b9baa),
                text_subtle: Rgba::hex(0x6c6c7a),
                text_on_accent: Rgba::hex(0x0b0b0d),
                accent: Rgba::hex(0xb9a7ff),
                accent_hover: Rgba::hex(0xc9bbff),
                accent_active: Rgba::hex(0xa78fff),
                success: Rgba::hex(0x5fd08a),
                warning: Rgba::hex(0xe8b34a),
                danger: Rgba::hex(0xf2685f),
            },
            text: TextScale::DEFAULT,
            radius: Radii::DEFAULT,
        }
    }

    /// The light theme.
    pub fn light() -> Self {
        Self {
            appearance: Appearance::Light,
            colors: Colors {
                background: Rgba::hex(0xfbfbfd),
                surface: Rgba::hex(0xffffff),
                surface_hover: Rgba::hex(0xf1f1f5),
                surface_active: Rgba::hex(0xe7e7ee),
                surface_selected: Rgba::hex(0xeeecff),
                border: Rgba::hex(0xdedee6),
                border_variant: Rgba::hex(0xececef),
                border_focused: Rgba::hex(0x4b5bd6),
                border_selected: Rgba::hex(0x6d5ae0),
                text: Rgba::hex(0x1b1b22),
                text_muted: Rgba::hex(0x5c5c6b),
                text_subtle: Rgba::hex(0x8d8d9c),
                text_on_accent: Rgba::hex(0xffffff),
                accent: Rgba::hex(0x5b45d6),
                accent_hover: Rgba::hex(0x6d5ae0),
                accent_active: Rgba::hex(0x4a37b8),
                success: Rgba::hex(0x2f9e5f),
                warning: Rgba::hex(0xb37a12),
                danger: Rgba::hex(0xc9453c),
            },
            text: TextScale::DEFAULT,
            radius: Radii::DEFAULT,
        }
    }

    /// The theme for `appearance`.
    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self::dark(),
            Appearance::Light => Self::light(),
        }
    }
}
