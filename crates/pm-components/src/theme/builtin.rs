//! The families the app ships.

use rustle::Color;

use super::{Role, Theme, ThemeFamily};

/// Near-black surfaces under light grey text, with a light grey accent.
fn dark() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::from_argb(0xff1a1b1c))
        .with(Role::Sidebar, Color::from_argb(0xff1a1b1c))
        .with(Role::Surface, Color::from_argb(0xff202122))
        .with(Role::SurfaceHover, Color::from_argb(0xff272829))
        .with(Role::SurfaceActive, Color::from_argb(0xff343536))
        .with(Role::SurfaceSelected, Color::from_argb(0xff2a2b2c))
        .with(Role::Border, Color::from_argb(0xff2a2b2c))
        .with(Role::Focused, Color::from_argb(0xa08c8c8c))
        .with(Role::Text, Color::from_argb(0xffd6d6d6))
        .with(Role::Muted, Color::from_argb(0xff9a9a9a))
        .with(Role::Subtle, Color::from_argb(0xff666666))
        .with(Role::Accent, Color::from_argb(0xffd6d6d6))
        .with(Role::AccentHover, Color::from_argb(0xffffffff))
        .with(Role::Success, Color::from_argb(0xff6fb98a))
        .with(Role::Danger, Color::from_argb(0xffd9806f))
        .with(Role::OnAccent, Color::from_argb(0xff1a1b1c))
        .with(Role::OnSuccess, Color::from_argb(0xff0e1a12))
        .with(Role::OnDanger, Color::from_argb(0xff1f0b07))
        .with(Role::Warning, Color::from_argb(0xffd7b46a))
        .with(Role::SyntaxKeyword, Color::from_argb(0xffc49fd8))
        .with(Role::SyntaxFunction, Color::from_argb(0xff8fb8e8))
        .with(Role::SyntaxType, Color::from_argb(0xff7fc9be))
        .with(Role::SyntaxString, Color::from_argb(0xffa9c98a))
        .with(Role::SyntaxConstant, Color::from_argb(0xffe0a77a))
        .with(Role::SyntaxComment, Color::from_argb(0xff6f7378))
        .with(Role::SyntaxProperty, Color::from_argb(0xffd8c79a))
        .with(Role::SyntaxPunctuation, Color::from_argb(0xff8a8c90))
}

/// Light grey surfaces under dark grey text, with a dark grey accent.
fn light() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::from_argb(0xfffbfbfb))
        .with(Role::Sidebar, Color::from_argb(0xfff4f4f4))
        .with(Role::Surface, Color::from_argb(0xfff6f6f7))
        .with(Role::SurfaceHover, Color::from_argb(0xffe8e8eb))
        .with(Role::SurfaceActive, Color::from_argb(0xffd8d8dc))
        .with(Role::SurfaceSelected, Color::from_argb(0xffe4e6ea))
        .with(Role::Border, Color::from_argb(0xffe2e2e6))
        .with(Role::Focused, Color::from_argb(0xa08c8c8c))
        .with(Role::Text, Color::from_argb(0xff26272a))
        .with(Role::Muted, Color::from_argb(0xff5f6165))
        .with(Role::Subtle, Color::from_argb(0xff8f9196))
        .with(Role::Accent, Color::from_argb(0xff2a2a2a))
        .with(Role::AccentHover, Color::from_argb(0xff000000))
        .with(Role::Success, Color::from_argb(0xff1f8a4c))
        .with(Role::Danger, Color::from_argb(0xffc4412c))
        .with(Role::OnAccent, Color::from_argb(0xfffbfbfb))
        .with(Role::OnSuccess, Color::from_argb(0xffffffff))
        .with(Role::OnDanger, Color::from_argb(0xffffffff))
        .with(Role::Warning, Color::from_argb(0xffa66f00))
        .with(Role::SyntaxKeyword, Color::from_argb(0xff8a3fb0))
        .with(Role::SyntaxFunction, Color::from_argb(0xff2f63b5))
        .with(Role::SyntaxType, Color::from_argb(0xff1b7d72))
        .with(Role::SyntaxString, Color::from_argb(0xff4f7a26))
        .with(Role::SyntaxConstant, Color::from_argb(0xffb35a1f))
        .with(Role::SyntaxComment, Color::from_argb(0xff8e9196))
        .with(Role::SyntaxProperty, Color::from_argb(0xff8a6a12))
        .with(Role::SyntaxPunctuation, Color::from_argb(0xff6d6f74))
}

/// Deep blue-grey surfaces under cool pale text, with a soft blue accent.
fn dusk_dark() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::from_argb(0xff161a22))
        .with(Role::Sidebar, Color::from_argb(0xff141820))
        .with(Role::Surface, Color::from_argb(0xff1c212b))
        .with(Role::SurfaceHover, Color::from_argb(0xff232936))
        .with(Role::SurfaceActive, Color::from_argb(0xff2e3544))
        .with(Role::SurfaceSelected, Color::from_argb(0xff262d3b))
        .with(Role::Border, Color::from_argb(0xff262c38))
        .with(Role::Focused, Color::from_argb(0xa07f9cc8))
        .with(Role::Text, Color::from_argb(0xffd5dbe6))
        .with(Role::Muted, Color::from_argb(0xff939cad))
        .with(Role::Subtle, Color::from_argb(0xff5f6879))
        .with(Role::Accent, Color::from_argb(0xff8fb0e6))
        .with(Role::AccentHover, Color::from_argb(0xffb3cbf2))
        .with(Role::Success, Color::from_argb(0xff78c29a))
        .with(Role::Danger, Color::from_argb(0xffe08a86))
        .with(Role::OnAccent, Color::from_argb(0xff10141b))
        .with(Role::OnSuccess, Color::from_argb(0xff0c1a12))
        .with(Role::OnDanger, Color::from_argb(0xff200c0b))
        .with(Role::Warning, Color::from_argb(0xffdcbc72))
        .with(Role::SyntaxKeyword, Color::from_argb(0xffb69cf0))
        .with(Role::SyntaxFunction, Color::from_argb(0xff82b4f2))
        .with(Role::SyntaxType, Color::from_argb(0xff6fcfd0))
        .with(Role::SyntaxString, Color::from_argb(0xff9fd08f))
        .with(Role::SyntaxConstant, Color::from_argb(0xffeaa77e))
        .with(Role::SyntaxComment, Color::from_argb(0xff657085))
        .with(Role::SyntaxProperty, Color::from_argb(0xffd9c594))
        .with(Role::SyntaxPunctuation, Color::from_argb(0xff8690a3))
}

/// Pale blue-grey surfaces under ink-blue text, with a deep blue accent.
fn dusk_light() -> Theme {
    Theme::blank()
        .with(Role::Background, Color::from_argb(0xfff7f9fc))
        .with(Role::Sidebar, Color::from_argb(0xffeef2f7))
        .with(Role::Surface, Color::from_argb(0xfff1f4f9))
        .with(Role::SurfaceHover, Color::from_argb(0xffe3e8f0))
        .with(Role::SurfaceActive, Color::from_argb(0xffd2d9e5))
        .with(Role::SurfaceSelected, Color::from_argb(0xffdde4ef))
        .with(Role::Border, Color::from_argb(0xffdbe1ea))
        .with(Role::Focused, Color::from_argb(0xa05a7bb0))
        .with(Role::Text, Color::from_argb(0xff1f2633))
        .with(Role::Muted, Color::from_argb(0xff56607a))
        .with(Role::Subtle, Color::from_argb(0xff8a93a8))
        .with(Role::Accent, Color::from_argb(0xff2f5ea8))
        .with(Role::AccentHover, Color::from_argb(0xff1d4689))
        .with(Role::Success, Color::from_argb(0xff1d8550))
        .with(Role::Danger, Color::from_argb(0xffc23f36))
        .with(Role::OnAccent, Color::from_argb(0xffffffff))
        .with(Role::OnSuccess, Color::from_argb(0xffffffff))
        .with(Role::OnDanger, Color::from_argb(0xffffffff))
        .with(Role::Warning, Color::from_argb(0xff9c6a00))
        .with(Role::SyntaxKeyword, Color::from_argb(0xff7140b8))
        .with(Role::SyntaxFunction, Color::from_argb(0xff2259b8))
        .with(Role::SyntaxType, Color::from_argb(0xff147a7c))
        .with(Role::SyntaxString, Color::from_argb(0xff3f7a2a))
        .with(Role::SyntaxConstant, Color::from_argb(0xffad5418))
        .with(Role::SyntaxComment, Color::from_argb(0xff8790a3))
        .with(Role::SyntaxProperty, Color::from_argb(0xff85660f))
        .with(Role::SyntaxPunctuation, Color::from_argb(0xff657088))
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
