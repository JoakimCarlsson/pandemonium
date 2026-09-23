//! The preferences the editor draws and behaves by, and the messages that
//! change them.
//!
//! Onboarding and the settings pane are two editors of this one value: both
//! build their screens from it, both answer with the same [`Message`], and
//! [`Preferences::apply`] is the one place either of them changes it.

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
    /// Whether anonymous usage data is sent.
    Metrics,
    /// Whether crash reports are sent.
    CrashReports,
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
    /// Whether anonymous usage data is sent.
    pub metrics: bool,
    /// Whether crash reports are sent.
    pub crash_reports: bool,
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
            metrics: true,
            crash_reports: true,
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
            Message::ToggleMetrics => self.metrics = !self.metrics,
            Message::ToggleCrashReports => self.crash_reports = !self.crash_reports,
            Message::ResetPreference(preference) => self.reset(preference),
            _ => return false,
        }
        true
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
            Preference::Metrics => self.metrics = defaults.metrics,
            Preference::CrashReports => self.crash_reports = defaults.crash_reports,
        }
    }
}
