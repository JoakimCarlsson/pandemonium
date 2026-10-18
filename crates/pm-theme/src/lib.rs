//! The theme: which colour each role is, and the font stacks, for one
//! appearance, and the families a reader chooses between.
//!
//! Styles name roles (`Role::Surface`, `Role::Muted`) and never colours; a
//! painter resolves them against the [`Theme`] in hand. A family holds a dark
//! and a light theme, and each writes out every role, so neither depends on
//! the other.

mod builtin;
mod extension;
mod family;
mod fonts;
mod theme;

pub use builtin::{default_family, families, family_named, shipped_families};
pub use extension::{family_from_colours, set_extension_families};
pub use family::{Appearance, ThemeFamily};
pub use fonts::Fonts;
pub use theme::Theme;
