//! The shape a theme takes on disk, how one is read in, and how one is
//! written out.
//!
//! A theme file names a family and gives either appearance of it. Every
//! colour is optional and falls back to the family the editor starts in, so a
//! file that renames three colours is a theme, and a colour added to the
//! editor later does not invalidate the files written before it. Colours are
//! named as [`crate::config::tokens`] names them, and written the way they
//! are written everywhere else: `"#rrggbb"`, or `"#rrggbbaa"` when a colour
//! is translucent.
//!
//! The reader's overrides are written in the same shape, one appearance at
//! a time, so a set of overrides pasted into a theme file is a theme.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use pm_gfx::Rgba;
use pm_ui::{Appearance, DEFAULT_FAMILY, Theme, ThemeFamily};
use serde::{Deserialize, Serialize};

use crate::config::overrides::ThemeOverrides;
use crate::config::paths;
use crate::config::tokens::{self, Group, TOKENS};

/// How many colours an ANSI palette has.
const ANSI: usize = 16;

/// The extension a theme file has.
const EXTENSION: &str = "yaml";

/// A theme family as it is written down.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct StoredFamily {
    /// The name the picker shows, which both appearances are named after.
    name: String,
    /// The dark appearance, where it differs from the default family's.
    dark: Option<StoredTheme>,
    /// The light appearance, where it differs from the default family's.
    light: Option<StoredTheme>,
}

/// The reader's overrides, as they are written down.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct StoredOverrides {
    /// What is repainted over a dark variant.
    #[serde(skip_serializing_if = "StoredTheme::is_empty")]
    dark: StoredTheme,
    /// What is repainted over a light variant.
    #[serde(skip_serializing_if = "StoredTheme::is_empty")]
    light: StoredTheme,
}

/// One appearance of a family, as it is written down.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredTheme {
    /// The semantic colours, by key.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    colors: BTreeMap<String, String>,
    /// The colours code is highlighted in, by key.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    syntax: BTreeMap<String, String>,
    /// The colours a terminal grid is drawn in.
    #[serde(skip_serializing_if = "StoredTerminal::is_empty")]
    terminal: StoredTerminal,
}

/// The colours a terminal grid is drawn in, as they are written down.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct StoredTerminal {
    /// The sixteen ANSI colours, in the order a terminal numbers them.
    #[serde(skip_serializing_if = "Option::is_none")]
    ansi: Option<Vec<String>>,
    /// The cursor and the selection, by key.
    #[serde(flatten)]
    named: BTreeMap<String, String>,
}

/// Every theme written in the editor's home, in the order their files sort.
///
/// A file that will not read or will not parse is skipped rather than
/// argued with: a theme the reader is halfway through writing must not stop
/// the editor opening.
pub(super) fn installed() -> Vec<ThemeFamily> {
    paths::texts(paths::themes(), EXTENSION)
        .iter()
        .filter_map(|text| serde_norway::from_str::<StoredFamily>(text).ok())
        .map(StoredFamily::into_family)
        .collect()
}

/// Writes a family called `name`, drawn `dark` and `light`, into the
/// editor's home as a theme of its own, saying where it went.
///
/// The file is named after the family, and a file already there is left
/// alone: a second theme of the same name is written beside it instead.
pub(super) fn write(name: &str, dark: &Theme, light: &Theme) -> Option<PathBuf> {
    let path = paths::unused_file(&paths::themes()?, name, "theme", EXTENSION)?;
    let family = StoredFamily {
        name: name.to_owned(),
        dark: Some(StoredTheme::of(dark)),
        light: Some(StoredTheme::of(light)),
    };
    let text = serde_norway::to_string(&family).ok()?;
    fs::write(&path, text).ok()?;
    Some(path)
}

impl StoredFamily {
    /// The family this file describes, over the family the editor starts in.
    fn into_family(self) -> ThemeFamily {
        let base = pm_ui::family(DEFAULT_FAMILY);
        let dark = self.dark.unwrap_or_default();
        let light = self.light.unwrap_or_default();
        ThemeFamily {
            name: leak(self.name.clone()),
            dark: dark.into_theme(base.dark, &self.name, Appearance::Dark),
            light: light.into_theme(base.light, &self.name, Appearance::Light),
        }
    }
}

impl StoredOverrides {
    /// The overrides this stands for, less any colour it misnames.
    pub(super) fn into_overrides(self) -> ThemeOverrides {
        ThemeOverrides::of(self.dark.colors(), self.light.colors())
    }

    /// How `overrides` are written down.
    pub(super) fn of(overrides: &ThemeOverrides) -> Self {
        Self {
            dark: StoredTheme::written(overrides.colors(Appearance::Dark)),
            light: StoredTheme::written(overrides.colors(Appearance::Light)),
        }
    }

    /// Whether nothing is repainted in either appearance.
    pub(super) fn is_empty(&self) -> bool {
        self.dark.is_empty() && self.light.is_empty()
    }
}

impl StoredTheme {
    /// This appearance over `base`, named after the family it belongs to.
    fn into_theme(self, base: Theme, family: &str, appearance: Appearance) -> Theme {
        let suffix = match appearance {
            Appearance::Dark => "Dark",
            Appearance::Light => "Light",
        };
        let mut theme = Theme {
            name: leak(format!("{family} {suffix}")),
            appearance,
            ..base
        };
        for (token, color) in self.colors() {
            TOKENS[token].write(&mut theme, color);
        }
        if let Some(ansi) = self.terminal.palette() {
            theme.terminal.ansi = ansi;
        }
        theme
    }

    /// `theme` written out in full, its palette included.
    fn of(theme: &Theme) -> Self {
        let colors = (0..TOKENS.len())
            .map(|token| (token, TOKENS[token].read(theme)))
            .collect();
        let mut stored = Self::written(&colors);
        stored.terminal.ansi = Some(
            theme
                .terminal
                .ansi
                .iter()
                .copied()
                .map(tokens::hex)
                .collect(),
        );
        stored
    }

    /// `colors` written under the groups and keys that name them.
    fn written(colors: &BTreeMap<usize, Rgba>) -> Self {
        let mut stored = Self::default();
        for (token, color) in colors {
            let token = &TOKENS[*token];
            let group = match token.group {
                Group::Colors => &mut stored.colors,
                Group::Syntax => &mut stored.syntax,
                Group::Terminal => &mut stored.terminal.named,
            };
            group.insert(token.key.to_owned(), tokens::hex(*color));
        }
        stored
    }

    /// Every colour this names that parses, by index into [`TOKENS`].
    fn colors(&self) -> BTreeMap<usize, Rgba> {
        [
            (Group::Colors, &self.colors),
            (Group::Syntax, &self.syntax),
            (Group::Terminal, &self.terminal.named),
        ]
        .into_iter()
        .flat_map(|(group, written)| {
            written.iter().filter_map(move |(key, hex)| {
                Some((tokens::find(group, key)?, tokens::from_hex(hex)?))
            })
        })
        .collect()
    }

    /// Whether this names no colour at all.
    fn is_empty(&self) -> bool {
        self.colors.is_empty() && self.syntax.is_empty() && self.terminal.is_empty()
    }
}

impl StoredTerminal {
    /// The palette this writes, when it writes a whole one.
    ///
    /// A palette of any length but sixteen is not a palette, and one with a
    /// colour that does not parse is not one either: the palette underneath
    /// is kept instead of being half overwritten.
    fn palette(&self) -> Option<[Rgba; ANSI]> {
        let written = self.ansi.as_ref().filter(|written| written.len() == ANSI)?;
        let parsed = written
            .iter()
            .map(|hex| tokens::from_hex(hex))
            .collect::<Option<Vec<_>>>()?;
        parsed.try_into().ok()
    }

    /// Whether this names no colour at all.
    fn is_empty(&self) -> bool {
        self.ansi.is_none() && self.named.is_empty()
    }
}

/// `name` as a name that lives as long as the editor does.
///
/// A theme is read once at launch and drawn from until the window closes, so
/// the handful of names a reader's themes bring are given the lifetime the
/// built-in ones already have rather than making every theme own its text.
fn leak(name: String) -> &'static str {
    name.leak()
}
