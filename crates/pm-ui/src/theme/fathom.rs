//! The Fathom family: deep blues, the colour of a terminal at two in the
//! morning.

use pm_gfx::Rgba;

use crate::theme::{
    Appearance, Colors, Emphasis, Radii, Sizes, Syntax, Terminal, TextScale, Theme, ThemeFamily,
};

/// The family, in both appearances.
pub const fn family() -> ThemeFamily {
    ThemeFamily {
        name: "Fathom",
        dark: dark(),
        light: light(),
    }
}

/// The dark variant.
pub const fn dark() -> Theme {
    Theme {
        name: "Fathom Dark",
        appearance: Appearance::Dark,
        colors: Colors {
            background: Rgba::hex(0x041025),
            surface: Rgba::hex(0x071630),
            surface_hover: Rgba::hex(0x0c1f40),
            surface_active: Rgba::hex(0x12284f),
            surface_selected: Rgba::hex(0x0b2d63),
            border: Rgba::hex(0x17284a),
            border_variant: Rgba::hex(0x0e1c35),
            border_focused: Rgba::hex(0x6f9bd8),
            border_selected: Rgba::hex(0x4e7ec4),
            drop_target: Rgba::hexa(0x7f94b880),
            text: Rgba::hex(0xc9d8f2),
            text_muted: Rgba::hex(0x7f94b8),
            text_subtle: Rgba::hex(0x566c92),
            text_on_accent: Rgba::hex(0x041025),
            cursor: Rgba::hex(0x4e9fdc),
            selection: Rgba::hex(0x4e9fdc),
            link: Rgba::hex(0x66b1e8),
            accent: Rgba::hex(0x4e9fdc),
            accent_hover: Rgba::hex(0x66b1e8),
            accent_active: Rgba::hex(0x3a87c0),
            success: Rgba::hex(0x458e73),
            warning: Rgba::hex(0xe8b86a),
            danger: Rgba::hex(0xef7a70),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x7fa6ff),
            string: Rgba::hex(0x7fd1c0),
            function: Rgba::hex(0xa5c8ff),
            comment: Rgba::hex(0x4a5f85),
            number: Rgba::hex(0xe8a35c),
            type_name: Rgba::hex(0x9ad4ff),
            punctuation: Rgba::hex(0x6b7fa8),
            variable: Rgba::hex(0xcfe0ff),
            property: Rgba::hex(0x9fb8e8),
            constant: Rgba::hex(0xe8c98a),
            operator: Rgba::hex(0x8fa3cc),
            tag: Rgba::hex(0x86b4ff),
            attribute: Rgba::hex(0x8fe0d0),
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
        name: "Fathom Light",
        appearance: Appearance::Light,
        colors: Colors {
            background: Rgba::hex(0xf7f9fd),
            surface: Rgba::hex(0xffffff),
            surface_hover: Rgba::hex(0xedf2fa),
            surface_active: Rgba::hex(0xe2eaf6),
            surface_selected: Rgba::hex(0xdbe8fb),
            border: Rgba::hex(0xd7e0ee),
            border_variant: Rgba::hex(0xe8eef7),
            border_focused: Rgba::hex(0x2f5fa8),
            border_selected: Rgba::hex(0x2f6ad0),
            drop_target: Rgba::hexa(0x4f668880),
            text: Rgba::hex(0x14243d),
            text_muted: Rgba::hex(0x4f6688),
            text_subtle: Rgba::hex(0x8496b3),
            text_on_accent: Rgba::hex(0xffffff),
            cursor: Rgba::hex(0x2f6ad0),
            selection: Rgba::hex(0x2f6ad0),
            link: Rgba::hex(0x2f6ad0),
            accent: Rgba::hex(0x2f6ad0),
            accent_hover: Rgba::hex(0x3d79df),
            accent_active: Rgba::hex(0x24549f),
            success: Rgba::hex(0x15634f),
            warning: Rgba::hex(0x9c6f1c),
            danger: Rgba::hex(0xbb3d34),
        },
        syntax: Syntax {
            keyword: Rgba::hex(0x2f5fa8),
            string: Rgba::hex(0x1d8a6e),
            function: Rgba::hex(0x2f6ad0),
            comment: Rgba::hex(0x8496b3),
            number: Rgba::hex(0x9c6f1c),
            type_name: Rgba::hex(0x2f7f9c),
            punctuation: Rgba::hex(0x6b7891),
            variable: Rgba::hex(0x23364f),
            property: Rgba::hex(0x3a5a86),
            constant: Rgba::hex(0x8a5a14),
            operator: Rgba::hex(0x55688a),
            tag: Rgba::hex(0x2a5f9e),
            attribute: Rgba::hex(0x1d7f8a),
        },
        terminal: Terminal::LIGHT,
        text: TextScale::DEFAULT,
        size: Sizes::DEFAULT,
        radius: Radii::DEFAULT,
        emphasis: Emphasis::DEFAULT,
    }
}
