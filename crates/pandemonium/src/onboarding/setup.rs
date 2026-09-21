//! What the setup screen decides, and the messages that change it.

use pm_core::{EntryId, ProjectId};
use pm_ui::{Appearance, DEFAULT_FAMILY, FAMILIES, ResizeEvent};
use serde::{Deserialize, Serialize};

use crate::keymap::BaseKeymap;

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
    /// Index into `pm_ui::FAMILIES` of the theme family the editor draws in.
    pub theme_family: usize,
    /// The keymap the editor starts from.
    pub keymap: BaseKeymap,
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
            keymap: BaseKeymap::default(),
            vim_mode: false,
            trust_worktrees: false,
            metrics: true,
            crash_reports: true,
            finished: false,
        }
    }
}

/// One thing the screen can be told to change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    /// Draw in this theme mode.
    SetThemeMode(ThemeMode),
    /// Draw in this theme family, by index into `pm_ui::FAMILIES`.
    SetThemeFamily(usize),
    /// Start from this keymap.
    SetKeymap(BaseKeymap),
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
    /// Ask for a repository and add it to the window as a project.
    OpenProject,
    /// Take this project out of the window.
    CloseProject(ProjectId),
    /// Make this the project the window's files and commands apply to.
    ActivateProject(ProjectId),
    /// Open the menu of things that can be done to this project.
    ProjectMenu(ProjectId),
    /// Show or hide what this directory of the file tree holds.
    ToggleEntry(EntryId),
    /// Resize the sessions sidebar.
    ResizeSidebar(ResizeEvent),
    /// Minimize the application window.
    MinimizeWindow,
    /// Toggle whether the application window is maximized.
    ToggleMaximizedWindow,
    /// Close the application window.
    CloseWindow,
    /// Toggle the primary sidebar.
    TogglePrimarySidebar,
    /// Toggle the bottom panel.
    ToggleBottomPanel,
    /// Toggle the secondary sidebar.
    ToggleSecondarySidebar,
    /// Resize the bottom panel.
    ResizeBottomPanel(ResizeEvent),
    /// Resize the secondary sidebar.
    ResizeSecondarySidebar(ResizeEvent),
}

impl Setup {
    /// Folds one message into the settings.
    pub fn apply(&mut self, message: Message) {
        match message {
            Message::SetThemeMode(mode) => self.theme_mode = mode,
            Message::SetThemeFamily(index) => {
                self.theme_family = index.min(FAMILIES.len() - 1);
            }
            Message::SetKeymap(keymap) => self.keymap = keymap,
            Message::ToggleVimMode => self.vim_mode = !self.vim_mode,
            Message::ToggleTrustWorktrees => self.trust_worktrees = !self.trust_worktrees,
            Message::ToggleMetrics => self.metrics = !self.metrics,
            Message::ToggleCrashReports => self.crash_reports = !self.crash_reports,
            Message::Finish => self.finished = true,
            Message::Reopen => self.finished = false,
            Message::OpenProject
            | Message::CloseProject(_)
            | Message::ActivateProject(_)
            | Message::ProjectMenu(_)
            | Message::ToggleEntry(_) => {}
            Message::ResizeSidebar(_) => {}
            Message::MinimizeWindow | Message::ToggleMaximizedWindow | Message::CloseWindow => {}
            Message::TogglePrimarySidebar
            | Message::ToggleBottomPanel
            | Message::ToggleSecondarySidebar
            | Message::ResizeBottomPanel(_)
            | Message::ResizeSecondarySidebar(_) => {}
        }
    }
}
