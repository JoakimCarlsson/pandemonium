//! File and folder icons, and which installed language a file is written in: a file is shown with the icon of that language, and as a plain file otherwise; nothing language-specific ships with the app.

use rustle::phosphor;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use pm_protocol::{ExtensionIconTheme, ExtensionInfo};
use rustle::Modifier;

use crate::icons::icon;
use crate::icons::{Artwork, artwork_with, icon_with};
use crate::theme::{Role, Tone, sp};

/// What an installed language says of the files whose names end as it reads.
#[derive(Clone)]
struct Known {
    /// The language, by its folder's name.
    language: String,
    /// The icon and colour its files are shown with, when it has one.
    icon: Option<(Artwork, Tone)>,
}

/// What the installed languages of one machine say of their files: by the ending of a file name, and by the whole names and name patterns that are read before an ending is.
#[derive(Default)]
struct Machine {
    /// By the ending of a file name, in lower case.
    endings: HashMap<String, Known>,
    /// Each language that reads files by their whole name or a pattern of it, as the listing describes it with its endings left out.
    named: Vec<(ExtensionInfo, Known)>,
}

/// What the installed languages of each machine say of their files, by machine.
type Learned = HashMap<Option<String>, Machine>;

/// What was learned from the languages installed on the machines reached.
static LEARNED: LazyLock<Mutex<Learned>> = LazyLock::new(Default::default);

/// The icons the icon themes of each machine give the endings of file names: by machine, then by ending.
type Themed = HashMap<Option<String>, HashMap<String, (Artwork, Tone)>>;

/// What was learned from the icon themes installed on the machines reached.
static THEMED: LazyLock<Mutex<Themed>> = LazyLock::new(Default::default);

/// The colour `hex`, written `#rrggbb`, names.
fn colour_of(hex: &str) -> Option<Tone> {
    let digits = hex.strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16)
        .ok()
        .filter(|_| digits.len() == 6)?;
    Some(Tone::rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

/// Takes in the languages installed on `host` and their icons, as `extensions` lists them, in place of those it had before.
pub fn learn_languages(host: &Option<String>, extensions: &[ExtensionInfo]) {
    let mut machine = Machine::default();
    for extension in extensions.iter().filter(|extension| extension.installed) {
        let icon = extension.icon.as_ref().map(|source| {
            let svg = Artwork::arrived(&format!("extension-{}", extension.id), source);
            let tone = extension
                .color
                .as_deref()
                .and_then(colour_of)
                .unwrap_or_else(|| Tone::from(Role::Muted));
            (svg, tone)
        });
        let known = Known {
            language: extension.id.clone(),
            icon,
        };
        for suffix in &extension.suffixes {
            machine.endings.insert(suffix.to_lowercase(), known.clone());
        }
        if !extension.names.is_empty() || !extension.patterns.is_empty() {
            machine.named.push((
                ExtensionInfo {
                    suffixes: Vec::new(),
                    ..extension.clone()
                },
                known,
            ));
        }
    }
    if let Ok(mut learned) = LEARNED.lock() {
        learned.insert(host.clone(), machine);
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
                .unwrap_or_else(|| Tone::from(Role::Muted));
            let svg = Artwork::arrived(
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
        learned.values().find_map(|machine| {
            let named = machine
                .named
                .iter()
                .find(|(info, _)| info.reads(path))
                .and_then(|(_, known)| read(known));
            named.or_else(|| machine.endings.get(&suffix).and_then(&read))
        })
    })
}

/// The installed language the file `path` names is written in, by its folder's name, when there is one.
pub fn file_language(path: &str) -> Option<String> {
    learned_of(path, |known| Some(known.language.clone()))
}

/// The icon and colour of the file `path` names: its installed language's or its icon theme's artwork, or no artwork, meaning the plain file icon, in muted.
pub fn file_kind(path: &str) -> (Option<Artwork>, Tone) {
    let suffix = suffix_of(path);
    let themed = THEMED.lock().ok().and_then(|themed| {
        themed
            .values()
            .find_map(|endings| endings.get(&suffix).copied())
    });
    themed
        .or_else(|| learned_of(path, |known| known.icon))
        .map_or((None, Tone::from(Role::Muted)), |(art, tone)| {
            (Some(art), tone)
        })
}

/// Emits the icon of the file `path` names, `steps` on a side, tinted by its kind.
#[track_caller]
pub fn file_icon(path: &str, steps: f32) {
    match file_kind(path) {
        (Some(art), tone) => artwork_with(Modifier::new(), art, sp(steps), tone),
        (None, tone) => icon_with(Modifier::new(), phosphor::FILE, sp(steps), tone),
    }
}

/// Emits a folder, open or closed, `steps` on a side, in the folder colour.
#[track_caller]
pub fn folder_icon(open: bool, steps: f32) {
    let name = if open {
        phosphor::FOLDER_OPEN
    } else {
        phosphor::FOLDER
    };
    icon(name, steps, Tone::rgb(0xd9, 0xa9, 0x5b));
}
