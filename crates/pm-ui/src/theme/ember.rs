//! The Ember family: warm browns and amber, easy on the eyes late on.

use pm_gfx::Rgba;

use crate::theme::{Appearance, Colors, Radii, Syntax, TextScale, Theme, ThemeFamily};

/// The family, in both appearances.
pub const fn family() -> ThemeFamily {
    ThemeFamily {
        name: "Ember",
        dark: dark(),
        light: light(),
    }
}

/// The dark variant.
pub const fn dark() -> Theme {
    Theme {
        name: "Ember Dark",
        appearance: Appearance::Dark,
        colors: Colors {
            background: Rgba::hex(0x1b1510),
            surface: Rgba::hex(0x241c13),
            surface_hover: Rgba::hex(0x2e2318),
            surface_active: Rgba::hex(0x3a2c1e),
            surface_selected: Rgba::hex(0x3a2a15),
            border: Rgba::hex(0x3a2e20),
            border_variant: Rgba::hex(0x262019),
            border_focused: Rgba::hex(0xc08a4e),
            border_selected: Rgba::hex(0xa57a4c),
            text: Rgba::hex(0xe8d6bd),
            text_muted: Rgba::hex(0xab947a),
            text_subtle: Rgba::hex(0x7d6b55),
            text_on_accent: Rgba::hex(0x1b1510),
            accent: Rgba::hex(0xe0a35c),
            accent_hover: Rgba::hex(0xeab475),
            accent_active: Rgba::hex(0xc88b46),
            success: Rgba::hex(0x6f854c),
            warning: Rgba::hex(0xe0a35c),
            danger: Rgba::hex(0xe0705c),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0xe0a35c),
            string: Rgba::hex(0xb8c26a),
            function: Rgba::hex(0xf0c68a),
            comment: Rgba::hex(0x7d6b55),
            number: Rgba::hex(0xd98b6a),
        },
        text: TextScale::DEFAULT,
        radius: Radii::DEFAULT,
    }
}

/// The light variant.
pub const fn light() -> Theme {
    Theme {
        name: "Ember Light",
        appearance: Appearance::Light,
        colors: Colors {
            background: Rgba::hex(0xfbf6ee),
            surface: Rgba::hex(0xfffdf8),
            surface_hover: Rgba::hex(0xf3ebde),
            surface_active: Rgba::hex(0xeae0cd),
            surface_selected: Rgba::hex(0xf7e7cb),
            border: Rgba::hex(0xe4d8c3),
            border_variant: Rgba::hex(0xf0e7d7),
            border_focused: Rgba::hex(0xb0762c),
            border_selected: Rgba::hex(0x96631f),
            text: Rgba::hex(0x3b2f22),
            text_muted: Rgba::hex(0x6f5f4b),
            text_subtle: Rgba::hex(0x9a8a72),
            text_on_accent: Rgba::hex(0xfffdf8),
            accent: Rgba::hex(0xb0762c),
            accent_hover: Rgba::hex(0xc2853a),
            accent_active: Rgba::hex(0x96631f),
            success: Rgba::hex(0x435b1e),
            warning: Rgba::hex(0xa8711c),
            danger: Rgba::hex(0xb04a35),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0xa8711c),
            string: Rgba::hex(0x5d7f2a),
            function: Rgba::hex(0x8a5a14),
            comment: Rgba::hex(0x9a8a72),
            number: Rgba::hex(0xb04a35),
        },
        text: TextScale::DEFAULT,
        radius: Radii::DEFAULT,
    }
}
