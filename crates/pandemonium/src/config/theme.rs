//! The shape a theme takes on disk, how one is read in, and how one is
//! written out.
//!
//! A theme file names its family, may name a family it builds on, and gives
//! what it paints in either appearance:
//!
//! ```yaml
//! name: My Theme
//! extends: Ember
//! dark:
//!   colors:
//!     background: "#101010"
//!   syntax:
//!     keyword: "#ff7b72"
//!   terminal:
//!     ansi: ["#000000", ...]
//!     cursor: "#bfbfbf"
//!   emphasis:
//!     selection: 0.5
//! ```
//!
//! Everything is optional and falls back to the family beneath, so a file
//! that repaints three colours is a theme, and a colour added to the editor
//! later does not invalidate the files written before it. Colours are named
//! as [`crate::theme::TOKENS`] names them, and written the way they are
//! written everywhere else: `"#rrggbb"`, or `"#rrggbbaa"` when a colour is
//! translucent. The families the editor ships are written the same way and
//! compiled in, and the reader's overrides are one appearance's colours at a
//! time in the same shape, so a set of overrides pasted into a theme file is
//! a theme. A colour that will not read is skipped, like a binding in a
//! keymap that will not: a theme half written must not stop the editor
//! opening.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use pm_gfx::Rgba;
use pm_ui::Appearance;
use serde::{Deserialize, Serialize};

use crate::config::overrides::ThemeOverrides;
use crate::config::paths;
use crate::theme::{self, ANSI, Group, Paint, TOKENS, ThemeFile};

/// The extension a theme file has.
const EXTENSION: &str = "yaml";

/// The families the editor ships, in the order they are offered.
const SHIPPED: [&str; 5] = [
    include_str!("../../themes/pandemonium.yaml"),
    include_str!("../../themes/fathom.yaml"),
    include_str!("../../themes/ember.yaml"),
    include_str!("../../themes/verdant.yaml"),
    include_str!("../../themes/vs-code.yaml"),
];

/// A theme family as it is written down.
#[derive(Debug, Deserialize, Serialize)]
struct StoredFamily {
    /// The name the picker shows, which both appearances are named after.
    name: String,
    /// The family it builds on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extends: Option<String>,
    /// What it paints over the dark appearance beneath.
    #[serde(default, skip_serializing_if = "StoredTheme::is_empty")]
    dark: StoredTheme,
    /// What it paints over the light appearance beneath.
    #[serde(default, skip_serializing_if = "StoredTheme::is_empty")]
    light: StoredTheme,
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
    /// How strongly the translucent parts of the window are drawn, by key.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    emphasis: BTreeMap<String, f32>,
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

/// The families the editor ships.
///
/// # Panics
///
/// Panics if one of them will not parse as a theme at all, which is a file
/// compiled into the binary that nothing running can repair.
pub(super) fn shipped() -> Vec<ThemeFile> {
    SHIPPED
        .iter()
        .map(|text| {
            serde_norway::from_str::<StoredFamily>(text)
                .unwrap_or_else(|error| panic!("a shipped theme does not parse: {error}"))
                .into_file()
        })
        .collect()
}

/// Every theme written in the editor's home, in the order their files sort.
pub(super) fn installed() -> Vec<ThemeFile> {
    paths::texts(paths::themes(), EXTENSION)
        .iter()
        .filter_map(|text| serde_norway::from_str::<StoredFamily>(text).ok())
        .map(StoredFamily::into_file)
        .collect()
}

/// Writes a family called `name` that builds on `extends` with `overrides`
/// painted over it into the editor's home, saying where it went.
pub(super) fn write(name: &str, extends: &str, overrides: &ThemeOverrides) -> Option<PathBuf> {
    let path = paths::unused_file(&paths::themes()?, name, "theme", EXTENSION)?;
    let written = StoredOverrides::of(overrides);
    let family = StoredFamily {
        name: name.to_owned(),
        extends: Some(extends.to_owned()),
        dark: written.dark,
        light: written.light,
    };
    let text = serde_norway::to_string(&family).ok()?;
    fs::write(&path, text).ok()?;
    Some(path)
}

impl StoredFamily {
    /// The family this file describes.
    fn into_file(self) -> ThemeFile {
        ThemeFile {
            name: self.name.leak(),
            extends: self.extends,
            dark: self.dark.paint(),
            light: self.light.paint(),
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
    /// What this appearance paints, less anything it cannot read.
    fn paint(&self) -> Paint {
        Paint {
            colors: self.colors(),
            palette: self.terminal.palette(),
            weights: self
                .emphasis
                .iter()
                .filter_map(|(key, value)| Some((theme::weight(key)?, *value)))
                .collect(),
        }
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
            group.insert(token.key.to_owned(), theme::hex(*color));
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
                Some((theme::token(group, key)?, theme::from_hex(hex)?))
            })
        })
        .collect()
    }

    /// Whether this names nothing at all.
    fn is_empty(&self) -> bool {
        self.colors.is_empty()
            && self.syntax.is_empty()
            && self.terminal.is_empty()
            && self.emphasis.is_empty()
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
            .map(|hex| theme::from_hex(hex))
            .collect::<Option<Vec<_>>>()?;
        parsed.try_into().ok()
    }

    /// Whether this names no colour at all.
    fn is_empty(&self) -> bool {
        self.ansi.is_none() && self.named.is_empty()
    }
}
