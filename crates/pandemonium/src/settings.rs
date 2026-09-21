//! Where the setup screen's decisions are kept between launches.
//!
//! The file is the one seam settings pass through: [`load`] answers what the
//! last launch left behind, [`save`] records what this one decided. Neither
//! fails loudly — a missing, unreadable or outdated file is a first launch,
//! and a write that cannot land leaves the running editor alone.

use std::fs;
use std::path::PathBuf;

use pm_ui::FAMILIES;
use serde::{Deserialize, Serialize};

use crate::keymap::BaseKeymap;
use crate::onboarding::{Setup, ThemeMode};

/// The environment variable that moves the editor's home somewhere else.
const HOME_VARIABLE: &str = "PANDEMONIUM_HOME";

/// The editor's home under the user's home directory.
const HOME_DIRECTORY: &str = ".pandemonium";

/// The file the settings are written to, inside the editor's home.
const FILE: &str = "settings.yaml";

/// The settings as they are written down.
///
/// Distinct from [`Setup`] so the file survives the in-memory shape changing:
/// the theme family is stored by name rather than by its index into
/// [`FAMILIES`], and every field is optional so an older file still loads.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Stored {
    /// Which theme the editor draws in.
    theme_mode: Option<ThemeMode>,
    /// The name of the theme family the editor draws in.
    theme_family: Option<String>,
    /// The keymap the editor starts from.
    keymap: Option<BaseKeymap>,
    /// Whether editing starts in vim mode.
    vim_mode: Option<bool>,
    /// Whether a new session's worktree is trusted without being asked about.
    trust_worktrees: Option<bool>,
    /// Whether anonymous usage data is sent.
    metrics: Option<bool>,
    /// Whether crash reports are sent.
    crash_reports: Option<bool>,
    /// Whether setup has been finished, which swaps the page.
    finished: Option<bool>,
}

impl Stored {
    /// The settings this file stands for, defaulting anything it leaves out.
    fn into_setup(self) -> Setup {
        let defaults = Setup::default();
        Setup {
            theme_mode: self.theme_mode.unwrap_or(defaults.theme_mode),
            theme_family: self
                .theme_family
                .as_deref()
                .and_then(family_index)
                .unwrap_or(defaults.theme_family),
            keymap: self.keymap.unwrap_or(defaults.keymap),
            vim_mode: self.vim_mode.unwrap_or(defaults.vim_mode),
            trust_worktrees: self.trust_worktrees.unwrap_or(defaults.trust_worktrees),
            metrics: self.metrics.unwrap_or(defaults.metrics),
            crash_reports: self.crash_reports.unwrap_or(defaults.crash_reports),
            finished: self.finished.unwrap_or(defaults.finished),
        }
    }
}

impl From<&Setup> for Stored {
    /// The file to write for these settings.
    fn from(setup: &Setup) -> Self {
        Self {
            theme_mode: Some(setup.theme_mode),
            theme_family: Some(pm_ui::family(setup.theme_family).name.to_owned()),
            keymap: Some(setup.keymap),
            vim_mode: Some(setup.vim_mode),
            trust_worktrees: Some(setup.trust_worktrees),
            metrics: Some(setup.metrics),
            crash_reports: Some(setup.crash_reports),
            finished: Some(setup.finished),
        }
    }
}

/// The index into [`FAMILIES`] of the family called `name`.
fn family_index(name: &str) -> Option<usize> {
    FAMILIES.iter().position(|family| family.name == name)
}

/// The editor's home: `PANDEMONIUM_HOME`, else `~/.pandemonium`.
///
/// Everything the editor keeps for itself lives under this one directory, the
/// way `.claude` and `.wasa` do, so a settings file, a log and a worktree root
/// are all one path away from each other.
fn home() -> Option<PathBuf> {
    match std::env::var_os(HOME_VARIABLE) {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => std::env::var_os("HOME").map(|home| PathBuf::from(home).join(HOME_DIRECTORY)),
    }
}

/// The file the settings live in, when the platform tells us where that is.
fn path() -> Option<PathBuf> {
    home().map(|home| home.join(FILE))
}

/// The settings the last launch left behind, or the ones a first launch starts
/// from.
pub fn load() -> Setup {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_norway::from_str::<Stored>(&text).ok())
        .map(Stored::into_setup)
        .unwrap_or_default()
}

/// Writes `setup` down, ignoring a file system that will not have it.
pub fn save(setup: &Setup) {
    let Some(path) = path() else {
        return;
    };
    let Ok(text) = serde_norway::to_string(&Stored::from(setup)) else {
        return;
    };
    if let Some(directory) = path.parent()
        && fs::create_dir_all(directory).is_err()
    {
        return;
    }
    let _ = fs::write(path, text);
}
