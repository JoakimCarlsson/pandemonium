//! Where the editor keeps what it owns.
//!
//! One directory holds everything the editor writes for itself — the
//! preferences today, a log or a worktree root later — so nothing has to
//! rediscover where "our" files live.

use std::path::PathBuf;

/// The environment variable that moves the editor's home somewhere else.
const HOME_VARIABLE: &str = "PANDEMONIUM_HOME";

/// The editor's home under the user's home directory.
const HOME_DIRECTORY: &str = ".pandemonium";

/// The file the preferences are written to, inside the editor's home.
const SETTINGS_FILE: &str = "settings.yaml";

/// The directory themes are read from, inside the editor's home.
const THEMES_DIRECTORY: &str = "themes";

/// The directory session worktrees are cut into, inside the editor's home.
const WORKTREES_DIRECTORY: &str = "worktrees";

/// The editor's home: `PANDEMONIUM_HOME`, else `~/.pandemonium`.
pub fn home() -> Option<PathBuf> {
    match std::env::var_os(HOME_VARIABLE) {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => std::env::var_os("HOME").map(|home| PathBuf::from(home).join(HOME_DIRECTORY)),
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

/// The directory session worktrees are cut into, one per project.
///
/// A worktree never lands beside the repository it was cut from: the project
/// on disk is the truth, and what the editor makes for itself lives in the
/// editor's own home.
pub fn worktrees() -> Option<PathBuf> {
    home().map(|home| home.join(WORKTREES_DIRECTORY))
}
