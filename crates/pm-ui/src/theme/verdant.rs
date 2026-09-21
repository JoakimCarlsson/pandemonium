//! The Verdant family: teal surfaces and a green accent, in the tradition of
//! solarized.

use pm_gfx::Rgba;

use crate::theme::{Appearance, Colors, Radii, Syntax, Terminal, TextScale, Theme, ThemeFamily};

/// The family, in both appearances.
pub const fn family() -> ThemeFamily {
    ThemeFamily {
        name: "Verdant",
        dark: dark(),
        light: light(),
    }
}

/// The dark variant.
pub const fn dark() -> Theme {
    Theme {
        name: "Verdant Dark",
        appearance: Appearance::Dark,
        colors: Colors {
            background: Rgba::hex(0x04262b),
            surface: Rgba::hex(0x073640),
            surface_hover: Rgba::hex(0x0b4450),
            surface_active: Rgba::hex(0x10505e),
            surface_selected: Rgba::hex(0x0b4a4a),
            border: Rgba::hex(0x145561),
            border_variant: Rgba::hex(0x0a323a),
            border_focused: Rgba::hex(0x2aa198),
            border_selected: Rgba::hex(0x1e8a82),
            text: Rgba::hex(0xc5d6d3),
            text_muted: Rgba::hex(0x86a3a0),
            text_subtle: Rgba::hex(0x5d7d7a),
            text_on_accent: Rgba::hex(0x04262b),
            accent: Rgba::hex(0x2aa198),
            accent_hover: Rgba::hex(0x37b6ac),
            accent_active: Rgba::hex(0x1e8a82),
            success: Rgba::hex(0x5b8a45),
            warning: Rgba::hex(0xcfa72a),
            danger: Rgba::hex(0xdc6b5f),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x2aa198),
            string: Rgba::hex(0xa4c25f),
            function: Rgba::hex(0x6fb3d2),
            comment: Rgba::hex(0x5d7d7a),
            number: Rgba::hex(0xcfa72a),
        },
        terminal: Terminal::DARK,
        text: TextScale::DEFAULT,
        radius: Radii::DEFAULT,
    }
}

/// The light variant.
pub const fn light() -> Theme {
    Theme {
        name: "Verdant Light",
        appearance: Appearance::Light,
        colors: Colors {
            background: Rgba::hex(0xfdf6e3),
            surface: Rgba::hex(0xfffbf0),
            surface_hover: Rgba::hex(0xf3ecd8),
            surface_active: Rgba::hex(0xe9e1c9),
            surface_selected: Rgba::hex(0xe3ecd8),
            border: Rgba::hex(0xe0d8c0),
            border_variant: Rgba::hex(0xefe8d5),
            border_focused: Rgba::hex(0x2aa198),
            border_selected: Rgba::hex(0x1e8a82),
            text: Rgba::hex(0x14383a),
            text_muted: Rgba::hex(0x5b7273),
            text_subtle: Rgba::hex(0x8a9c9a),
            text_on_accent: Rgba::hex(0xfffbf0),
            accent: Rgba::hex(0x1e8a82),
            accent_hover: Rgba::hex(0x2aa198),
            accent_active: Rgba::hex(0x176b65),
            success: Rgba::hex(0x456722),
            warning: Rgba::hex(0xa8801a),
            danger: Rgba::hex(0xbb4a3d),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x1e8a82),
            string: Rgba::hex(0x5f8f2f),
            function: Rgba::hex(0x2f6f8f),
            comment: Rgba::hex(0x8a9c9a),
            number: Rgba::hex(0xa8801a),
        },
        terminal: Terminal::LIGHT,
        text: TextScale::DEFAULT,
        radius: Radii::DEFAULT,
    }
}
