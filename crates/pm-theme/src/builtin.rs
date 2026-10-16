//! The family the app ships.

use pm_color::{Color, Role};

use crate::{Theme, ThemeFamily};

/// Near-black surfaces under light grey text, with a light grey accent.
fn dark() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::hex(0x1a1b1c))
        .with(Role::Sidebar, Color::hex(0x1a1b1c))
        .with(Role::Surface, Color::hex(0x202122))
        .with(Role::SurfaceHover, Color::hex(0x272829))
        .with(Role::SurfaceActive, Color::hex(0x343536))
        .with(Role::SurfaceSelected, Color::hex(0x2a2b2c))
        .with(Role::Border, Color::hex(0x2a2b2c))
        .with(Role::Focused, Color::hexa(0x8c8c8ca0))
        .with(Role::Text, Color::hex(0xd6d6d6))
        .with(Role::Muted, Color::hex(0x9a9a9a))
        .with(Role::Subtle, Color::hex(0x666666))
        .with(Role::Accent, Color::hex(0xd6d6d6))
        .with(Role::AccentHover, Color::hex(0xffffff))
        .with(Role::Success, Color::hex(0x6fb98a))
        .with(Role::Danger, Color::hex(0xd9806f))
        .with(Role::OnAccent, Color::hex(0x1a1b1c))
        .with(Role::OnSuccess, Color::hex(0x0e1a12))
        .with(Role::OnDanger, Color::hex(0x1f0b07))
        .with(Role::Warning, Color::hex(0xd7b46a))
        .with(Role::SyntaxKeyword, Color::hex(0xc49fd8))
        .with(Role::SyntaxFunction, Color::hex(0x8fb8e8))
        .with(Role::SyntaxType, Color::hex(0x7fc9be))
        .with(Role::SyntaxString, Color::hex(0xa9c98a))
        .with(Role::SyntaxConstant, Color::hex(0xe0a77a))
        .with(Role::SyntaxComment, Color::hex(0x6f7378))
        .with(Role::SyntaxProperty, Color::hex(0xd8c79a))
        .with(Role::SyntaxPunctuation, Color::hex(0x8a8c90))
}

/// Light grey surfaces under dark grey text, with a dark grey accent.
fn light() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::hex(0xfbfbfb))
        .with(Role::Sidebar, Color::hex(0xf4f4f4))
        .with(Role::Surface, Color::hex(0xf6f6f7))
        .with(Role::SurfaceHover, Color::hex(0xe8e8eb))
        .with(Role::SurfaceActive, Color::hex(0xd8d8dc))
        .with(Role::SurfaceSelected, Color::hex(0xe4e6ea))
        .with(Role::Border, Color::hex(0xe2e2e6))
        .with(Role::Focused, Color::hexa(0x8c8c8ca0))
        .with(Role::Text, Color::hex(0x26272a))
        .with(Role::Muted, Color::hex(0x5f6165))
        .with(Role::Subtle, Color::hex(0x8f9196))
        .with(Role::Accent, Color::hex(0x2a2a2a))
        .with(Role::AccentHover, Color::hex(0x000000))
        .with(Role::Success, Color::hex(0x1f8a4c))
        .with(Role::Danger, Color::hex(0xc4412c))
        .with(Role::OnAccent, Color::hex(0xfbfbfb))
        .with(Role::OnSuccess, Color::hex(0xffffff))
        .with(Role::OnDanger, Color::hex(0xffffff))
        .with(Role::Warning, Color::hex(0xa66f00))
        .with(Role::SyntaxKeyword, Color::hex(0x8a3fb0))
        .with(Role::SyntaxFunction, Color::hex(0x2f63b5))
        .with(Role::SyntaxType, Color::hex(0x1b7d72))
        .with(Role::SyntaxString, Color::hex(0x4f7a26))
        .with(Role::SyntaxConstant, Color::hex(0xb35a1f))
        .with(Role::SyntaxComment, Color::hex(0x8e9196))
        .with(Role::SyntaxProperty, Color::hex(0x8a6a12))
        .with(Role::SyntaxPunctuation, Color::hex(0x6d6f74))
}

/// The family the app ships, in both appearances.
pub fn default_family() -> ThemeFamily {
    ThemeFamily {
        name: "Default",
        dark: dark(),
        light: light(),
    }
}
