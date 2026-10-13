//! The family the app ships.

use pm_color::{Color, Role};

use crate::{Theme, ThemeFamily};

/// Near-black surfaces under light grey text, with a blue-teal accent.
fn dark() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::hex(0x121314))
        .with(Role::Surface, Color::hex(0x191a1b))
        .with(Role::SurfaceHover, Color::hex(0x2b2c2d))
        .with(Role::SurfaceActive, Color::hex(0x474849))
        .with(Role::SurfaceSelected, Color::hex(0x383939))
        .with(Role::Border, Color::hex(0x2a2b2c))
        .with(Role::Focused, Color::hexa(0x3994bcb3))
        .with(Role::Text, Color::hex(0xbfbfbf))
        .with(Role::Muted, Color::hex(0x8c8c8c))
        .with(Role::Subtle, Color::hex(0x555555))
        .with(Role::Accent, Color::hex(0x297aa0))
        .with(Role::AccentHover, Color::hex(0x2b7da3))
        .with(Role::Success, Color::hex(0x73c991))
        .with(Role::Danger, Color::hex(0xf48771))
        .with(Role::OnAccent, Color::hex(0xffffff))
        .with(Role::OnSuccess, Color::hex(0x0e1a12))
        .with(Role::OnDanger, Color::hex(0x1f0b07))
}

/// Light grey surfaces under dark grey text, with the same accent as the dark theme.
fn light() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::hex(0xf5f5f5))
        .with(Role::Surface, Color::hex(0xebebeb))
        .with(Role::SurfaceHover, Color::hex(0xdedede))
        .with(Role::SurfaceActive, Color::hex(0xcccccc))
        .with(Role::SurfaceSelected, Color::hex(0xd5dce0))
        .with(Role::Border, Color::hex(0xd0d0d0))
        .with(Role::Focused, Color::hexa(0x3994bcb3))
        .with(Role::Text, Color::hex(0x303030))
        .with(Role::Muted, Color::hex(0x606060))
        .with(Role::Subtle, Color::hex(0x8a8a8a))
        .with(Role::Accent, Color::hex(0x297aa0))
        .with(Role::AccentHover, Color::hex(0x2b7da3))
        .with(Role::Success, Color::hex(0x278047))
        .with(Role::Danger, Color::hex(0xbe4935))
        .with(Role::OnAccent, Color::hex(0xffffff))
        .with(Role::OnSuccess, Color::hex(0xffffff))
        .with(Role::OnDanger, Color::hex(0xffffff))
}

/// The family the app ships, in both appearances.
pub fn default_family() -> ThemeFamily {
    ThemeFamily {
        name: "Default",
        dark: dark(),
        light: light(),
    }
}
