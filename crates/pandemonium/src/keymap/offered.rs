//! The keymaps on offer: the ones the editor ships and the reader's own.
//!
//! A keymap is a name, the keymap it builds on and what it changes there,
//! which is how both kinds are written down. The editor ships each of its
//! keymaps in full; a keymap the reader writes names any of them to build on.
//! Like the theme families, the list is installed once at launch and again
//! when the reader writes or reloads one, and everything that offers a
//! keymap reads it from here.

use std::sync::RwLock;

use crate::keymap::binding::Keymap;
use crate::keymap::changes::Changes;
use crate::keymap::context::keys;

/// Index of the keymap a first launch starts from.
pub const DEFAULT_KEYMAP: usize = 0;

/// How many keymaps a chain of `extends` is followed through before it is
/// taken to go round in a circle.
const DEEPEST: usize = 8;

/// The names earlier versions wrote the shipped keymaps down by, and the
/// names they go by now.
const FORMER_NAMES: &[(&str, &str)] = &[("VsCode", "VS Code"), ("SublimeText", "Sublime Text")];

/// A keymap as it is written: its name, what it builds on and what it changes.
#[derive(Clone, Debug)]
pub struct KeymapFile {
    /// The name the keymap is offered by.
    pub name: &'static str,
    /// The name of the keymap it builds on, if it builds on one.
    pub extends: Option<String>,
    /// The bindings it adds over that keymap, and the ones it takes away.
    pub changes: Changes,
    /// Whether the editor ships it, rather than the reader having written it.
    pub shipped: bool,
}

/// The keymaps on offer.
static OFFERED: RwLock<&'static [KeymapFile]> = RwLock::new(&[]);

/// Puts `files` on offer in place of whatever was offered before.
///
/// The handful of lists a session installs live as long as it does, so a
/// frame still holding the last one never sees it go.
pub fn install(files: Vec<KeymapFile>) {
    let offered = files.leak();
    if let Ok(mut keymaps) = OFFERED.write() {
        *keymaps = offered;
    }
}

/// Every keymap on offer, the shipped ones first.
pub fn keymaps() -> &'static [KeymapFile] {
    OFFERED.read().map_or(&[], |keymaps| *keymaps)
}

/// The index of the keymap called `name`, or called that once.
pub fn find(name: &str) -> Option<usize> {
    let named = FORMER_NAMES
        .iter()
        .find(|(former, _)| *former == name)
        .map_or(name, |(_, current)| current);
    keymaps().iter().position(|keymap| keymap.name == named)
}

/// The name of the keymap at `index`, or of the default one when there is
/// none there.
pub fn name(index: usize) -> &'static str {
    let keymaps = keymaps();
    keymaps
        .get(index)
        .or_else(|| keymaps.get(DEFAULT_KEYMAP))
        .map_or("", |keymap| keymap.name)
}

/// The keymap at `index` in full, with `changes` laid over it, settled for
/// the platform the editor runs on.
///
/// The keymaps it builds on are laid down first, from the one at the root
/// of the chain up, so that each one's changes land on what it named.
pub fn resolve(index: usize, changes: &Changes) -> Keymap {
    let mut keymap = Keymap::new();
    for file in chain(index).into_iter().rev() {
        file.changes.apply(&mut keymap);
    }
    changes.apply(&mut keymap);
    keymap.settle(keys::OS, platform())
}

/// The platform the editor runs on, as a `when` clause names it.
pub const fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// The keymap at `index` and every keymap it builds on, nearest first.
///
/// A name that is not on offer ends the chain where it is, and so does one
/// already in it: a keymap that builds on itself builds on nothing more.
fn chain(index: usize) -> Vec<&'static KeymapFile> {
    let keymaps = keymaps();
    let mut chain = Vec::new();
    let mut next = keymaps.get(index).or_else(|| keymaps.get(DEFAULT_KEYMAP));
    while let Some(file) = next {
        if chain.len() == DEEPEST
            || chain
                .iter()
                .any(|held: &&KeymapFile| std::ptr::eq(*held, file))
        {
            break;
        }
        chain.push(file);
        next = file
            .extends
            .as_deref()
            .and_then(find)
            .and_then(|index| keymaps.get(index));
    }
    chain
}
