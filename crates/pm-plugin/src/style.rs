//! The themes and icon themes an extension carries, read from their files.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::manifest::IconEntry;
use crate::package::Package;

/// One theme: a colour for each role it names, in a dark and a light variant.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Theme {
    /// The extension it comes from.
    pub extension: String,
    /// What the reader calls it.
    pub name: String,
    /// The colour of each role of the dark variant, as `#rrggbb` or `#rrggbbaa`, by the role's name.
    pub dark: Vec<(String, String)>,
    /// The colour of each role of the light variant.
    pub light: Vec<(String, String)>,
}

/// One icon of an icon theme with its artwork read.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Icons {
    /// The ending of the file names it is for, without the dot.
    pub suffix: String,
    /// The SVG document.
    pub svg: String,
    /// The colour it is tinted, as `#rrggbb`.
    pub color: Option<String>,
}

/// An icon theme: the icons files are shown with, by their endings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IconTheme {
    /// The extension it comes from.
    pub extension: String,
    /// What the reader calls it.
    pub name: String,
    /// The icons.
    pub icons: Vec<Icons>,
}

/// A theme file as it is written.
#[derive(Deserialize)]
struct ThemeFile {
    /// What the reader calls it.
    name: String,
    /// The dark variant.
    #[serde(default)]
    dark: BTreeMap<String, String>,
    /// The light variant.
    #[serde(default)]
    light: BTreeMap<String, String>,
}

/// An icon theme file as it is written.
#[derive(Deserialize)]
struct IconThemeFile {
    /// What the reader calls it.
    name: String,
    /// The icons.
    #[serde(default)]
    icons: Vec<IconEntry>,
}

/// Whether `text` is a colour written `#rrggbb` or `#rrggbbaa`.
fn is_colour(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|digits| {
        matches!(digits.len(), 6 | 8) && digits.chars().all(|digit| digit.is_ascii_hexdigit())
    })
}

/// `variant` as the pairs of a theme, or why a colour in it is not one.
fn colours(
    theme: &str,
    variant: BTreeMap<String, String>,
) -> Result<Vec<(String, String)>, String> {
    variant
        .into_iter()
        .map(|(role, colour)| {
            if is_colour(&colour) {
                Ok((role, colour))
            } else {
                Err(format!("{theme}: {colour} is not a colour like #1a2b3c"))
            }
        })
        .collect()
}

impl Package {
    /// The themes the extension carries; one whose file cannot be read is left out.
    pub fn themes(&self) -> Vec<Theme> {
        self.manifest
            .themes
            .iter()
            .filter_map(|entry| self.theme_at(&entry.file).ok())
            .collect()
    }

    /// The theme in the file `relative` of the extension.
    fn theme_at(&self, relative: &str) -> Result<Theme, String> {
        let text = std::fs::read_to_string(self.file(relative)?)
            .map_err(|error| format!("{relative}: {error}"))?;
        let file: ThemeFile =
            toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))?;
        Ok(Theme {
            extension: self.id.clone(),
            dark: colours(&file.name, file.dark)?,
            light: colours(&file.name, file.light)?,
            name: file.name,
        })
    }

    /// The icon themes the extension carries; one whose file cannot be read is left out.
    pub fn icon_themes(&self) -> Vec<IconTheme> {
        self.manifest
            .icon_themes
            .iter()
            .filter_map(|entry| self.icon_theme_at(&entry.file).ok())
            .collect()
    }

    /// The icon theme in the file `relative` of the extension, with the artwork of every icon read.
    fn icon_theme_at(&self, relative: &str) -> Result<IconTheme, String> {
        let text = std::fs::read_to_string(self.file(relative)?)
            .map_err(|error| format!("{relative}: {error}"))?;
        let file: IconThemeFile =
            toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))?;
        let icons = file
            .icons
            .into_iter()
            .filter_map(|entry| {
                Some(Icons {
                    svg: std::fs::read_to_string(self.file(&entry.file).ok()?).ok()?,
                    suffix: entry.suffix.to_lowercase(),
                    color: entry.color,
                })
            })
            .collect();
        Ok(IconTheme {
            extension: self.id.clone(),
            name: file.name,
            icons,
        })
    }
}
