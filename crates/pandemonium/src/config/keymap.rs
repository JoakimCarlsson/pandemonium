//! The shape a keymap takes on disk, how one is read in, and how one is
//! written out.
//!
//! A keymap file names itself, may name a keymap it builds on, and lists
//! its bindings in blocks, one `when` clause to a block:
//!
//! ```yaml
//! name: My Keymap
//! extends: VS Code
//! bindings:
//!   - when: editor.focused
//!     keys:
//!       primary+shift+k: edit.delete_line
//!       ctrl+t: null
//! unbind:
//!   - keys: primary+d
//!     action: edit.add_next_match
//! ```
//!
//! A sequence bound to `null` does nothing where its clause holds; an
//! `unbind` entry takes one action off one sequence wherever it was bound.
//! The keymaps the editor ships are written the same way and compiled in,
//! and the reader's own bindings in the preferences are the same blocks
//! without the name. A binding that will not read is skipped, like a colour
//! in a theme that will not: a keymap half written must not stop the editor
//! opening.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_norway::{Mapping, Value};

use crate::config::paths;
use crate::keymap::{Action, Binding, Changes, KeymapFile, Sequence, When};

/// The extension a keymap file has.
const EXTENSION: &str = "yaml";

/// The keymaps the editor ships, in the order they are offered.
const SHIPPED: [&str; 7] = [
    include_str!("../../keymaps/pandemonium.yaml"),
    include_str!("../../keymaps/vs-code.yaml"),
    include_str!("../../keymaps/zed.yaml"),
    include_str!("../../keymaps/jetbrains.yaml"),
    include_str!("../../keymaps/emacs.yaml"),
    include_str!("../../keymaps/helix.yaml"),
    include_str!("../../keymaps/sublime-text.yaml"),
];

/// A keymap as it is written down.
#[derive(Debug, Deserialize, Serialize)]
struct StoredKeymap {
    /// The name the keymap is offered by.
    name: String,
    /// The keymap it builds on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extends: Option<String>,
    /// The bindings it lays over that keymap.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bindings: Vec<StoredBlock>,
    /// The bindings it takes out of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unbind: Vec<StoredUnbind>,
}

/// The reader's own bindings, as the preferences write them down.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct StoredChanges {
    /// The bindings laid over the chosen keymap.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bindings: Vec<StoredBlock>,
    /// The bindings taken out of it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unbind: Vec<StoredUnbind>,
}

/// Bindings that share a `when` clause, as they are written down.
#[derive(Debug, Deserialize, Serialize)]
struct StoredBlock {
    /// Where the bindings apply; everywhere when left out.
    #[serde(default = "everywhere")]
    when: String,
    /// Each sequence, and the action it is bound to or `null`.
    keys: Mapping,
}

/// One action taken off one sequence, as it is written down.
#[derive(Debug, Deserialize, Serialize)]
struct StoredUnbind {
    /// The sequence.
    keys: String,
    /// The action it no longer does.
    action: String,
}

/// The keymaps the editor ships.
///
/// # Panics
///
/// Panics if one of them will not parse as a keymap at all, which is a file
/// compiled into the binary that nothing running can repair.
pub(super) fn shipped() -> Vec<KeymapFile> {
    SHIPPED
        .iter()
        .map(|text| {
            serde_norway::from_str::<StoredKeymap>(text)
                .unwrap_or_else(|error| panic!("a shipped keymap does not parse: {error}"))
                .into_file(true)
        })
        .collect()
}

/// Every keymap written in the editor's home, in the order their files sort.
pub(super) fn installed() -> Vec<KeymapFile> {
    paths::texts(paths::keymaps(), EXTENSION)
        .into_iter()
        .chain(super::extensions::keymaps())
        .filter_map(|text| serde_norway::from_str::<StoredKeymap>(&text).ok())
        .map(|stored| stored.into_file(false))
        .collect()
}

/// Writes a keymap called `name` that builds on `extends` with `changes`
/// into the editor's home, saying where it went.
pub(super) fn write(name: &str, extends: &str, changes: &Changes) -> Option<PathBuf> {
    let path = paths::unused_file(&paths::keymaps()?, name, "keymap", EXTENSION)?;
    let written = StoredChanges::of(changes);
    let keymap = StoredKeymap {
        name: name.to_owned(),
        extends: Some(extends.to_owned()),
        bindings: written.bindings,
        unbind: written.unbind,
    };
    let text = serde_norway::to_string(&keymap).ok()?;
    fs::write(&path, text).ok()?;
    Some(path)
}

impl StoredKeymap {
    /// The keymap this file describes.
    fn into_file(self, shipped: bool) -> KeymapFile {
        KeymapFile {
            name: self.name.leak(),
            extends: self.extends,
            changes: changes_of(&self.bindings, &self.unbind),
            shipped,
        }
    }
}

impl StoredChanges {
    /// The bindings this stands for, less any it cannot read.
    pub(super) fn into_changes(self) -> Changes {
        changes_of(&self.bindings, &self.unbind)
    }

    /// How `changes` are written down: runs of bindings under one clause
    /// as a block each, in the order they were made.
    pub(super) fn of(changes: &Changes) -> Self {
        let mut bindings: Vec<StoredBlock> = Vec::new();
        for binding in &changes.bindings {
            let when = binding.when.to_string();
            let action = binding
                .action
                .map_or(Value::Null, |action| Value::String(action.id().to_owned()));
            let sequence = Value::String(binding.sequence.to_string());
            match bindings.last_mut() {
                Some(block) if block.when == when => {
                    block.keys.insert(sequence, action);
                }
                _ => {
                    let mut keys = Mapping::new();
                    keys.insert(sequence, action);
                    bindings.push(StoredBlock { when, keys });
                }
            }
        }
        let unbind = changes
            .removed
            .iter()
            .map(|(sequence, action)| StoredUnbind {
                keys: sequence.to_string(),
                action: action.id().to_owned(),
            })
            .collect();
        Self { bindings, unbind }
    }

    /// Whether nothing is bound and nothing taken away.
    pub(super) fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.unbind.is_empty()
    }
}

/// The changes `bindings` and `unbind` write, less any entry that will not
/// read.
fn changes_of(bindings: &[StoredBlock], unbind: &[StoredUnbind]) -> Changes {
    let bindings = bindings
        .iter()
        .flat_map(|block| {
            block.keys.iter().filter_map(|(sequence, action)| {
                let action = match action {
                    Value::Null => None,
                    Value::String(action) => Some(action.as_str()),
                    _ => return None,
                };
                Binding::parse(sequence.as_str()?, action, &block.when).ok()
            })
        })
        .collect();
    let removed = unbind
        .iter()
        .filter_map(|entry| {
            Some((
                entry.keys.parse::<Sequence>().ok()?,
                entry.action.parse::<Action>().ok()?,
            ))
        })
        .collect();
    Changes { bindings, removed }
}

/// The clause a block written without one applies in.
fn everywhere() -> String {
    When::Always.to_string()
}
