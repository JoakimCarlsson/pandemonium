//! File and folder icons, and which installed language a file is written in: a file is shown with the icon of that language, and as a plain file otherwise; nothing language-specific ships with the app.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use pm_icon::{IconName, Svg};
use pm_protocol::{ExtensionIconTheme, ExtensionInfo};
use pm_style::{Color, Role, size, text_color};
use pm_ui::{Node, node};

use crate::parts::glyph;

/// What an installed language says of the files whose names end as it reads.
struct Known {
    /// The language, by its folder's name.
    language: String,
    /// The icon and colour its files are shown with, when it has one.
    icon: Option<(Svg, Color)>,
}

/// What the installed languages of each machine say of their files: by machine, then by the ending of a file name.
type Learned = HashMap<Option<String>, HashMap<String, Known>>;

/// What was learned from the languages installed on the machines reached.
static LEARNED: LazyLock<Mutex<Learned>> = LazyLock::new(Default::default);

/// The icons the icon themes of each machine give the endings of file names: by machine, then by ending.
type Themed = HashMap<Option<String>, HashMap<String, (Svg, Color)>>;

/// What was learned from the icon themes installed on the machines reached.
static THEMED: LazyLock<Mutex<Themed>> = LazyLock::new(Default::default);

/// The colour `hex`, written `#rrggbb`, names.
fn colour_of(hex: &str) -> Option<Color> {
    let digits = hex.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16)
        .ok()
        .filter(|_| digits.len() == 6)?;
    Some(Color::rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

/// Takes in the languages installed on `host` and their icons, as `extensions` lists them, in place of those it had before.
pub fn learn_languages(host: &Option<String>, extensions: &[ExtensionInfo]) {
    let mut endings = HashMap::new();
    for extension in extensions.iter().filter(|extension| extension.installed) {
        let icon = extension.icon.as_ref().map(|source| {
            let svg = Svg::arrived(&format!("extension-{}", extension.id), source);
            let tone = extension
                .color
                .as_deref()
                .and_then(colour_of)
                .unwrap_or_else(|| Color::from(Role::Muted));
            (svg, tone)
        });
        for suffix in &extension.suffixes {
            endings.insert(
                suffix.to_lowercase(),
                Known {
                    language: extension.id.clone(),
                    icon,
                },
            );
        }
    }
    if let Ok(mut learned) = LEARNED.lock() {
        learned.insert(host.clone(), endings);
    }
}

/// Takes in the icon themes installed on `host`, in place of those it had before; a later theme's icon for an ending replaces an earlier one's.
pub fn learn_icon_themes(host: &Option<String>, themes: &[ExtensionIconTheme]) {
    let mut endings = HashMap::new();
    for theme in themes {
        for icon in &theme.icons {
            let tone = icon
                .color
                .as_deref()
                .and_then(colour_of)
                .unwrap_or_else(|| Color::from(Role::Muted));
            let svg = Svg::arrived(
                &format!(
                    "icon-theme-{}-{}-{}",
                    theme.extension, theme.name, icon.suffix
                ),
                &icon.svg,
            );
            endings.insert(icon.suffix.to_lowercase(), (svg, tone));
        }
    }
    if let Ok(mut themed) = THEMED.lock() {
        themed.insert(host.clone(), endings);
    }
}

/// The ending of the name of the file `path`, in lower case; empty when it has none.
fn suffix_of(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, suffix)| suffix.to_lowercase())
        .unwrap_or_default()
}

/// What `read` finds in what the installed language of the file `path` names says of it, on the first machine where it finds something.
fn learned_of<T>(path: &str, read: impl Fn(&Known) -> Option<T>) -> Option<T> {
    let suffix = suffix_of(path);
    LEARNED.lock().ok().and_then(|learned| {
        learned
            .values()
            .find_map(|endings| endings.get(&suffix).and_then(&read))
    })
}

/// The installed language the file `path` names is written in, by its folder's name, when there is one.
pub fn file_language(path: &str) -> Option<String> {
    learned_of(path, |known| Some(known.language.clone()))
}

/// The icon and colour of the file `path` names: its installed language's, or a plain muted file.
pub fn file_kind(path: &str) -> (Svg, Color) {
    let suffix = suffix_of(path);
    let themed = THEMED.lock().ok().and_then(|themed| {
        themed
            .values()
            .find_map(|endings| endings.get(&suffix).copied())
    });
    themed
        .or_else(|| learned_of(path, |known| known.icon))
        .unwrap_or_else(|| (IconName::File.svg(), Color::from(Role::Muted)))
}

/// The icon of the file `path` names, `steps` on a side, tinted by its kind.
pub fn file_icon(path: &str, steps: f32) -> Node {
    let (svg, tone) = file_kind(path);
    node().styles([size(steps), text_color(tone)]).artwork(svg)
}

/// A folder, open or closed, `steps` on a side, in the folder colour.
pub fn folder_icon(open: bool, steps: f32) -> Node {
    let icon = if open {
        IconName::FolderOpen
    } else {
        IconName::Folder
    };
    glyph(icon, steps, Color::rgb(0xd9, 0xa9, 0x5b))
}
