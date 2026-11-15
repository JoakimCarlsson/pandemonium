//! Pandemonium's theme: Rustle's Material colour scheme with Pandemonium's
//! greys, plus the [`ExtendedColors`] Material has no role for, provided
//! together through their own composition locals.

use rustle::{
    Color, ColorScheme, CompositionLocal, dark_color_scheme, is_system_in_dark_theme,
    light_color_scheme, theme,
};

/// The colours Pandemonium paints with that Material's colour scheme has no
/// role for, read with [`extended_colors`] as Compose's custom design systems
/// guide extends `MaterialTheme`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtendedColors {
    /// Success: a passed check, an added line.
    pub success: Color,
    /// Text and icons on `success`.
    pub on_success: Color,
    /// Caution.
    pub warning: Color,
    /// Faint text and marks, a step quieter than `on_surface_variant`.
    pub subtle: Color,
    /// Keywords in code.
    pub syntax_keyword: Color,
    /// Functions and macros in code.
    pub syntax_function: Color,
    /// Types and modules in code.
    pub syntax_type: Color,
    /// Strings in code.
    pub syntax_string: Color,
    /// Numbers and constants in code.
    pub syntax_constant: Color,
    /// Comments in code.
    pub syntax_comment: Color,
    /// Fields and attributes in code.
    pub syntax_property: Color,
    /// Punctuation and operators in code.
    pub syntax_punctuation: Color,
    /// The colours projects are drawn in: twelve hues at one lightness and
    /// chroma, readable under white text on both light and dark backgrounds.
    pub projects: [Color; 12],
}

/// The project colours, the same in both appearances.
const PROJECT_COLOURS: [Color; 12] = [
    Color::rgb(176, 72, 110),
    Color::rgb(181, 76, 61),
    Color::rgb(171, 89, 0),
    Color::rgb(145, 107, 0),
    Color::rgb(104, 124, 0),
    Color::rgb(28, 135, 66),
    Color::rgb(0, 138, 117),
    Color::rgb(0, 133, 158),
    Color::rgb(0, 121, 186),
    Color::rgb(82, 106, 195),
    Color::rgb(127, 91, 182),
    Color::rgb(158, 79, 152),
];

impl ExtendedColors {
    /// The extended colours drawn on a dark background.
    pub const fn dark() -> Self {
        Self {
            success: Color::rgb(0x6f, 0xb9, 0x8a),
            on_success: Color::rgb(0x0e, 0x1a, 0x12),
            warning: Color::rgb(0xd7, 0xb4, 0x6a),
            subtle: Color::rgb(0x70, 0x70, 0x70),
            syntax_keyword: Color::rgb(0xc4, 0x9f, 0xd8),
            syntax_function: Color::rgb(0x8f, 0xb8, 0xe8),
            syntax_type: Color::rgb(0x7f, 0xc9, 0xbe),
            syntax_string: Color::rgb(0xa9, 0xc9, 0x8a),
            syntax_constant: Color::rgb(0xe0, 0xa7, 0x7a),
            syntax_comment: Color::rgb(0x7a, 0x7e, 0x83),
            syntax_property: Color::rgb(0xd8, 0xc7, 0x9a),
            syntax_punctuation: Color::rgb(0x8a, 0x8c, 0x90),
            projects: PROJECT_COLOURS,
        }
    }

    /// The extended colours drawn on a light background.
    pub const fn light() -> Self {
        Self {
            success: Color::rgb(0x1f, 0x8a, 0x4c),
            on_success: Color::rgb(0xff, 0xff, 0xff),
            warning: Color::rgb(0xa6, 0x6f, 0x00),
            subtle: Color::rgb(0x82, 0x84, 0x89),
            syntax_keyword: Color::rgb(0x8a, 0x3f, 0xb0),
            syntax_function: Color::rgb(0x2f, 0x63, 0xb5),
            syntax_type: Color::rgb(0x1b, 0x7d, 0x72),
            syntax_string: Color::rgb(0x4f, 0x7a, 0x26),
            syntax_constant: Color::rgb(0xb3, 0x5a, 0x1f),
            syntax_comment: Color::rgb(0x82, 0x85, 0x8a),
            syntax_property: Color::rgb(0x8a, 0x6a, 0x12),
            syntax_punctuation: Color::rgb(0x6d, 0x6f, 0x74),
            projects: PROJECT_COLOURS,
        }
    }
}

/// The extended colours in effect, dark until [`app_theme`] provides others.
static LOCAL_EXTENDED_COLORS: CompositionLocal<ExtendedColors> =
    CompositionLocal::new(ExtendedColors::dark);

/// The extended colours in effect at this point of the composition.
pub fn extended_colors() -> ExtendedColors {
    LOCAL_EXTENDED_COLORS.current()
}

/// Pandemonium's near-black colour scheme: light grey text on near-black
/// surfaces, with a light grey primary.
pub fn pandemonium_dark_scheme() -> ColorScheme {
    let text = Color::rgb(0xd6, 0xd6, 0xd6);
    let muted = Color::rgb(0x9a, 0x9a, 0x9a);
    let background = Color::rgb(0x1a, 0x1b, 0x1c);
    ColorScheme {
        primary: text,
        on_primary: background,
        primary_container: Color::rgb(0x34, 0x35, 0x36),
        on_primary_container: text,
        inverse_primary: background,
        secondary: muted,
        on_secondary: background,
        secondary_container: Color::rgb(0x2a, 0x2b, 0x2c),
        on_secondary_container: text,
        tertiary: muted,
        on_tertiary: background,
        tertiary_container: Color::rgb(0x2a, 0x2b, 0x2c),
        on_tertiary_container: text,
        background,
        on_background: text,
        surface: background,
        on_surface: text,
        surface_variant: Color::rgb(0x27, 0x28, 0x29),
        on_surface_variant: muted,
        surface_tint: text,
        inverse_surface: text,
        inverse_on_surface: background,
        error: Color::rgb(0xd9, 0x80, 0x6f),
        on_error: Color::rgb(0x1f, 0x0b, 0x07),
        error_container: Color::rgb(0x3a, 0x22, 0x1d),
        on_error_container: Color::rgb(0xf0, 0xc4, 0xba),
        outline: Color::rgba(0x8c, 0x8c, 0x8c, 0xa0),
        outline_variant: Color::rgb(0x2a, 0x2b, 0x2c),
        scrim: Color::rgb(0, 0, 0),
        surface_bright: Color::rgb(0x34, 0x35, 0x36),
        surface_dim: background,
        surface_container_lowest: Color::rgb(0x15, 0x16, 0x17),
        surface_container_low: background,
        surface_container: Color::rgb(0x20, 0x21, 0x22),
        surface_container_high: Color::rgb(0x27, 0x28, 0x29),
        surface_container_highest: Color::rgb(0x34, 0x35, 0x36),
        ..dark_color_scheme()
    }
}

/// Pandemonium's light colour scheme: dark grey text on light grey surfaces,
/// with a dark grey primary.
pub fn pandemonium_light_scheme() -> ColorScheme {
    let text = Color::rgb(0x26, 0x27, 0x2a);
    let muted = Color::rgb(0x5f, 0x61, 0x65);
    let background = Color::rgb(0xfb, 0xfb, 0xfb);
    ColorScheme {
        primary: Color::rgb(0x2a, 0x2a, 0x2a),
        on_primary: background,
        primary_container: Color::rgb(0xd8, 0xd8, 0xdc),
        on_primary_container: text,
        inverse_primary: Color::rgb(0xd6, 0xd6, 0xd6),
        secondary: muted,
        on_secondary: background,
        secondary_container: Color::rgb(0xe4, 0xe6, 0xea),
        on_secondary_container: text,
        tertiary: muted,
        on_tertiary: background,
        tertiary_container: Color::rgb(0xe4, 0xe6, 0xea),
        on_tertiary_container: text,
        background,
        on_background: text,
        surface: background,
        on_surface: text,
        surface_variant: Color::rgb(0xe8, 0xe8, 0xeb),
        on_surface_variant: muted,
        surface_tint: text,
        inverse_surface: Color::rgb(0x2a, 0x2b, 0x2c),
        inverse_on_surface: Color::rgb(0xf4, 0xf4, 0xf4),
        error: Color::rgb(0xc4, 0x41, 0x2c),
        on_error: Color::rgb(0xff, 0xff, 0xff),
        error_container: Color::rgb(0xf8, 0xdd, 0xd8),
        on_error_container: Color::rgb(0x5a, 0x16, 0x0b),
        outline: Color::rgba(0x8c, 0x8c, 0x8c, 0xa0),
        outline_variant: Color::rgb(0xe2, 0xe2, 0xe6),
        scrim: Color::rgb(0, 0, 0),
        surface_bright: background,
        surface_dim: Color::rgb(0xd8, 0xd8, 0xdc),
        surface_container_lowest: Color::rgb(0xff, 0xff, 0xff),
        surface_container_low: Color::rgb(0xf4, 0xf4, 0xf4),
        surface_container: Color::rgb(0xf6, 0xf6, 0xf7),
        surface_container_high: Color::rgb(0xe8, 0xe8, 0xeb),
        surface_container_highest: Color::rgb(0xd8, 0xd8, 0xdc),
        ..light_color_scheme()
    }
}

/// A whole Pandemonium theme: the Material colour scheme and the extended
/// colours that go with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppTheme {
    /// The Material roles, read with Rustle's `color_scheme()`.
    pub color_scheme: ColorScheme,
    /// The colours Material has no role for, read with [`extended_colors`].
    pub extended_colors: ExtendedColors,
}

impl AppTheme {
    /// The dark theme.
    pub fn dark() -> Self {
        Self {
            color_scheme: pandemonium_dark_scheme(),
            extended_colors: ExtendedColors::dark(),
        }
    }

    /// The light theme.
    pub fn light() -> Self {
        Self {
            color_scheme: pandemonium_light_scheme(),
            extended_colors: ExtendedColors::light(),
        }
    }

    /// The dark or light theme, whichever the desktop is set to.
    pub fn system() -> Self {
        Self::of_dark(is_system_in_dark_theme())
    }

    /// The dark theme when `dark` holds, the light one otherwise.
    pub fn of_dark(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }
}

/// Runs `content` with `app` in effect: its colour scheme through Rustle's
/// `theme` and its extended colours through their own composition local.
pub fn app_theme<R>(app: AppTheme, content: impl FnOnce() -> R) -> R {
    LOCAL_EXTENDED_COLORS.provide(app.extended_colors, || {
        theme(app.color_scheme, None, None, content)
    })
}
