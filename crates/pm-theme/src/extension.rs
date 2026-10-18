//! Families that extensions bring, written as a colour for each role they name.

use std::collections::BTreeMap;
use std::sync::RwLock;

use pm_color::{Color, Role};

use crate::{Theme, ThemeFamily, default_family};

/// The families extensions brought, by the source that told of them, such as the machine a server runs on.
static BROUGHT: RwLock<BTreeMap<String, Vec<ThemeFamily>>> = RwLock::new(BTreeMap::new());

/// The colour written `#rrggbb` or `#rrggbbaa`.
fn colour_of(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(Color::hex(value)),
        8 => Some(Color::hexa(value)),
        _ => None,
    }
}

/// The role a theme file calls `name`.
fn role_named(name: &str) -> Option<Role> {
    Role::ALL.into_iter().find(|role| role.name() == name)
}

/// `base` with each role `colours` names painted in the colour it gives; a role that is not one, or a colour that is not one, is left out.
fn painted(base: &Theme, colours: &[(String, String)]) -> Theme {
    colours.iter().fold(base.clone(), |theme, (role, colour)| {
        match (role_named(role), colour_of(colour)) {
            (Some(role), Some(colour)) => theme.with(role, colour),
            _ => theme,
        }
    })
}

/// The family called `name` that paints the roles `dark` and `light` name, in the default family's colours for the roles they leave out.
pub fn family_from_colours(
    name: &str,
    dark: &[(String, String)],
    light: &[(String, String)],
) -> ThemeFamily {
    let base = default_family();
    ThemeFamily {
        name: name.to_owned(),
        dark: painted(&base.dark, dark),
        light: painted(&base.light, light),
    }
}

/// Replaces the families `source` brought with `families`.
pub fn set_extension_families(source: &str, families: Vec<ThemeFamily>) {
    if let Ok(mut brought) = BROUGHT.write() {
        if families.is_empty() {
            brought.remove(source);
        } else {
            brought.insert(source.to_owned(), families);
        }
    }
}

/// Every family extensions brought, by source and then in the order they were told.
pub(crate) fn extension_families() -> Vec<ThemeFamily> {
    BROUGHT
        .read()
        .map(|brought| brought.values().flatten().cloned().collect())
        .unwrap_or_default()
}
