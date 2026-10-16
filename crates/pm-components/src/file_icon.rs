//! File and folder icons: a file is shown with the icon of the installed language it is written in, and as a plain file otherwise; nothing language-specific ships with the app.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use pm_icon::{IconName, Svg};
use pm_protocol::ExtensionInfo;
use pm_style::{Color, Role, size, text_color};
use pm_ui::{Node, node};

use crate::parts::glyph;

/// The icons the installed languages of each machine show their files with: by machine, then by the ending of a file name.
type Learned = HashMap<Option<String>, HashMap<String, (Svg, Color)>>;

/// The icons learned from the languages installed on the machines reached.
static LEARNED: LazyLock<Mutex<Learned>> = LazyLock::new(Default::default);

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

/// Takes in the icons of the languages installed on `host`, as `extensions` lists them, in place of those it had before.
pub fn learn_icons(host: &Option<String>, extensions: &[ExtensionInfo]) {
    let mut endings = HashMap::new();
    for extension in extensions.iter().filter(|extension| extension.installed) {
        let Some(source) = &extension.icon else {
            continue;
        };
        let svg = Svg::arrived(&format!("extension-{}", extension.id), source);
        let tone = extension
            .color
            .as_deref()
            .and_then(colour_of)
            .unwrap_or_else(|| Color::from(Role::Muted));
        for suffix in &extension.suffixes {
            endings.insert(suffix.to_lowercase(), (svg, tone));
        }
    }
    if let Ok(mut learned) = LEARNED.lock() {
        learned.insert(host.clone(), endings);
    }
}

/// The icon and colour of the file `path` names: its installed language's, or a plain muted file.
pub fn file_kind(path: &str) -> (Svg, Color) {
    let suffix = path
        .rsplit(['/', '\\'])
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, suffix)| suffix.to_lowercase())
        .unwrap_or_default();
    LEARNED
        .lock()
        .ok()
        .and_then(|learned| {
            learned
                .values()
                .find_map(|endings| endings.get(&suffix).copied())
        })
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
