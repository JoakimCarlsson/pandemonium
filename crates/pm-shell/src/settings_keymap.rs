//! The keymap page's rows: every action of the editor with its keys, in
//! groups, narrowed by what the page is searched for.

use pm_editor::{Action, shown};

use crate::settings_model::SettingsModel;

/// The groups of actions, in the order the page shows them.
const GROUPS: [&str; 11] = [
    "Window",
    "Session",
    "Source Control",
    "Tabs",
    "Navigation",
    "Code Intelligence",
    "Editing",
    "Selection",
    "Search",
    "File",
    "Terminal",
];

/// One action as the keymap page lists it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeymapRow {
    /// What the settings keep the action under.
    pub key: String,
    /// What it is called.
    pub title: String,
    /// Its keys, as they are shown.
    pub keys: Vec<String>,
    /// Whether the reader gave it keys of their own.
    pub own: bool,
    /// The other actions bound to the same keys.
    pub clashes: Vec<String>,
}

/// A group of actions as the keymap page lists them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeymapGroup {
    /// What the group is called.
    pub title: String,
    /// Its actions.
    pub rows: Vec<KeymapRow>,
}

/// Whether `action`, bound to `keys`, is one `needle`, lowercased, finds: by its name, its group, or its keys as they are written or shown.
fn finds(action: Action, keys: &[String], needle: &str) -> bool {
    needle.is_empty()
        || action.title().to_lowercase().contains(needle)
        || action.group().to_lowercase().contains(needle)
        || keys.iter().any(|binding| {
            binding.to_lowercase().contains(needle)
                || shown(binding).to_lowercase().contains(needle)
        })
}

impl SettingsModel {
    /// The actions the keymap page lists, in their groups, narrowed by what it is searched for.
    pub fn keymap_groups(&self) -> Vec<KeymapGroup> {
        let keymap = self.editor.keymap();
        let needle = self.page.keymap_find.trim().to_lowercase();
        GROUPS
            .iter()
            .map(|group| KeymapGroup {
                title: (*group).to_owned(),
                rows: Action::ALL
                    .into_iter()
                    .filter(|action| action.group() == *group)
                    .filter_map(|action| {
                        let keys = keymap.keys_of(action);
                        finds(action, &keys, &needle).then(|| KeymapRow {
                            key: action.key().to_owned(),
                            title: action.title().to_owned(),
                            own: self.editor.keys.contains_key(action.key()),
                            clashes: keys
                                .iter()
                                .flat_map(|binding| keymap.clashes(action, binding))
                                .map(|other| other.title().to_owned())
                                .collect(),
                            keys: keys.iter().map(|binding| shown(binding)).collect(),
                        })
                    })
                    .collect(),
            })
            .filter(|group| !group.rows.is_empty())
            .collect()
    }
}
