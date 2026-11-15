//! Theme families: a named pair of palettes, one for each appearance, that
//! a window is painted with. The app ships Default and Dusk; extensions
//! bring more as a colour for each role they name, painted over Default.

use rustle::{Color, ColorScheme, dark_color_scheme, light_color_scheme};

use crate::app_theme::{AppTheme, ExtendedColors};

/// The name of the family the app paints with when none other is chosen.
pub const DEFAULT_FAMILY: &str = "Default";

/// One appearance of a family: a colour for each role the window paints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// The window's ground.
    pub background: Color,
    /// The sidebar's ground.
    pub sidebar: Color,
    /// Cards, bars and panels.
    pub surface: Color,
    /// A hovered row or control.
    pub surface_hover: Color,
    /// A pressed or raised control.
    pub surface_active: Color,
    /// A chosen row.
    pub surface_selected: Color,
    /// Lines between things.
    pub border: Color,
    /// The ring around what has the keyboard.
    pub focused: Color,
    /// Text.
    pub text: Color,
    /// Quieter text and icons.
    pub muted: Color,
    /// The faintest text and marks.
    pub subtle: Color,
    /// The primary action.
    pub accent: Color,
    /// The primary action while hovered.
    pub accent_hover: Color,
    /// Something that went well.
    pub success: Color,
    /// Something that went wrong or destroys.
    pub danger: Color,
    /// Text on `accent`.
    pub on_accent: Color,
    /// Text on `success`.
    pub on_success: Color,
    /// Text on `danger`.
    pub on_danger: Color,
    /// Caution.
    pub warning: Color,
    /// Keywords in code.
    pub syntax_keyword: Color,
    /// Functions in code.
    pub syntax_function: Color,
    /// Types in code.
    pub syntax_type: Color,
    /// Strings in code.
    pub syntax_string: Color,
    /// Constants in code.
    pub syntax_constant: Color,
    /// Comments in code.
    pub syntax_comment: Color,
    /// Fields in code.
    pub syntax_property: Color,
    /// Punctuation in code.
    pub syntax_punctuation: Color,
}

/// A named pair of palettes, one for each appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeFamily {
    /// What the reader calls it.
    pub name: String,
    /// The palette drawn while the window is dark.
    pub dark: Palette,
    /// The palette drawn while the window is light.
    pub light: Palette,
}

/// The colour `hex`, written `0xrrggbb`.
const fn hex(value: u32) -> Color {
    Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// The colour `hex`, written `0xrrggbbaa`.
const fn hexa(value: u32) -> Color {
    Color::rgba(
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    )
}

/// The colour `share` of the way from `from` to `to`.
fn mix(from: Color, to: Color, share: f32) -> Color {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * share).round() as u8;
    Color::rgba(
        channel(from.r, to.r),
        channel(from.g, to.g),
        channel(from.b, to.b),
        channel(from.a, to.a),
    )
}

impl Palette {
    /// Default's dark palette: near-black surfaces under light grey text.
    pub const fn default_dark() -> Self {
        Self {
            background: hex(0x1a1b1c),
            sidebar: hex(0x1a1b1c),
            surface: hex(0x202122),
            surface_hover: hex(0x272829),
            surface_active: hex(0x343536),
            surface_selected: hex(0x2a2b2c),
            border: hex(0x2a2b2c),
            focused: hexa(0x8c8c8ca0),
            text: hex(0xd6d6d6),
            muted: hex(0x9a9a9a),
            subtle: hex(0x707070),
            accent: hex(0xd6d6d6),
            accent_hover: hex(0xffffff),
            success: hex(0x6fb98a),
            danger: hex(0xd9806f),
            on_accent: hex(0x1a1b1c),
            on_success: hex(0x0e1a12),
            on_danger: hex(0x1f0b07),
            warning: hex(0xd7b46a),
            syntax_keyword: hex(0xc49fd8),
            syntax_function: hex(0x8fb8e8),
            syntax_type: hex(0x7fc9be),
            syntax_string: hex(0xa9c98a),
            syntax_constant: hex(0xe0a77a),
            syntax_comment: hex(0x7a7e83),
            syntax_property: hex(0xd8c79a),
            syntax_punctuation: hex(0x8a8c90),
        }
    }

    /// Default's light palette: light grey surfaces under dark grey text.
    pub const fn default_light() -> Self {
        Self {
            background: hex(0xfbfbfb),
            sidebar: hex(0xf4f4f4),
            surface: hex(0xf6f6f7),
            surface_hover: hex(0xe8e8eb),
            surface_active: hex(0xd8d8dc),
            surface_selected: hex(0xe4e6ea),
            border: hex(0xe2e2e6),
            focused: hexa(0x8c8c8ca0),
            text: hex(0x26272a),
            muted: hex(0x5f6165),
            subtle: hex(0x828489),
            accent: hex(0x2a2a2a),
            accent_hover: hex(0x000000),
            success: hex(0x1f8a4c),
            danger: hex(0xc4412c),
            on_accent: hex(0xfbfbfb),
            on_success: hex(0xffffff),
            on_danger: hex(0xffffff),
            warning: hex(0xa66f00),
            syntax_keyword: hex(0x8a3fb0),
            syntax_function: hex(0x2f63b5),
            syntax_type: hex(0x1b7d72),
            syntax_string: hex(0x4f7a26),
            syntax_constant: hex(0xb35a1f),
            syntax_comment: hex(0x82858a),
            syntax_property: hex(0x8a6a12),
            syntax_punctuation: hex(0x6d6f74),
        }
    }

    /// Dusk's dark palette: deep blue-grey surfaces under cool pale text, with a soft blue accent.
    pub const fn dusk_dark() -> Self {
        Self {
            background: hex(0x161a22),
            sidebar: hex(0x141820),
            surface: hex(0x1c212b),
            surface_hover: hex(0x232936),
            surface_active: hex(0x2e3544),
            surface_selected: hex(0x262d3b),
            border: hex(0x262c38),
            focused: hexa(0x7f9cc8a0),
            text: hex(0xd5dbe6),
            muted: hex(0x939cad),
            subtle: hex(0x5f6879),
            accent: hex(0x8fb0e6),
            accent_hover: hex(0xb3cbf2),
            success: hex(0x78c29a),
            danger: hex(0xe08a86),
            on_accent: hex(0x10141b),
            on_success: hex(0x0c1a12),
            on_danger: hex(0x200c0b),
            warning: hex(0xdcbc72),
            syntax_keyword: hex(0xb69cf0),
            syntax_function: hex(0x82b4f2),
            syntax_type: hex(0x6fcfd0),
            syntax_string: hex(0x9fd08f),
            syntax_constant: hex(0xeaa77e),
            syntax_comment: hex(0x657085),
            syntax_property: hex(0xd9c594),
            syntax_punctuation: hex(0x8690a3),
        }
    }

    /// Dusk's light palette: pale blue-grey surfaces under ink-blue text, with a deep blue accent.
    pub const fn dusk_light() -> Self {
        Self {
            background: hex(0xf7f9fc),
            sidebar: hex(0xeef2f7),
            surface: hex(0xf1f4f9),
            surface_hover: hex(0xe3e8f0),
            surface_active: hex(0xd2d9e5),
            surface_selected: hex(0xdde4ef),
            border: hex(0xdbe1ea),
            focused: hexa(0x5a7bb0a0),
            text: hex(0x1f2633),
            muted: hex(0x56607a),
            subtle: hex(0x8a93a8),
            accent: hex(0x2f5ea8),
            accent_hover: hex(0x1d4689),
            success: hex(0x1d8550),
            danger: hex(0xc23f36),
            on_accent: hex(0xffffff),
            on_success: hex(0xffffff),
            on_danger: hex(0xffffff),
            warning: hex(0x9c6a00),
            syntax_keyword: hex(0x7140b8),
            syntax_function: hex(0x2259b8),
            syntax_type: hex(0x147a7c),
            syntax_string: hex(0x3f7a2a),
            syntax_constant: hex(0xad5418),
            syntax_comment: hex(0x8790a3),
            syntax_property: hex(0x85660f),
            syntax_punctuation: hex(0x657088),
        }
    }

    /// The role a theme file calls `name`, such as `surface-hover`, to be painted.
    fn role_mut(&mut self, name: &str) -> Option<&mut Color> {
        Some(match name {
            "background" => &mut self.background,
            "sidebar" => &mut self.sidebar,
            "surface" => &mut self.surface,
            "surface-hover" => &mut self.surface_hover,
            "surface-active" => &mut self.surface_active,
            "surface-selected" => &mut self.surface_selected,
            "border" => &mut self.border,
            "focused" => &mut self.focused,
            "text" => &mut self.text,
            "muted" => &mut self.muted,
            "subtle" => &mut self.subtle,
            "accent" => &mut self.accent,
            "accent-hover" => &mut self.accent_hover,
            "success" => &mut self.success,
            "danger" => &mut self.danger,
            "on-accent" => &mut self.on_accent,
            "on-success" => &mut self.on_success,
            "on-danger" => &mut self.on_danger,
            "warning" => &mut self.warning,
            "syntax-keyword" => &mut self.syntax_keyword,
            "syntax-function" => &mut self.syntax_function,
            "syntax-type" => &mut self.syntax_type,
            "syntax-string" => &mut self.syntax_string,
            "syntax-constant" => &mut self.syntax_constant,
            "syntax-comment" => &mut self.syntax_comment,
            "syntax-property" => &mut self.syntax_property,
            "syntax-punctuation" => &mut self.syntax_punctuation,
            _ => return None,
        })
    }

    /// This palette with each role `colours` names painted in the colour it gives, written `#rrggbb` or `#rrggbbaa`; unknown roles and colours are left out.
    pub fn painted(mut self, colours: &[(String, String)]) -> Self {
        for (role, colour) in colours {
            if let (Some(colour), Some(slot)) = (colour_of(colour), self.role_mut(role)) {
                *slot = colour;
            }
        }
        self
    }

    /// The Material colour scheme this palette paints, on a dark ground when `dark`.
    pub fn scheme(&self, dark: bool) -> ColorScheme {
        let base = if dark {
            dark_color_scheme()
        } else {
            light_color_scheme()
        };
        let (lowest, error_share, ink) = if dark {
            (
                mix(self.background, hex(0x000000), 0.2),
                0.18,
                hex(0xffffff),
            )
        } else {
            (hex(0xffffff), 0.15, hex(0x000000))
        };
        ColorScheme {
            primary: self.accent,
            on_primary: self.on_accent,
            primary_container: self.surface_active,
            on_primary_container: self.text,
            inverse_primary: self.on_accent,
            secondary: self.muted,
            on_secondary: self.background,
            secondary_container: self.surface_selected,
            on_secondary_container: self.text,
            tertiary: self.muted,
            on_tertiary: self.background,
            tertiary_container: self.surface_selected,
            on_tertiary_container: self.text,
            background: self.background,
            on_background: self.text,
            surface: self.background,
            on_surface: self.text,
            surface_variant: self.surface_hover,
            on_surface_variant: self.muted,
            surface_tint: self.text,
            inverse_surface: self.text,
            inverse_on_surface: self.background,
            error: self.danger,
            on_error: self.on_danger,
            error_container: mix(self.background, self.danger, error_share),
            on_error_container: mix(self.danger, ink, 0.5),
            outline: self.focused,
            outline_variant: self.border,
            surface_bright: if dark {
                self.surface_active
            } else {
                self.background
            },
            surface_dim: if dark {
                self.background
            } else {
                self.surface_active
            },
            surface_container_lowest: lowest,
            surface_container_low: self.sidebar,
            surface_container: self.surface,
            surface_container_high: self.surface_hover,
            surface_container_highest: self.surface_active,
            ..base
        }
    }

    /// The colours Material has no role for that this palette paints.
    pub fn extended(&self, dark: bool) -> ExtendedColors {
        let base = if dark {
            ExtendedColors::dark()
        } else {
            ExtendedColors::light()
        };
        ExtendedColors {
            success: self.success,
            on_success: self.on_success,
            warning: self.warning,
            subtle: self.subtle,
            syntax_keyword: self.syntax_keyword,
            syntax_function: self.syntax_function,
            syntax_type: self.syntax_type,
            syntax_string: self.syntax_string,
            syntax_constant: self.syntax_constant,
            syntax_comment: self.syntax_comment,
            syntax_property: self.syntax_property,
            syntax_punctuation: self.syntax_punctuation,
            ..base
        }
    }
}

/// The colour written `#rrggbb` or `#rrggbbaa`.
fn colour_of(text: &str) -> Option<Color> {
    let digits = text.trim().strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(hex(value)),
        8 => Some(hexa(value)),
        _ => None,
    }
}

impl ThemeFamily {
    /// The family the app paints with when none other is chosen.
    pub fn default_family() -> Self {
        Self {
            name: DEFAULT_FAMILY.to_owned(),
            dark: Palette::default_dark(),
            light: Palette::default_light(),
        }
    }

    /// The family called `name` that paints the roles `dark` and `light` name, in Default's colours for the roles they leave out.
    pub fn from_colours(name: &str, dark: &[(String, String)], light: &[(String, String)]) -> Self {
        Self {
            name: name.to_owned(),
            dark: Palette::default_dark().painted(dark),
            light: Palette::default_light().painted(light),
        }
    }

    /// The whole theme this family paints, dark when `dark`.
    pub fn theme(&self, dark: bool) -> AppTheme {
        let base = AppTheme::of_dark(dark);
        if self.name == DEFAULT_FAMILY && *self == Self::default_family() {
            return base;
        }
        let palette = if dark { &self.dark } else { &self.light };
        AppTheme {
            color_scheme: palette.scheme(dark),
            extended_colors: palette.extended(dark),
            ..base
        }
    }
}

/// Every family the app ships, Default first.
pub fn shipped_families() -> Vec<ThemeFamily> {
    vec![
        ThemeFamily::default_family(),
        ThemeFamily {
            name: "Dusk".to_owned(),
            dark: Palette::dusk_dark(),
            light: Palette::dusk_light(),
        },
    ]
}

/// The family called `name` among the shipped families and `brought`, or Default when none is.
pub fn family_named(name: &str, brought: &[ThemeFamily]) -> ThemeFamily {
    shipped_families()
        .into_iter()
        .chain(brought.iter().cloned())
        .find(|family| family.name == name)
        .unwrap_or_else(ThemeFamily::default_family)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_theme::{pandemonium_dark_scheme, pandemonium_light_scheme};

    /// Default's palettes paint the schemes the app has always drawn.
    #[test]
    fn default_matches_the_app_schemes() {
        let dark = Palette::default_dark().scheme(true);
        let shipped = pandemonium_dark_scheme();
        assert_eq!(dark.background, shipped.background);
        assert_eq!(dark.surface_container_high, shipped.surface_container_high);
        assert_eq!(dark.outline_variant, shipped.outline_variant);
        let light = Palette::default_light().scheme(false);
        let shipped = pandemonium_light_scheme();
        assert_eq!(light.primary, shipped.primary);
        assert_eq!(light.surface_container_low, shipped.surface_container_low);
    }

    /// Dusk is shipped, and a family named nowhere falls back to Default.
    #[test]
    fn families_are_found_by_name() {
        assert_eq!(family_named("Dusk", &[]).dark.background, hex(0x161a22));
        assert_eq!(family_named("Nope", &[]).name, DEFAULT_FAMILY);
    }

    /// An extension's colours replace the roles they name and leave the rest Default's.
    #[test]
    fn extension_colours_paint_over_default() {
        let family = ThemeFamily::from_colours(
            "Ember",
            &[
                ("accent".to_owned(), "#ff8800".to_owned()),
                ("nonsense".to_owned(), "#000000".to_owned()),
                ("text".to_owned(), "bad".to_owned()),
            ],
            &[],
        );
        assert_eq!(family.dark.accent, hex(0xff8800));
        assert_eq!(family.dark.text, Palette::default_dark().text);
        assert_eq!(family.light, Palette::default_light());
        assert_eq!(family.theme(true).color_scheme.primary, hex(0xff8800));
    }
}
