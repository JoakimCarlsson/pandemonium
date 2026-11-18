//! The families the app ships.

use super::{Role, Tone};

use super::{Theme, ThemeFamily};

/// Near-black surfaces under light grey text, with a light grey accent.
fn dark() -> Theme {
    Theme::blank()
        .with(Role::Background, Tone::hex(0x1a1b1c))
        .with(Role::Sidebar, Tone::hex(0x1a1b1c))
        .with(Role::Surface, Tone::hex(0x202122))
        .with(Role::SurfaceHover, Tone::hex(0x272829))
        .with(Role::SurfaceActive, Tone::hex(0x343536))
        .with(Role::SurfaceSelected, Tone::hex(0x2a2b2c))
        .with(Role::Border, Tone::hex(0x2a2b2c))
        .with(Role::Focused, Tone::hexa(0x8c8c8ca0))
        .with(Role::Text, Tone::hex(0xd6d6d6))
        .with(Role::Muted, Tone::hex(0x9a9a9a))
        .with(Role::Subtle, Tone::hex(0x666666))
        .with(Role::Accent, Tone::hex(0xd6d6d6))
        .with(Role::AccentHover, Tone::hex(0xffffff))
        .with(Role::Success, Tone::hex(0x6fb98a))
        .with(Role::Danger, Tone::hex(0xd9806f))
        .with(Role::OnAccent, Tone::hex(0x1a1b1c))
        .with(Role::OnSuccess, Tone::hex(0x0e1a12))
        .with(Role::OnDanger, Tone::hex(0x1f0b07))
        .with(Role::Warning, Tone::hex(0xd7b46a))
        .with(Role::SyntaxKeyword, Tone::hex(0xc49fd8))
        .with(Role::SyntaxFunction, Tone::hex(0x8fb8e8))
        .with(Role::SyntaxType, Tone::hex(0x7fc9be))
        .with(Role::SyntaxString, Tone::hex(0xa9c98a))
        .with(Role::SyntaxConstant, Tone::hex(0xe0a77a))
        .with(Role::SyntaxComment, Tone::hex(0x6f7378))
        .with(Role::SyntaxProperty, Tone::hex(0xd8c79a))
        .with(Role::SyntaxPunctuation, Tone::hex(0x8a8c90))
}

/// Light grey surfaces under dark grey text, with a dark grey accent.
fn light() -> Theme {
    Theme::blank()
        .with(Role::Background, Tone::hex(0xfbfbfb))
        .with(Role::Sidebar, Tone::hex(0xf4f4f4))
        .with(Role::Surface, Tone::hex(0xf6f6f7))
        .with(Role::SurfaceHover, Tone::hex(0xe8e8eb))
        .with(Role::SurfaceActive, Tone::hex(0xd8d8dc))
        .with(Role::SurfaceSelected, Tone::hex(0xe4e6ea))
        .with(Role::Border, Tone::hex(0xe2e2e6))
        .with(Role::Focused, Tone::hexa(0x8c8c8ca0))
        .with(Role::Text, Tone::hex(0x26272a))
        .with(Role::Muted, Tone::hex(0x5f6165))
        .with(Role::Subtle, Tone::hex(0x8f9196))
        .with(Role::Accent, Tone::hex(0x2a2a2a))
        .with(Role::AccentHover, Tone::hex(0x000000))
        .with(Role::Success, Tone::hex(0x1f8a4c))
        .with(Role::Danger, Tone::hex(0xc4412c))
        .with(Role::OnAccent, Tone::hex(0xfbfbfb))
        .with(Role::OnSuccess, Tone::hex(0xffffff))
        .with(Role::OnDanger, Tone::hex(0xffffff))
        .with(Role::Warning, Tone::hex(0xa66f00))
        .with(Role::SyntaxKeyword, Tone::hex(0x8a3fb0))
        .with(Role::SyntaxFunction, Tone::hex(0x2f63b5))
        .with(Role::SyntaxType, Tone::hex(0x1b7d72))
        .with(Role::SyntaxString, Tone::hex(0x4f7a26))
        .with(Role::SyntaxConstant, Tone::hex(0xb35a1f))
        .with(Role::SyntaxComment, Tone::hex(0x8e9196))
        .with(Role::SyntaxProperty, Tone::hex(0x8a6a12))
        .with(Role::SyntaxPunctuation, Tone::hex(0x6d6f74))
}

/// Deep blue-grey surfaces under cool pale text, with a soft blue accent.
fn dusk_dark() -> Theme {
    Theme::blank()
        .with(Role::Background, Tone::hex(0x161a22))
        .with(Role::Sidebar, Tone::hex(0x141820))
        .with(Role::Surface, Tone::hex(0x1c212b))
        .with(Role::SurfaceHover, Tone::hex(0x232936))
        .with(Role::SurfaceActive, Tone::hex(0x2e3544))
        .with(Role::SurfaceSelected, Tone::hex(0x262d3b))
        .with(Role::Border, Tone::hex(0x262c38))
        .with(Role::Focused, Tone::hexa(0x7f9cc8a0))
        .with(Role::Text, Tone::hex(0xd5dbe6))
        .with(Role::Muted, Tone::hex(0x939cad))
        .with(Role::Subtle, Tone::hex(0x5f6879))
        .with(Role::Accent, Tone::hex(0x8fb0e6))
        .with(Role::AccentHover, Tone::hex(0xb3cbf2))
        .with(Role::Success, Tone::hex(0x78c29a))
        .with(Role::Danger, Tone::hex(0xe08a86))
        .with(Role::OnAccent, Tone::hex(0x10141b))
        .with(Role::OnSuccess, Tone::hex(0x0c1a12))
        .with(Role::OnDanger, Tone::hex(0x200c0b))
        .with(Role::Warning, Tone::hex(0xdcbc72))
        .with(Role::SyntaxKeyword, Tone::hex(0xb69cf0))
        .with(Role::SyntaxFunction, Tone::hex(0x82b4f2))
        .with(Role::SyntaxType, Tone::hex(0x6fcfd0))
        .with(Role::SyntaxString, Tone::hex(0x9fd08f))
        .with(Role::SyntaxConstant, Tone::hex(0xeaa77e))
        .with(Role::SyntaxComment, Tone::hex(0x657085))
        .with(Role::SyntaxProperty, Tone::hex(0xd9c594))
        .with(Role::SyntaxPunctuation, Tone::hex(0x8690a3))
}

/// Pale blue-grey surfaces under ink-blue text, with a deep blue accent.
fn dusk_light() -> Theme {
    Theme::blank()
        .with(Role::Background, Tone::hex(0xf7f9fc))
        .with(Role::Sidebar, Tone::hex(0xeef2f7))
        .with(Role::Surface, Tone::hex(0xf1f4f9))
        .with(Role::SurfaceHover, Tone::hex(0xe3e8f0))
        .with(Role::SurfaceActive, Tone::hex(0xd2d9e5))
        .with(Role::SurfaceSelected, Tone::hex(0xdde4ef))
        .with(Role::Border, Tone::hex(0xdbe1ea))
        .with(Role::Focused, Tone::hexa(0x5a7bb0a0))
        .with(Role::Text, Tone::hex(0x1f2633))
        .with(Role::Muted, Tone::hex(0x56607a))
        .with(Role::Subtle, Tone::hex(0x8a93a8))
        .with(Role::Accent, Tone::hex(0x2f5ea8))
        .with(Role::AccentHover, Tone::hex(0x1d4689))
        .with(Role::Success, Tone::hex(0x1d8550))
        .with(Role::Danger, Tone::hex(0xc23f36))
        .with(Role::OnAccent, Tone::hex(0xffffff))
        .with(Role::OnSuccess, Tone::hex(0xffffff))
        .with(Role::OnDanger, Tone::hex(0xffffff))
        .with(Role::Warning, Tone::hex(0x9c6a00))
        .with(Role::SyntaxKeyword, Tone::hex(0x7140b8))
        .with(Role::SyntaxFunction, Tone::hex(0x2259b8))
        .with(Role::SyntaxType, Tone::hex(0x147a7c))
        .with(Role::SyntaxString, Tone::hex(0x3f7a2a))
        .with(Role::SyntaxConstant, Tone::hex(0xad5418))
        .with(Role::SyntaxComment, Tone::hex(0x8790a3))
        .with(Role::SyntaxProperty, Tone::hex(0x85660f))
        .with(Role::SyntaxPunctuation, Tone::hex(0x657088))
}

/// The family the app ships, in both appearances.
pub fn default_family() -> ThemeFamily {
    ThemeFamily {
        name: "Default".to_owned(),
        dark: dark(),
        light: light(),
    }
}

/// Every family the app ships, the default first.
pub fn shipped_families() -> Vec<ThemeFamily> {
    vec![
        default_family(),
        ThemeFamily {
            name: "Dusk".to_owned(),
            dark: dusk_dark(),
            light: dusk_light(),
        },
    ]
}

/// Every family there is: those the app ships, the default first, then those extensions brought.
pub fn families() -> Vec<ThemeFamily> {
    let mut every = shipped_families();
    every.extend(super::extension::extension_families());
    every
}

/// The family called `name`, or the default when none is.
pub fn family_named(name: &str) -> ThemeFamily {
    families()
        .into_iter()
        .find(|family| family.name == name)
        .unwrap_or_else(default_family)
}
