//! The shape a theme takes on disk, and how one is read in.
//!
//! A theme file names a family and gives either appearance of it. Every
//! token is optional and falls back to the family the editor starts in, so a
//! file that renames three colours is a theme, and a token added to the
//! editor later does not invalidate the files written before it.
//!
//! Colours are written the way they are written everywhere else: `"#rrggbb"`,
//! or `"#rrggbbaa"` when a token is translucent.

use std::fs;

use pm_gfx::Rgba;
use pm_ui::{Appearance, Colors, DEFAULT_FAMILY, Syntax, Terminal, Theme, ThemeFamily};
use serde::Deserialize;

use crate::config::paths;

/// How many colours an ANSI palette has.
const ANSI: usize = 16;

/// A theme family as it is written down.
#[derive(Debug, Deserialize)]
pub(super) struct StoredFamily {
    /// The name the picker shows, which both appearances are named after.
    name: String,
    /// The dark appearance, where it differs from the default family's.
    dark: Option<StoredTheme>,
    /// The light appearance, where it differs from the default family's.
    light: Option<StoredTheme>,
}

/// One appearance of a family, as it is written down.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StoredTheme {
    /// The semantic colours.
    colors: StoredColors,
    /// The colours code is highlighted in.
    syntax: StoredSyntax,
    /// The colours a terminal grid is drawn in.
    terminal: StoredTerminal,
}

/// The semantic colours, as they are written down.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StoredColors {
    /// The background colour.
    background: Option<String>,
    /// The surface colour.
    surface: Option<String>,
    /// The surface hover colour.
    surface_hover: Option<String>,
    /// The surface active colour.
    surface_active: Option<String>,
    /// The surface selected colour.
    surface_selected: Option<String>,
    /// The border colour.
    border: Option<String>,
    /// The border variant colour.
    border_variant: Option<String>,
    /// The border focused colour.
    border_focused: Option<String>,
    /// The border selected colour.
    border_selected: Option<String>,
    /// The drop target colour.
    drop_target: Option<String>,
    /// The text colour.
    text: Option<String>,
    /// The text muted colour.
    text_muted: Option<String>,
    /// The text subtle colour.
    text_subtle: Option<String>,
    /// The text on accent colour.
    text_on_accent: Option<String>,
    /// The cursor colour.
    cursor: Option<String>,
    /// The selection colour.
    selection: Option<String>,
    /// The accent colour.
    accent: Option<String>,
    /// The accent hover colour.
    accent_hover: Option<String>,
    /// The accent active colour.
    accent_active: Option<String>,
    /// The success colour.
    success: Option<String>,
    /// The warning colour.
    warning: Option<String>,
    /// The danger colour.
    danger: Option<String>,
}

/// The colours code is highlighted in, as they are written down.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StoredSyntax {
    /// The colour keyword is drawn in.
    keyword: Option<String>,
    /// The colour string is drawn in.
    string: Option<String>,
    /// The colour function is drawn in.
    function: Option<String>,
    /// The colour comment is drawn in.
    comment: Option<String>,
    /// The colour number is drawn in.
    number: Option<String>,
    /// The colour type name is drawn in.
    type_name: Option<String>,
    /// The colour punctuation is drawn in.
    punctuation: Option<String>,
    /// The colour variable is drawn in.
    variable: Option<String>,
    /// The colour property is drawn in.
    property: Option<String>,
    /// The colour constant is drawn in.
    constant: Option<String>,
    /// The colour operator is drawn in.
    operator: Option<String>,
    /// The colour tag is drawn in.
    tag: Option<String>,
    /// The colour attribute is drawn in.
    attribute: Option<String>,
}

/// The colours a terminal grid is drawn in, as they are written down.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StoredTerminal {
    /// The sixteen ANSI colours, in the order a terminal numbers them.
    ansi: Option<Vec<String>>,
    /// The colour the terminal cursor is drawn in.
    cursor: Option<String>,
    /// The colour a terminal selection is washed in.
    selection: Option<String>,
}

/// Every theme written in the editor's home, in the order their files sort.
///
/// A file that will not read or will not parse is skipped rather than
/// argued with: a theme the reader is halfway through writing must not stop
/// the editor opening.
pub(super) fn installed() -> Vec<ThemeFamily> {
    let Some(directory) = paths::themes() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "yaml"))
        .collect::<Vec<_>>();
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| serde_norway::from_str::<StoredFamily>(&text).ok())
        .map(StoredFamily::into_family)
        .collect()
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

impl StoredTheme {
    /// This appearance over `base`, named after the family it belongs to.
    fn into_theme(self, base: Theme, family: &str, appearance: Appearance) -> Theme {
        let suffix = match appearance {
            Appearance::Dark => "Dark",
            Appearance::Light => "Light",
        };
        Theme {
            name: leak(format!("{family} {suffix}")),
            appearance,
            colors: self.colors.into_colors(base.colors),
            syntax: self.syntax.into_syntax(base.syntax),
            terminal: self.terminal.into_terminal(base.terminal),
            ..base
        }
    }
}

impl StoredColors {
    /// These colours over `base`.
    fn into_colors(self, base: Colors) -> Colors {
        Colors {
            background: color(self.background.as_deref(), base.background),
            surface: color(self.surface.as_deref(), base.surface),
            surface_hover: color(self.surface_hover.as_deref(), base.surface_hover),
            surface_active: color(self.surface_active.as_deref(), base.surface_active),
            surface_selected: color(self.surface_selected.as_deref(), base.surface_selected),
            border: color(self.border.as_deref(), base.border),
            border_variant: color(self.border_variant.as_deref(), base.border_variant),
            border_focused: color(self.border_focused.as_deref(), base.border_focused),
            border_selected: color(self.border_selected.as_deref(), base.border_selected),
            drop_target: color(self.drop_target.as_deref(), base.drop_target),
            text: color(self.text.as_deref(), base.text),
            text_muted: color(self.text_muted.as_deref(), base.text_muted),
            text_subtle: color(self.text_subtle.as_deref(), base.text_subtle),
            text_on_accent: color(self.text_on_accent.as_deref(), base.text_on_accent),
            cursor: color(self.cursor.as_deref(), base.cursor),
            selection: color(self.selection.as_deref(), base.selection),
            accent: color(self.accent.as_deref(), base.accent),
            accent_hover: color(self.accent_hover.as_deref(), base.accent_hover),
            accent_active: color(self.accent_active.as_deref(), base.accent_active),
            success: color(self.success.as_deref(), base.success),
            warning: color(self.warning.as_deref(), base.warning),
            danger: color(self.danger.as_deref(), base.danger),
        }
    }
}

impl StoredSyntax {
    /// These colours over `base`.
    fn into_syntax(self, base: Syntax) -> Syntax {
        Syntax {
            keyword: color(self.keyword.as_deref(), base.keyword),
            string: color(self.string.as_deref(), base.string),
            function: color(self.function.as_deref(), base.function),
            comment: color(self.comment.as_deref(), base.comment),
            number: color(self.number.as_deref(), base.number),
            type_name: color(self.type_name.as_deref(), base.type_name),
            punctuation: color(self.punctuation.as_deref(), base.punctuation),
            variable: color(self.variable.as_deref(), base.variable),
            property: color(self.property.as_deref(), base.property),
            constant: color(self.constant.as_deref(), base.constant),
            operator: color(self.operator.as_deref(), base.operator),
            tag: color(self.tag.as_deref(), base.tag),
            attribute: color(self.attribute.as_deref(), base.attribute),
        }
    }
}

impl StoredTerminal {
    /// These colours over `base`.
    ///
    /// A palette of any length but sixteen is not a palette, and the one
    /// underneath is kept instead of being half overwritten.
    fn into_terminal(self, base: Terminal) -> Terminal {
        let mut ansi = base.ansi;
        if let Some(written) = self.ansi.filter(|written| written.len() == ANSI) {
            for (slot, hex) in ansi.iter_mut().zip(written) {
                *slot = color(Some(&hex), *slot);
            }
        }
        Terminal {
            ansi,
            cursor: color(self.cursor.as_deref(), base.cursor),
            selection: color(self.selection.as_deref(), base.selection),
        }
    }
}

/// `hex` as a colour, or `base` when it is absent or not one.
fn color(hex: Option<&str>, base: Rgba) -> Rgba {
    hex.and_then(parse).unwrap_or(base)
}

/// `"#rrggbb"` or `"#rrggbbaa"` as a colour.
fn parse(hex: &str) -> Option<Rgba> {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok().map(Rgba::hex),
        8 => u32::from_str_radix(digits, 16).ok().map(Rgba::hexa),
        _ => None,
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
