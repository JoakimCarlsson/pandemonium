//! Families that extensions bring, written as a colour for each role they name.

use std::collections::BTreeMap;
use std::sync::RwLock;

use rustle::Color;

use super::{Role, Theme, ThemeFamily, default_family};

/// The families extensions brought, by the source that told of them, such as the machine a server runs on.
static BROUGHT: RwLock<BTreeMap<String, Vec<ThemeFamily>>> = RwLock::new(BTreeMap::new());

/// The colour written `#rrggbb` or `#rrggbbaa`.
fn colour_of(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(Color::from_argb(0xff00_0000 | value)),
        8 => Some(Color::from_argb(value.rotate_right(8))),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An extension's family paints the roles it names and keeps the default family's colours for the rest.
    #[test]
    fn extension_families_paint_the_roles_they_name() {
        let base = default_family();
        let family = family_from_colours(
            "Ember",
            &[("background".to_owned(), "#102030".to_owned())],
            &[],
        );
        assert_eq!(family.name, "Ember");
        assert_eq!(family.light, base.light);
        assert_ne!(family.dark, base.dark);
        set_extension_families("test-extension", vec![family.clone()]);
        assert!(
            super::super::families()
                .iter()
                .any(|held| held.name == "Ember")
        );
        assert_eq!(super::super::family_named("Ember").name, "Ember");
        set_extension_families("test-extension", Vec::new());
        assert!(
            !super::super::families()
                .iter()
                .any(|held| held.name == "Ember")
        );
    }
}
