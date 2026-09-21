//! The Pandemonium family: near-black neutral greys around a plum accent.
//!
//! The theme the editor starts in: surfaces dark enough to disappear, one
//! colour on top of them, and nothing competing with the code.

use pm_gfx::Rgba;

use crate::theme::{Appearance, Colors, Radii, Syntax, TextScale, Theme, ThemeFamily};

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
            surface_hover: Rgba::hex(0x1d1f21),
            surface_active: Rgba::hex(0x26282b),
            surface_selected: Rgba::hex(0x241c2d),
            border: Rgba::hex(0x26282b),
            border_variant: Rgba::hex(0x1a1c1d),
            border_focused: Rgba::hex(0xb57bd6),
            border_selected: Rgba::hex(0x9d63bd),
            text: Rgba::hex(0xe2e3e6),
            text_muted: Rgba::hex(0x93979b),
            text_subtle: Rgba::hex(0x63676a),
            text_on_accent: Rgba::hex(0x0e0f10),
            accent: Rgba::hex(0xb57bd6),
            accent_hover: Rgba::hex(0xc690e2),
            accent_active: Rgba::hex(0x9d63bd),
            success: Rgba::hex(0x5fb37a),
            warning: Rgba::hex(0xd9a343),
            danger: Rgba::hex(0xe06c62),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0xb57bd6),
            string: Rgba::hex(0x8bc79a),
            function: Rgba::hex(0x8fb2cf),
            comment: Rgba::hex(0x63676a),
            number: Rgba::hex(0xd9a343),
        },
        text: TextScale::DEFAULT,
        radius: Radii::DEFAULT,
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
            surface_hover: Rgba::hex(0xededed),
            surface_active: Rgba::hex(0xe2e2e2),
            surface_selected: Rgba::hex(0xf0e8f7),
            border: Rgba::hex(0xe0e0e0),
            border_variant: Rgba::hex(0xececec),
            border_focused: Rgba::hex(0x7b4aa0),
            border_selected: Rgba::hex(0x8d5bb3),
            text: Rgba::hex(0x3b3b3b),
            text_muted: Rgba::hex(0x6a6a6a),
            text_subtle: Rgba::hex(0x949494),
            text_on_accent: Rgba::hex(0xffffff),
            accent: Rgba::hex(0x7b4aa0),
            accent_hover: Rgba::hex(0x8d5bb3),
            accent_active: Rgba::hex(0x653a86),
            success: Rgba::hex(0x1f7a45),
            warning: Rgba::hex(0x9a6b0f),
            danger: Rgba::hex(0xb3332b),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x7b4aa0),
            string: Rgba::hex(0x1f7a45),
            function: Rgba::hex(0x8d5bb3),
            comment: Rgba::hex(0x8a8a8a),
            number: Rgba::hex(0x9a6b0f),
        },
        text: TextScale::DEFAULT,
        radius: Radii::DEFAULT,
    }
}
