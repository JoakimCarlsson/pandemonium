//! The preferences on disk, and the one seam they pass through.
//!
//! Every preference the editor remembers between launches lives in one file:
//! [`load`] answers what the last launch left behind, [`save`] records what
//! this one decided. Onboarding writes through here on its first run and a
//! settings surface will edit the same file later — neither keeps a store of
//! its own. Themes the reader wrote are read from the same home and put on
//! offer beside the built-in ones. Nothing fails loudly: a missing, unreadable
//! or outdated file is a first launch, and a write that cannot land leaves the
//! running editor alone.

mod paths;
mod stored;
mod theme;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::onboarding::Setup;
use crate::panes::Saved;
use crate::workspace::Layout;
use stored::Stored;

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
    pub setup: Setup,
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
