//! Where the editor keeps what it owns.
//!
//! One directory holds everything the editor writes for itself — the
//! preferences today, a log or a worktree root later — so nothing has to
//! rediscover where "our" files live.

use std::fs;
use std::path::{Path, PathBuf};

/// The environment variable that moves the editor's home somewhere else.
const HOME_VARIABLE: &str = "PANDEMONIUM_HOME";

/// The editor's home under the user's home directory.
const HOME_DIRECTORY: &str = ".pandemonium";

/// The file the preferences are written to, inside the editor's home.
const SETTINGS_FILE: &str = "settings.yaml";

/// The directory themes are read from, inside the editor's home.
const THEMES_DIRECTORY: &str = "themes";

/// The directory keymaps are read from, inside the editor's home.
const KEYMAPS_DIRECTORY: &str = "keymaps";

/// The directory installed extensions live in.
const EXTENSIONS_DIRECTORY: &str = "extensions";

/// The directory session worktrees are cut into, inside the editor's home.
const WORKTREES_DIRECTORY: &str = "worktrees";

/// The directory editor-managed language servers live in.
const SERVERS_DIRECTORY: &str = "servers";

/// The directory agents downloaded from the registry live in, inside the editor's home.
const AGENTS_DIRECTORY: &str = "agents";

/// The directory agents downloaded from the registry are unpacked in.
pub fn agents() -> Option<PathBuf> {
    home().map(|home| home.join(AGENTS_DIRECTORY))
}

/// The directory language servers' logs are written in.
const LOGS_DIRECTORY: &str = "logs";

/// The editor's home: `PANDEMONIUM_HOME`, else `~/.pandemonium`.
pub fn home() -> Option<PathBuf> {
    match std::env::var_os(HOME_VARIABLE) {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => std::env::home_dir().map(|home| home.join(HOME_DIRECTORY)),
    }
}

/// The file the preferences live in, when the platform tells us where that is.
pub fn settings() -> Option<PathBuf> {
    home().map(|home| home.join(SETTINGS_FILE))
}

/// The directory a reader's own themes live in.
pub fn themes() -> Option<PathBuf> {
    home().map(|home| home.join(THEMES_DIRECTORY))
}

/// The directory a reader's own keymaps live in.
pub fn keymaps() -> Option<PathBuf> {
    home().map(|home| home.join(KEYMAPS_DIRECTORY))
}

/// The directory installed extensions live in.
pub fn extensions() -> Option<PathBuf> {
    home().map(|home| home.join(EXTENSIONS_DIRECTORY))
}

/// The text of every file in `directory` with `extension`, in the order
/// their names sort.
///
/// A directory or a file that will not read is passed over: what the reader
/// keeps there is read when it can be and never stops the editor opening.
pub fn texts(directory: Option<PathBuf>, extension: &str) -> Vec<String> {
    let Some(entries) = directory.and_then(|directory| fs::read_dir(directory).ok()) else {
        return Vec::new();
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == extension))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect()
}

/// A file in `directory`, made if it is missing, named after `name` with
/// `extension` and numbered past any file already there.
///
/// What the reader names is written beside what is there rather than over
/// it; a name with nothing to make a file name of is called `fallback`.
pub fn unused_file(
    directory: &Path,
    name: &str,
    fallback: &str,
    extension: &str,
) -> Option<PathBuf> {
    fs::create_dir_all(directory).ok()?;
    let stem = match slug(name) {
        stem if stem.is_empty() => fallback.to_owned(),
        stem => stem,
    };
    (1..)
        .map(|count| match count {
            1 => directory.join(format!("{stem}.{extension}")),
            _ => directory.join(format!("{stem}-{count}.{extension}")),
        })
        .find(|path| !path.exists())
}

/// `name` as the stem of a file: lower case, words joined by hyphens.
fn slug(name: &str) -> String {
    name.split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// The directory session worktrees are cut into, one per project.
///
/// A worktree never lands beside the repository it was cut from: the project
/// on disk is the truth, and what the editor makes for itself lives in the
/// editor's own home.
pub fn worktrees() -> Option<PathBuf> {
    home().map(|home| home.join(WORKTREES_DIRECTORY))
}

/// The directory editor-managed language servers live in.
pub fn servers() -> Option<PathBuf> {
    home().map(|home| home.join(SERVERS_DIRECTORY))
}

/// The directory language servers' logs are written in.
pub fn logs() -> Option<PathBuf> {
    home().map(|home| home.join(LOGS_DIRECTORY))
}

/// The directory pasted images are kept in while agents need their file links.
pub fn clipboard() -> Option<PathBuf> {
    home().map(|home| home.join("clipboard"))
}
