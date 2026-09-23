//! The VS Code family: the Dark 2026 and Light 2026 themes VS Code ships as
//! its default, with its default terminal palettes.
//!
//! VS Code paints hovers and selections as translucent washes; each is
//! given here as the colour it comes out as over the surface it is drawn on.

use pm_gfx::Rgba;

use crate::theme::{
    Appearance, Colors, Emphasis, Radii, Sizes, Syntax, Terminal, TextScale, Theme, ThemeFamily,
};

/// The family, in both appearances.
pub const fn family() -> ThemeFamily {
    ThemeFamily {
        name: "VS Code",
        dark: dark(),
        light: light(),
    }
}

/// The dark variant: Dark 2026.
pub const fn dark() -> Theme {
    Theme {
        name: "VS Code Dark",
        appearance: Appearance::Dark,
        colors: Colors {
            background: Rgba::hex(0x121314),
            surface: Rgba::hex(0x191a1b),
            surface_hover: Rgba::hex(0x2b2c2d),
            surface_active: Rgba::hex(0x474849),
            surface_selected: Rgba::hex(0x383939),
            border: Rgba::hex(0x2a2b2c),
            border_variant: Rgba::hex(0x2a2b2c),
            border_focused: Rgba::hexa(0x3994bcb3),
            border_selected: Rgba::hexa(0x3994bcb3),
            drop_target: Rgba::hexa(0x3994bc1a),
            text: Rgba::hex(0xbfbfbf),
            text_muted: Rgba::hex(0x8c8c8c),
            text_subtle: Rgba::hex(0x555555),
            text_on_accent: Rgba::hex(0xffffff),
            cursor: Rgba::hex(0xbbbebf),
            selection: Rgba::hex(0x276782),
            link: Rgba::hex(0x48a0c7),
            accent: Rgba::hex(0x297aa0),
            accent_hover: Rgba::hex(0x2b7da3),
            accent_active: Rgba::hex(0x297aa0),
            success: Rgba::hex(0x73c991),
            warning: Rgba::hex(0xcca700),
            danger: Rgba::hex(0xf48771),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0xff7b72),
            string: Rgba::hex(0xa5d6ff),
            function: Rgba::hex(0xd2a8ff),
            comment: Rgba::hex(0x8b949e),
            number: Rgba::hex(0x79c0ff),
            type_name: Rgba::hex(0xffa657),
            punctuation: Rgba::hex(0xbbbebf),
            variable: Rgba::hex(0xc9d1d9),
            property: Rgba::hex(0x79c0ff),
            constant: Rgba::hex(0x79c0ff),
            operator: Rgba::hex(0xff7b72),
            tag: Rgba::hex(0x7ee787),
            attribute: Rgba::hex(0x79c0ff),
        },
        terminal: Terminal {
            ansi: [
                Rgba::hex(0x000000),
                Rgba::hex(0xcd3131),
                Rgba::hex(0x0dbc79),
                Rgba::hex(0xe5e510),
                Rgba::hex(0x2472c8),
                Rgba::hex(0xbc3fbc),
                Rgba::hex(0x11a8cd),
                Rgba::hex(0xe5e5e5),
                Rgba::hex(0x666666),
                Rgba::hex(0xf14c4c),
                Rgba::hex(0x23d18b),
                Rgba::hex(0xf5f543),
                Rgba::hex(0x3b8eea),
                Rgba::hex(0xd670d6),
                Rgba::hex(0x29b8db),
                Rgba::hex(0xe5e5e5),
            ],
            cursor: Rgba::hex(0xbfbfbf),
            selection: Rgba::hexa(0x3994bc33),
        },
        text: TextScale::DEFAULT,
        size: Sizes::DEFAULT,
        radius: Radii::DEFAULT,
        emphasis: Emphasis {
            selection: 0.87,
            current_line: 0.11,
            ..Emphasis::DEFAULT
        },
    }
}

/// The light variant: Light 2026.
pub const fn light() -> Theme {
    Theme {
        name: "VS Code Light",
        appearance: Appearance::Light,
        colors: Colors {
            background: Rgba::hex(0xffffff),
            surface: Rgba::hex(0xfafafd),
            surface_hover: Rgba::hex(0xe6e6e9),
            surface_active: Rgba::hex(0xd6d6d8),
            surface_selected: Rgba::hex(0xd6d6d8),
            border: Rgba::hex(0xe2e2e5),
            border_variant: Rgba::hex(0xf0f1f2),
            border_focused: Rgba::hex(0x0069cc),
            border_selected: Rgba::hex(0x0069cc),
            drop_target: Rgba::hexa(0x0069cc15),
            text: Rgba::hex(0x202020),
            text_muted: Rgba::hex(0x606060),
            text_subtle: Rgba::hex(0xbbbbbb),
            text_on_accent: Rgba::hex(0xffffff),
            cursor: Rgba::hex(0x202020),
            selection: Rgba::hex(0x0069cc),
            link: Rgba::hex(0x0069cc),
            accent: Rgba::hex(0x0069cc),
            accent_hover: Rgba::hex(0x0063c1),
            accent_active: Rgba::hex(0x0063c1),
            success: Rgba::hex(0x587c0c),
            warning: Rgba::hex(0xb69500),
            danger: Rgba::hex(0xad0707),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0xcf222e),
            string: Rgba::hex(0x0a3069),
            function: Rgba::hex(0x8250df),
            comment: Rgba::hex(0x6e7781),
            number: Rgba::hex(0x0550ae),
            type_name: Rgba::hex(0x953800),
            punctuation: Rgba::hex(0x202020),
            variable: Rgba::hex(0x1f2328),
            property: Rgba::hex(0x0550ae),
            constant: Rgba::hex(0x0550ae),
            operator: Rgba::hex(0xcf222e),
            tag: Rgba::hex(0x116329),
            attribute: Rgba::hex(0x0550ae),
        },
        terminal: Terminal {
            ansi: [
                Rgba::hex(0x000000),
                Rgba::hex(0xcd3131),
                Rgba::hex(0x107c10),
                Rgba::hex(0x949800),
                Rgba::hex(0x0451a5),
                Rgba::hex(0xbc05bc),
                Rgba::hex(0x0598bc),
                Rgba::hex(0x555555),
                Rgba::hex(0x666666),
                Rgba::hex(0xf14c4c),
                Rgba::hex(0x14ce14),
                Rgba::hex(0xb5ba00),
                Rgba::hex(0x3b8eea),
                Rgba::hex(0xd670d6),
                Rgba::hex(0x29b8db),
                Rgba::hex(0xa5a5a5),
            ],
            cursor: Rgba::hex(0x202020),
            selection: Rgba::hexa(0x0069cc26),
        },
        text: TextScale::DEFAULT,
        size: Sizes::DEFAULT,
        radius: Radii::DEFAULT,
        emphasis: Emphasis {
            selection: 0.25,
            current_line: 0.02,
            ..Emphasis::DEFAULT
        },
    }
}
