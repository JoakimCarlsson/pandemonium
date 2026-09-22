//! What the window is told, in one enum every screen answers with.
//!
//! A screen is a function of the window's state and nothing else: input
//! comes back as a [`Message`], the window folds it in, and the next frame
//! is built from the result. There is no widget state anywhere in between,
//! and no screen reaches into the window behind its back.

use pm_core::{EntryId, ProjectId};
use pm_text::Position;
use pm_ui::ResizeEvent;

use crate::editor::{FileId, ScrollAxis, SearchField};
use crate::keymap::{Action, BaseKeymap};
use crate::onboarding::ThemeMode;
use crate::panes::{PaneId, SplitDirection, SplitId};
use crate::terminal::ShellId;

/// One thing the window can be told to do.
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
    /// Turn laying a file out when it is saved on or off.
    ToggleFormatOnSave,
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
    /// Write this file to disk and then close its tab in this pane.
    SaveAndClose(PaneId, FileId),
    /// Close this file's tab in this pane, losing what is not on disk.
    DiscardAndClose(PaneId, FileId),
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
    /// Drag one of this pane's scrollbars, so much of it to a pixel of travel.
    ScrollEditor(PaneId, ScrollAxis, ResizeEvent, f32),
    /// Select every line a drag down this pane's gutter reaches.
    SelectLines(PaneId, Position, Position),
    /// Open the menu of things that can be done to the text in this pane.
    ShowEditorMenu(PaneId),
    /// Fold what this line of this pane holds, or unfold it.
    ToggleFold(PaneId, Position),
    /// Give this pane the keyboard, then carry out this command in it.
    PaneAction(PaneId, Action),
    /// Send later keystrokes to this field of this pane's search bar.
    FocusSearch(PaneId, SearchField, usize),
    /// Turn matching upper case against upper case on or off.
    ToggleSearchCase(PaneId),
    /// Turn matching whole words only on or off.
    ToggleSearchWord(PaneId),
    /// Show or hide the replacement field of this pane's search bar.
    ToggleSearchReplace(PaneId),
    /// Close this pane's search bar.
    CloseSearch(PaneId),
    /// Put the caret this many characters into the picker's field.
    PlacePicker(usize),
    /// Take the row the picker is showing in this place.
    ChoosePicker(usize),
    /// Take the completion the list is showing in this place.
    ChooseCompletion(usize),
    /// Take the fix the server offered in this place.
    TakeCodeAction(usize),
    /// Put away whatever is open over the text.
    DismissPopup,
    /// Open the menu of things that can be done to this entry of the tree.
    ShowEntryMenu(EntryId),
    /// Ask for the name of a file to make beside or inside this entry.
    NewFileIn(EntryId),
    /// Ask for the name of a directory to make beside or inside this entry.
    NewFolderIn(EntryId),
    /// Ask for a new name for this entry.
    RenameEntry(EntryId),
    /// Ask whether this entry should be taken off the disk.
    DeleteEntry(EntryId),
    /// Put this entry's path on the clipboard.
    CopyEntryPath(EntryId),
    /// Put this entry's path, from the worktree down, on the clipboard.
    CopyEntryRelativePath(EntryId),
    /// Show this entry in the desktop's file manager.
    RevealEntry(EntryId),
    /// Start a shell in this entry's directory.
    OpenEntryInTerminal(EntryId),
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
