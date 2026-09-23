//! The preferences the editor draws and behaves by, and the messages that
//! change them.
//!
//! Onboarding and the settings pane are two editors of this one value: both
//! build their screens from it, both answer with the same [`Message`], and
//! [`Preferences::apply`] is the one place either of them changes it.

use std::path::PathBuf;

use pm_core::Bootstrap;
use pm_ui::{Appearance, DEFAULT_FAMILY, families};
use serde::{Deserialize, Serialize};

use crate::keymap::BaseKeymap;
use crate::message::Message;

/// Which theme the editor draws in.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ThemeMode {
    /// Always the light theme.
    Light,
    /// Always the dark theme.
    Dark,
    /// Whichever one the desktop is set to.
    System,
}

impl ThemeMode {
    /// Every mode, in the order a toggle offers them.
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    /// The mode's label in the theme toggle.
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
        }
    }

    /// The appearance to draw in, given what the system asks for.
    pub fn resolve(self, system: Appearance) -> Appearance {
        match self {
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
            Self::System => system,
        }
    }
}

/// One preference, named so it can be put back to its default on its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preference {
    /// Which theme the editor draws in.
    ThemeMode,
    /// The theme family the editor draws in.
    ThemeFamily,
    /// The keymap the editor starts from.
    Keymap,
    /// Whether editing starts in vim mode.
    VimMode,
    /// Whether a file is laid out by its formatter when it is saved.
    FormatOnSave,
    /// Whether a new session's worktree is trusted without being asked about.
    TrustWorktrees,
    /// The paths symlinked into a new session's worktree.
    WorktreeLink,
    /// The paths copied into it.
    WorktreeCopy,
    /// The variable a session's own port is handed to its programs in.
    WorktreePort,
}

/// Which of a new worktree's two lists of paths is meant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreePaths {
    /// The paths symlinked in from the repository.
    Linked,
    /// The paths copied in from it.
    Copied,
}

/// Everything the reader decides about how the editor draws and behaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    /// Which theme the editor draws in.
    pub theme_mode: ThemeMode,
    /// Index into `pm_ui::families` of the theme family the editor draws in.
    pub theme_family: usize,
    /// The keymap the editor starts from.
    pub keymap: BaseKeymap,
    /// Whether editing starts in vim mode.
    pub vim_mode: bool,
    /// Whether a file is laid out the way its formatter would when it is saved.
    pub format_on_save: bool,
    /// Whether a new session's worktree is trusted without being asked about.
    pub trust_worktrees: bool,
    /// What a session's fresh worktree is given, git having left it out.
    pub bootstrap: Bootstrap,
}

impl Default for Preferences {
    /// The preferences a first launch starts from.
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            theme_family: DEFAULT_FAMILY,
            keymap: BaseKeymap::default(),
            vim_mode: false,
            format_on_save: false,
            trust_worktrees: false,
            bootstrap: Bootstrap::default(),
        }
    }
}

impl Preferences {
    /// Folds one message into the preferences, saying whether it was one of
    /// theirs.
    pub fn apply(&mut self, message: Message) -> bool {
        match message {
            Message::SetThemeMode(mode) => self.theme_mode = mode,
            Message::SetThemeFamily(index) => {
                self.theme_family = index.min(families().len() - 1);
            }
            Message::SetKeymap(keymap) => self.keymap = keymap,
            Message::ToggleVimMode => self.vim_mode = !self.vim_mode,
            Message::ToggleFormatOnSave => self.format_on_save = !self.format_on_save,
            Message::ToggleTrustWorktrees => self.trust_worktrees = !self.trust_worktrees,
            Message::RemoveWorktreePath(list, index) => {
                let paths = self.paths_mut(list);
                if index < paths.len() {
                    paths.remove(index);
                }
            }
            Message::ResetPreference(preference) => self.reset(preference),
            _ => return false,
        }
        true
    }

    /// Adds `typed` to the `list` of paths a new worktree is given.
    ///
    /// The path is the repository's, so it is kept as it was typed, less the
    /// space and slashes at either end; one already listed is not listed
    /// twice.
    pub fn add_worktree_path(&mut self, list: WorktreePaths, typed: &str) {
        let trimmed = typed.trim().trim_matches('/');
        if trimmed.is_empty() {
            return;
        }
        let path = PathBuf::from(trimmed);
        let paths = self.paths_mut(list);
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    /// Names the variable a session's port is handed in, or none for
    /// nothing typed.
    pub fn set_worktree_port(&mut self, typed: &str) {
        let name = typed.trim();
        self.bootstrap.port = (!name.is_empty()).then(|| name.to_owned());
    }

    /// The `list` of paths a new worktree is given.
    pub fn worktree_paths(&self, list: WorktreePaths) -> &[PathBuf] {
        match list {
            WorktreePaths::Linked => &self.bootstrap.link,
            WorktreePaths::Copied => &self.bootstrap.copy,
        }
    }

    /// The `list` of paths a new worktree is given, to change.
    fn paths_mut(&mut self, list: WorktreePaths) -> &mut Vec<PathBuf> {
        match list {
            WorktreePaths::Linked => &mut self.bootstrap.link,
            WorktreePaths::Copied => &mut self.bootstrap.copy,
        }
    }

    /// Whether `preference` is set to something other than its default.
    pub fn is_modified(&self, preference: Preference) -> bool {
        let mut reset = self.clone();
        reset.reset(preference);
        reset != *self
    }

    /// Puts `preference` back to what a first launch starts from.
    fn reset(&mut self, preference: Preference) {
        let defaults = Self::default();
        match preference {
            Preference::ThemeMode => self.theme_mode = defaults.theme_mode,
            Preference::ThemeFamily => self.theme_family = defaults.theme_family,
            Preference::Keymap => self.keymap = defaults.keymap,
            Preference::VimMode => self.vim_mode = defaults.vim_mode,
            Preference::FormatOnSave => self.format_on_save = defaults.format_on_save,
            Preference::TrustWorktrees => self.trust_worktrees = defaults.trust_worktrees,
            Preference::WorktreeLink => self.bootstrap.link = defaults.bootstrap.link,
            Preference::WorktreeCopy => self.bootstrap.copy = defaults.bootstrap.copy,
            Preference::WorktreePort => self.bootstrap.port = defaults.bootstrap.port,
        }
    }
}
