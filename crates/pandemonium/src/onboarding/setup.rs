//! What the setup screen decides, and the messages that change it.

use pm_core::{EntryId, ProjectId};
use pm_text::Position;
use pm_ui::{Appearance, DEFAULT_FAMILY, FAMILIES, ResizeEvent};
use serde::{Deserialize, Serialize};

use crate::editor::FileId;
use crate::keymap::BaseKeymap;
use crate::panes::{PaneId, SplitDirection, SplitId};
use crate::terminal::ShellId;

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
    /// Send later keystrokes to the terminal.
    FocusTerminal,
    /// Start another shell in the active project's worktree.
    NewTerminal,
    /// Show this shell in the terminal pane.
    SelectTerminal(ShellId),
    /// End this shell.
    CloseTerminal(ShellId),
    /// Drag the terminal's scrollbar, so many lines to a pixel of travel.
    ScrollTerminal(ResizeEvent, f32),
    /// Toggle the secondary sidebar.
    ToggleSecondarySidebar,
    /// Resize the bottom panel.
    ResizeBottomPanel(ResizeEvent),
    /// Resize the secondary sidebar.
    ResizeSecondarySidebar(ResizeEvent),
    /// Open this entry of the file tree in the pane that has the keyboard.
    OpenFile(EntryId),
    /// Show this open file in this pane.
    SelectFile(PaneId, FileId),
    /// Close this open file's tab in this pane.
    CloseFile(PaneId, FileId),
    /// Send later keystrokes to this pane.
    FocusPane(PaneId),
    /// Divide this pane that way, showing the same file in both halves.
    SplitPane(PaneId, SplitDirection),
    /// Divide this pane that way, showing this file in the new half.
    SplitFile(PaneId, FileId, SplitDirection),
    /// Open the menu of things that can be done to this pane.
    ShowPaneMenu(PaneId),
    /// Carry this pane's tab across the window, and let go of it somewhere.
    DragTab(PaneId, FileId, ResizeEvent),
    /// Close this pane, giving what it held back to its neighbour.
    ClosePane(PaneId),
    /// Drag this divider of this split, so much of it to a pixel of travel.
    ResizeSplit(SplitId, usize, ResizeEvent, f32),
    /// Put the cursor where a press landed, selecting to where it reached.
    SelectText(PaneId, Position, Position),
    /// Drag the editor's scrollbar, so many lines to a pixel of travel.
    ScrollEditor(PaneId, ResizeEvent, f32),
    /// Open the menu of things that can be done to this file's tab.
    ShowFileMenu(PaneId, FileId),
    /// Open the menu of things that can be done to this shell's tab.
    ShowTerminalMenu(ShellId),
    /// Put away whatever menu is open.
    DismissMenu,
    /// Close every tab of this pane but this one.
    CloseOtherFiles(PaneId, FileId),
    /// Close the tabs of this pane left of this one.
    CloseFilesLeft(PaneId, FileId),
    /// Close the tabs of this pane right of this one.
    CloseFilesRight(PaneId, FileId),
    /// Close the tabs of this pane that are the same as they are on disk.
    CloseSavedFiles(PaneId),
    /// Close every tab of this pane.
    CloseAllFiles(PaneId),
    /// Put this file's path on the clipboard.
    CopyFilePath(FileId),
    /// Put this file's path, from the worktree down, on the clipboard.
    CopyFileRelativePath(FileId),
    /// Show this file in the desktop's file manager.
    RevealFile(FileId),
    /// Start a shell in the directory this file is in.
    OpenFileInTerminal(FileId),
    /// Keep this previewed file open, so nothing takes its tab.
    KeepFileOpen(FileId),
    /// End every shell but this one.
    CloseOtherTerminals(ShellId),
    /// End every shell of the project.
    CloseAllTerminals,
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
            | Message::FocusTerminal
            | Message::NewTerminal
            | Message::SelectTerminal(_)
            | Message::CloseTerminal(_)
            | Message::ScrollTerminal(_, _)
            | Message::ToggleSecondarySidebar
            | Message::ResizeBottomPanel(_)
            | Message::ResizeSecondarySidebar(_) => {}
            Message::OpenFile(_)
            | Message::SelectFile(_, _)
            | Message::CloseFile(_, _)
            | Message::FocusPane(_)
            | Message::SplitPane(_, _)
            | Message::SplitFile(_, _, _)
            | Message::ShowPaneMenu(_)
            | Message::DragTab(_, _, _)
            | Message::ClosePane(_)
            | Message::ResizeSplit(_, _, _, _)
            | Message::SelectText(_, _, _)
            | Message::ScrollEditor(_, _, _) => {}
            Message::ShowFileMenu(_, _)
            | Message::ShowTerminalMenu(_)
            | Message::DismissMenu
            | Message::CloseOtherFiles(_, _)
            | Message::CloseFilesLeft(_, _)
            | Message::CloseFilesRight(_, _)
            | Message::CloseSavedFiles(_)
            | Message::CloseAllFiles(_)
            | Message::CopyFilePath(_)
            | Message::CopyFileRelativePath(_)
            | Message::RevealFile(_)
            | Message::OpenFileInTerminal(_)
            | Message::KeepFileOpen(_)
            | Message::CloseOtherTerminals(_)
            | Message::CloseAllTerminals => {}
        }
    }
}
