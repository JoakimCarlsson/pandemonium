//! The preferences on disk, and the one seam they pass through.
//!
//! Every preference the editor remembers between launches lives in one file:
//! [`load`] answers what the last launch left behind, [`save`] records what
//! this one decided. Onboarding writes through here on its first run and a
//! settings pane edits the same file — neither keeps a store of
//! its own. Themes the reader wrote are read from the same home and put on
//! offer beside the built-in ones, and a theme the reader makes in the
//! settings pane is written there. Nothing fails loudly: a missing, unreadable
//! or outdated file is a first launch, and a write that cannot land leaves the
//! running editor alone.

mod fonts;
mod overrides;
mod paths;
mod preferences;
mod stored;
mod theme;
mod tokens;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::panes::Saved;
use crate::workspace::Layout;
use stored::Stored;

pub use fonts::FontSlot;
pub use overrides::ThemeOverrides;
pub use paths::{settings as settings_file, themes as themes_directory, worktrees};
pub use preferences::{Preference, Preferences, Step, ThemeMode, WorktreePaths};
pub use tokens::{Group, TOKENS, from_hex, hex, in_group};

/// The window's own size and state, as a launch leaves it.
#[derive(Clone, Copy, Debug)]
pub struct WindowState {
    /// Logical width of the window when it is not maximized.
    pub width: f32,
    /// Logical height of the window when it is not maximized.
    pub height: f32,
    /// Whether the window fills the screen it is on.
    pub maximized: bool,
}

impl Default for WindowState {
    /// The window a first launch opens.
    fn default() -> Self {
        Self {
            width: 1440.0,
            height: 900.0,
            maximized: false,
        }
    }
}

/// Everything a launch picks up where the one before it left off.
#[derive(Debug, Default)]
pub struct Restored {
    /// The preferences the editor draws and behaves by.
    pub preferences: Preferences,
    /// Whether the first run's setup has been finished.
    pub onboarded: bool,
    /// The roots of the projects the window had open.
    pub projects: Vec<PathBuf>,
    /// The root of the project the window was pointed at.
    pub active: Option<PathBuf>,
    /// Which regions the window showed, and how large they were.
    pub layout: Layout,
    /// How the window was divided into panes, and what was open in them.
    pub panes: Saved,
    /// The size and state of the window itself.
    pub window: WindowState,
    /// The servers to run for a language, in place of the ones it names.
    pub language_servers: BTreeMap<String, Vec<pm_text::Server>>,
}

/// What the last launch left behind, or a first launch's defaults.
///
/// The reader's own themes go on offer before the file is read, because the
/// family it names is resolved against the themes there are: a launch that
/// read the preferences first could not find a theme it had not loaded yet.
pub fn load() -> Restored {
    pm_ui::install_themes(theme::installed());
    paths::settings()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_norway::from_str::<Stored>(&text).ok())
        .map(Stored::into_restored)
        .unwrap_or_default()
}

/// Reads the reader's themes in again, still drawing in the family
/// `preferences` names wherever it now sits among them.
pub fn reload_themes(preferences: &mut Preferences) {
    let drawn = pm_ui::family(preferences.theme_family).name;
    pm_ui::install_themes(theme::installed());
    preferences.theme_family = stored::family_index(drawn).unwrap_or(pm_ui::DEFAULT_FAMILY);
}

/// Writes the family `preferences` draw in, with their overrides painted
/// over it, as a theme of the reader's own called `name`, and draws in it
/// from now on, saying whether it could be written.
///
/// The overrides are the theme now, so they are cleared rather than being
/// painted over it a second time. A name a family already has is numbered
/// rather than shadowing that family.
pub fn save_theme(preferences: &mut Preferences, name: &str) -> bool {
    let base = pm_ui::family(preferences.theme_family);
    let overrides = &preferences.theme_overrides;
    let name = unused_family_name(name.trim());
    let (dark, light) = (overrides.apply(base.dark), overrides.apply(base.light));
    if theme::write(&name, &dark, &light).is_none() {
        return false;
    }
    pm_ui::install_themes(theme::installed());
    preferences.theme_family = stored::family_index(&name).unwrap_or(preferences.theme_family);
    preferences.theme_overrides = ThemeOverrides::default();
    true
}

/// `wanted`, or `wanted` numbered, whichever no family on offer is called.
fn unused_family_name(wanted: &str) -> String {
    let wanted = match wanted.is_empty() {
        true => "My Theme",
        false => wanted,
    };
    let taken = |name: &str| pm_ui::families().iter().any(|family| family.name == name);
    (1..)
        .map(|count| match count {
            1 => wanted.to_owned(),
            _ => format!("{wanted} {count}"),
        })
        .find(|name| !taken(name))
        .unwrap_or_else(|| wanted.to_owned())
}

/// Writes the window down, ignoring a file system that will not have it.
pub fn save(restored: &Restored) {
    let Some(path) = paths::settings() else {
        return;
    };
    let Ok(text) = serde_norway::to_string(&Stored::of(restored)) else {
        return;
    };
    if let Some(directory) = path.parent()
        && fs::create_dir_all(directory).is_err()
    {
        return;
    }
    let _ = fs::write(path, text);
}
