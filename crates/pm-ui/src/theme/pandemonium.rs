//! The Pandemonium family: near-black neutral greys, and no accent colour.
//!
//! The theme the editor starts in. The primary action is a darker surface
//! rather than a bright fill, so the only colour on the screen comes from the
//! code and from the states that mean something — success, warning, danger.

use pm_gfx::Rgba;

use crate::theme::{
    Appearance, Colors, Emphasis, Radii, Sizes, Syntax, Terminal, TextScale, Theme, ThemeFamily,
};

/// The family, in both appearances.
pub const fn family() -> ThemeFamily {
    ThemeFamily {
        name: "Pandemonium",
        dark: dark(),
        light: light(),
    }
}

/// The dark variant.
pub const fn dark() -> Theme {
    Theme {
        name: "Pandemonium Dark",
        appearance: Appearance::Dark,
        colors: Colors {
            background: Rgba::hex(0x0e0f10),
            surface: Rgba::hex(0x151618),
            surface_hover: Rgba::hex(0x24272a),
            surface_active: Rgba::hex(0x2e3134),
            surface_selected: Rgba::hex(0x1f2225),
            border: Rgba::hex(0x26282b),
            border_variant: Rgba::hex(0x1a1c1d),
            border_focused: Rgba::hex(0x5d6268),
            border_selected: Rgba::hex(0x474c52),
            drop_target: Rgba::hexa(0x93979b80),
            text: Rgba::hex(0xe2e3e6),
            text_muted: Rgba::hex(0x93979b),
            text_subtle: Rgba::hex(0x63676a),
            text_on_accent: Rgba::hex(0xe2e3e6),
            cursor: Rgba::hex(0xe2e3e6),
            selection: Rgba::hex(0x9aa4b0),
            link: Rgba::hex(0x8fb2cf),
            accent: Rgba::hex(0x060708),
            accent_hover: Rgba::hex(0x191b1e),
            accent_active: Rgba::hex(0x030304),
            success: Rgba::hex(0x458158),
            warning: Rgba::hex(0xd9a343),
            danger: Rgba::hex(0xe06c62),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x8fa8c4),
            string: Rgba::hex(0x8bc79a),
            function: Rgba::hex(0x8fb2cf),
            comment: Rgba::hex(0x63676a),
            number: Rgba::hex(0xd9a343),
            type_name: Rgba::hex(0x9ec5a8),
            punctuation: Rgba::hex(0x7c848c),
            variable: Rgba::hex(0xe2e3e6),
            property: Rgba::hex(0xb9c6d4),
            constant: Rgba::hex(0xc9a97a),
            operator: Rgba::hex(0xa3abb3),
            tag: Rgba::hex(0xa9bfd8),
            attribute: Rgba::hex(0xb0c9a9),
        },
        terminal: Terminal::DARK,
        text: TextScale::DEFAULT,
        size: Sizes::DEFAULT,
        radius: Radii::DEFAULT,
        emphasis: Emphasis::DEFAULT,
    }
}

/// The light variant.
pub const fn light() -> Theme {
    Theme {
        name: "Pandemonium Light",
        appearance: Appearance::Light,
        colors: Colors {
            background: Rgba::hex(0xffffff),
            surface: Rgba::hex(0xf8f8f8),
            surface_hover: Rgba::hex(0xe5e5e7),
            surface_active: Rgba::hex(0xd8d8db),
            surface_selected: Rgba::hex(0xebedef),
            border: Rgba::hex(0xe0e0e0),
            border_variant: Rgba::hex(0xececec),
            border_focused: Rgba::hex(0x8d9298),
            border_selected: Rgba::hex(0x676c72),
            drop_target: Rgba::hexa(0x6a6a6a80),
            text: Rgba::hex(0x3b3b3b),
            text_muted: Rgba::hex(0x6a6a6a),
            text_subtle: Rgba::hex(0x949494),
            text_on_accent: Rgba::hex(0xffffff),
            cursor: Rgba::hex(0x16181b),
            selection: Rgba::hex(0x4a5560),
            link: Rgba::hex(0x3f5a78),
            accent: Rgba::hex(0x16181b),
            accent_hover: Rgba::hex(0x2e3237),
            accent_active: Rgba::hex(0x0b0c0e),
            success: Rgba::hex(0x165832),
            warning: Rgba::hex(0x9a6b0f),
            danger: Rgba::hex(0xb3332b),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x3f5a78),
            string: Rgba::hex(0x1f7a45),
            function: Rgba::hex(0x5c7d9e),
            comment: Rgba::hex(0x8a8a8a),
            number: Rgba::hex(0x9a6b0f),
            type_name: Rgba::hex(0x2f6b4a),
            punctuation: Rgba::hex(0x6b7178),
            variable: Rgba::hex(0x3b3b3b),
            property: Rgba::hex(0x40566b),
            constant: Rgba::hex(0x7a5410),
            operator: Rgba::hex(0x55606a),
            tag: Rgba::hex(0x34506e),
            attribute: Rgba::hex(0x3f6b4f),
        },
        terminal: Terminal::LIGHT,
        text: TextScale::DEFAULT,
        size: Sizes::DEFAULT,
        radius: Radii::DEFAULT,
        emphasis: Emphasis::DEFAULT,
    }
}
