//! The preferences on disk, and the one seam they pass through.
//!
//! Every preference the editor remembers between launches lives in one file:
//! [`load`] answers what the last launch left behind, [`save`] records what
//! this one decided. Onboarding writes through here on its first run and a
//! settings pane edits the same file — neither keeps a store of
//! its own. Themes and keymaps the reader wrote are read from the same home
//! and put on offer beside the built-in ones, and a theme or a keymap the
//! reader makes in the settings pane is written there. Nothing fails loudly: a missing, unreadable
//! or outdated file is a first launch, and a write that cannot land leaves the
//! running editor alone.

mod extensions;
mod fonts;
mod keymap;
mod overrides;
mod paths;
mod preferences;
mod stored;
mod theme;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::panes::Saved;
use crate::terminal::SavedShell;
use crate::workspace::Layout;
use stored::Stored;

pub use fonts::FontSlot;
pub use overrides::ThemeOverrides;
pub use paths::{
    keymaps as keymaps_directory, logs, servers, settings as settings_file,
    themes as themes_directory, worktrees,
};
pub use preferences::{
    AgentOptions, EditPredictions, InstallLanguageServers, KnobValue, Preference, Preferences,
    Step, ThemeMode, VimBinding, WorktreePaths,
};

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
    /// The shells the window had running, and what they were called.
    pub shells: Vec<SavedShell>,
    /// The size and state of the window itself.
    pub window: WindowState,
    /// The servers a language runs, in place of the ones it names or after them.
    pub language_servers: BTreeMap<String, ServerList>,
    /// The agents the reader added, beside the ones the editor ships.
    pub agent_servers: Vec<pm_acp::Agent>,
    /// The tool servers every agent is opened with.
    pub mcp_servers: Vec<pm_acp::McpServer>,
}

/// The servers a reader configured for one language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerList {
    /// These run instead of the servers the language names.
    Replace(Vec<pm_text::Server>),
    /// These run after the servers the language names.
    Add(Vec<pm_text::Server>),
}

/// What the last launch left behind, or a first launch's defaults.
///
/// The reader's own themes and keymaps go on offer before the file is read,
/// because the family and the keymap it names are resolved against the ones
/// there are: a launch that read the preferences first could not find a
/// theme it had not loaded yet.
pub fn load() -> Restored {
    extensions::reload();
    install_themes();
    install_keymaps();
    let restored = paths::settings()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_norway::from_str::<Stored>(&text).ok())
        .map(Stored::into_restored)
        .unwrap_or_default();
    pm_acp::install(restored.agent_servers.clone());
    pm_acp::install_mcp(restored.mcp_servers.clone());
    restored
}

/// Reads extensions again and immediately follows their theme and keymap changes.
pub fn reload_extensions(preferences: &mut Preferences) -> Vec<String> {
    extensions::reload();
    reload_themes(preferences);
    reload_keymaps(preferences);
    extensions::take_errors()
}

/// Errors from extensions read during launch.
pub fn take_extension_errors() -> Vec<String> {
    extensions::take_errors()
}

/// The language servers the settings file names now, read afresh, for a
/// file the reader has just saved.
pub fn language_servers() -> Option<BTreeMap<String, ServerList>> {
    let text = fs::read_to_string(paths::settings()?).ok()?;
    let stored = serde_norway::from_str::<Stored>(&text).ok()?;
    Some(stored.into_restored().language_servers)
}

/// Reads the reader's themes in again, still drawing in the family
/// `preferences` names wherever it now sits among them.
pub fn reload_themes(preferences: &mut Preferences) {
    let drawn = crate::theme::family(preferences.theme_family).name;
    install_themes();
    preferences.theme_family = crate::theme::find(drawn).unwrap_or(crate::theme::DEFAULT_FAMILY);
}

/// Reads the reader's keymaps in again, still pressing the keymap
/// `preferences` names wherever it now sits among them.
pub fn reload_keymaps(preferences: &mut Preferences) {
    let pressed = crate::keymap::name(preferences.keymap);
    install_keymaps();
    preferences.keymap = crate::keymap::find(pressed).unwrap_or(crate::keymap::DEFAULT_KEYMAP);
}

/// Writes the keymap `preferences` press, with the reader's own bindings
/// over it, as a keymap of the reader's own called `name` that builds on it,
/// and presses that one from now on, saying whether it could be written.
///
/// The reader's bindings are the keymap now, so they are cleared rather
/// than being laid over it a second time.
pub fn save_keymap(preferences: &mut Preferences, name: &str) -> bool {
    let taken = |name: &str| crate::keymap::find(name).is_some();
    let name = unused_name(name.trim(), "My Keymap", taken);
    let extends = crate::keymap::name(preferences.keymap);
    if keymap::write(&name, extends, &preferences.keybindings).is_none() {
        return false;
    }
    install_keymaps();
    preferences.keymap = crate::keymap::find(&name).unwrap_or(preferences.keymap);
    preferences.keybindings = crate::keymap::Changes::default();
    true
}

/// Every keymap on offer, by name, with the message that picks it.
pub fn keymap_choices() -> impl Iterator<Item = (String, crate::message::Message)> {
    crate::keymap::keymaps()
        .iter()
        .enumerate()
        .map(|(index, keymap)| {
            (
                keymap.name.to_owned(),
                crate::message::Message::SetKeymap(index),
            )
        })
}

/// Puts the shipped keymaps on offer, and the reader's after them.
fn install_keymaps() {
    let mut keymaps = keymap::shipped();
    keymaps.extend(keymap::installed());
    crate::keymap::install(keymaps);
}

/// Puts the shipped theme families on offer, and the reader's after them.
fn install_themes() {
    let mut themes = theme::shipped();
    themes.extend(theme::installed());
    crate::theme::install(themes);
}

/// Writes the family `preferences` draw in, with their overrides painted
/// over it, as a theme of the reader's own called `name` that builds on it,
/// and draws in it from now on, saying whether it could be written.
///
/// The overrides are the theme now, so they are cleared rather than being
/// painted over it a second time. A name a family already has is numbered
/// rather than shadowing that family.
pub fn save_theme(preferences: &mut Preferences, name: &str) -> bool {
    let taken = |name: &str| crate::theme::find(name).is_some();
    let name = unused_name(name.trim(), "My Theme", taken);
    let extends = crate::theme::family(preferences.theme_family).name;
    if theme::write(&name, extends, &preferences.theme_overrides).is_none() {
        return false;
    }
    install_themes();
    preferences.theme_family = crate::theme::find(&name).unwrap_or(preferences.theme_family);
    preferences.theme_overrides = ThemeOverrides::default();
    true
}

/// `wanted`, or `fallback` when nothing is wanted, numbered until it is a
/// name nothing is `taken` by.
fn unused_name(wanted: &str, fallback: &str, taken: impl Fn(&str) -> bool) -> String {
    let wanted = match wanted.is_empty() {
        true => fallback,
        false => wanted,
    };
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
