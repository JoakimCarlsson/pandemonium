//! What the setup screen decides, and the messages that change it.

use pm_ui::{Appearance, DEFAULT_FAMILY, FAMILIES};

/// Keymaps the editor can start from, in the order they are offered.
pub(super) const KEYMAPS: [&str; 8] = [
    "Pandemonium",
    "VS Code",
    "Zed",
    "JetBrains",
    "Vim",
    "Emacs",
    "Helix",
    "Sublime Text",
];

/// Which theme the editor draws in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// Index into `pm_ui::FAMILIES` of the theme family the editor draws in.
    pub theme_family: usize,
    /// Index into [`KEYMAPS`] of the keymap the editor starts from.
    pub keymap: usize,
    /// Whether editing starts in vim mode.
    pub vim_mode: bool,
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
            keymap: 0,
            vim_mode: false,
            trust_worktrees: false,
            metrics: true,
            crash_reports: true,
            finished: false,
        }
    }
}

/// One thing the screen can be told to change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Message {
    /// Draw in this theme mode.
    SetThemeMode(ThemeMode),
    /// Draw in this theme family, by index into `pm_ui::FAMILIES`.
    SetThemeFamily(usize),
    /// Start from this keymap, by index into [`KEYMAPS`].
    SetKeymap(usize),
    /// Turn vim mode on or off.
    ToggleVimMode,
    /// Turn worktree auto-trust on or off.
    ToggleTrustWorktrees,
    /// Turn anonymous usage data on or off.
    ToggleMetrics,
    /// Turn crash reports on or off.
    ToggleCrashReports,
    /// Leave the setup flow.
    Finish,
    /// Come back to the setup flow.
    Reopen,
}

impl Setup {
    /// Folds one message into the settings.
    pub fn apply(&mut self, message: Message) {
        match message {
            Message::SetThemeMode(mode) => self.theme_mode = mode,
            Message::SetThemeFamily(index) => {
                self.theme_family = index.min(FAMILIES.len() - 1);
            }
            Message::SetKeymap(index) => self.keymap = index,
            Message::ToggleVimMode => self.vim_mode = !self.vim_mode,
            Message::ToggleTrustWorktrees => self.trust_worktrees = !self.trust_worktrees,
            Message::ToggleMetrics => self.metrics = !self.metrics,
            Message::ToggleCrashReports => self.crash_reports = !self.crash_reports,
            Message::Finish => self.finished = true,
            Message::Reopen => self.finished = false,
        }
    }
}
