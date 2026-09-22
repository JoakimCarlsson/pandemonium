//! What the setup screen decides, and the messages that change it.

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
    /// The mode's label in the theme toggle.
    pub(super) fn label(self) -> &'static str {
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

/// Everything the setup screen decides.
#[derive(Clone, Debug)]
pub struct Setup {
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
    /// Whether setup has been finished, which swaps the page.
    pub finished: bool,
}

impl Default for Setup {
    /// The settings a first launch starts from.
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
            finished: false,
        }
    }
}

impl Setup {
    /// Folds one message into the settings.
    ///
    /// Everything else the window is told is the window's own business; the
    /// setup screen only holds what a first run decides.
    pub fn apply(&mut self, message: Message) {
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
            Message::Finish => self.finished = true,
            Message::Reopen => self.finished = false,
            _ => {}
        }
    }
}
