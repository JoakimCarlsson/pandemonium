//! What the window is told, in one enum every screen answers with.
//!
//! A screen is a function of the window's state and nothing else: input
//! comes back as a [`Message`], the window folds it in, and the next frame
//! is built from the result. There is no widget state anywhere in between,
//! and no screen reaches into the window behind its back.

use pm_core::{EntryId, ProjectId, SessionId};
use pm_text::Position;
use pm_ui::{ResizeEvent, ResizePhase};

use crate::agent::TalkId;
use crate::editor::{FileId, ScrollAxis, SearchField};
use crate::keymap::{Action, BaseKeymap};
use crate::onboarding::ThemeMode;
use crate::panes::{Item, PaneId, SplitDirection, SplitId};
use crate::review::Group;
use crate::terminal::ShellId;
use crate::workspace::SidebarView;

/// One thing the window can be told to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    /// Draw in this theme mode.
    SetThemeMode(ThemeMode),
    /// Draw in this theme family, by index into `pm_ui::families`.
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
    /// Cut a session of the active project from the branch it has out.
    NewSession,
    /// Cut a session of this project from the branch in this place of its list.
    NewSessionFrom(ProjectId, usize),
    /// Show or hide the branches a new session can be cut from.
    ShowSessionBases,
    /// Point the window at this session, bringing its agent forward.
    SelectSession(SessionId),
    /// Open the menu of things that can be done to this session.
    SessionMenu(SessionId),
    /// Ask whether to finish this session, which takes its worktree away.
    FinishSession(SessionId),
    /// Finish this session, having been told to.
    EndSession(SessionId),
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
    /// Show this tab of this pane.
    SelectItem(PaneId, Item),
    /// Close this tab of this pane.
    CloseItem(PaneId, Item),
    /// Write this file to disk and then close its tab in this pane.
    SaveAndClose(PaneId, FileId),
    /// Close this file's tab in this pane, losing what is not on disk.
    DiscardAndClose(PaneId, FileId),
    /// Send later keystrokes to this pane.
    FocusPane(PaneId),
    /// Divide this pane that way, showing the same file in both halves.
    SplitPane(PaneId, SplitDirection),
    /// Divide this pane that way, showing this tab's contents in the new half.
    SplitItem(PaneId, Item, SplitDirection),
    /// Open the menu of things that can be done to this pane.
    ShowPaneMenu(PaneId),
    /// Carry this pane's tab across the window, and let go of it somewhere.
    DragTab(PaneId, Item, ResizeEvent),
    /// Close this pane, giving what it held back to its neighbour.
    ClosePane(PaneId),
    /// Drag this divider of this split, so much of it to a pixel of travel.
    ResizeSplit(SplitId, usize, ResizeEvent, f32),
    /// Put the cursor where a press landed, selecting to where it reached.
    ///
    /// The stage of the gesture comes with it: a press, the drag after it
    /// and the release that ends it say the same two places over again, and
    /// only the press begins anything.
    SelectText(PaneId, ResizePhase, Position, Position),
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
    /// Take the answer the question is showing in this place.
    ChoosePrompt(usize),
    /// Put away the question without answering it.
    DismissPrompt,
    /// Throw away the change the question was asked about.
    ConfirmDiscard,
    /// Take off the disk what the question was asked about.
    ConfirmDelete,
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
    /// Open the menu of things that can be done to this tab.
    ShowTabMenu(PaneId, Item),
    /// Open the menu of things that can be done to this shell's tab.
    ShowTerminalMenu(ShellId),
    /// Put away whatever menu is open.
    DismissMenu,
    /// Close every tab of this pane but this one.
    CloseOtherTabs(PaneId, Item),
    /// Close the tabs of this pane left of this one.
    CloseTabsLeft(PaneId, Item),
    /// Close the tabs of this pane right of this one.
    CloseTabsRight(PaneId, Item),
    /// Close the tabs of this pane that are the same as they are on disk.
    CloseSavedTabs(PaneId),
    /// Close every tab of this pane.
    CloseAllTabs(PaneId),
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
    /// Keep this tab in this pane through a change of project, or let it go.
    TogglePin(PaneId, Item),
    /// End every shell but this one.
    CloseOtherTerminals(ShellId),
    /// End every shell of the project.
    CloseAllTerminals,
    /// Show this in the sidebar that lists the worktree.
    SetSidebarView(SidebarView),
    /// Open the active project's changes for review, in a pane.
    OpenReview,
    /// Ask git again what it makes of every open worktree.
    RefreshChanges,
    /// Open the active project's branch selector.
    ShowBranches,
    /// Open the branch selector from the window-wide status bar.
    ShowStatusBranches,
    /// Create the branch currently typed into the branch selector.
    CreateTypedBranch,
    /// Push the active branch, publishing it first when it has no upstream.
    PushBranch,
    /// Fetch updates from every remote of the active project.
    Fetch,
    /// Pull the active branch with a merge.
    Pull,
    /// Pull the active branch by rebasing its local commits.
    PullRebase,
    /// Push the active branch with a force-with-lease safeguard.
    ForcePush,
    /// Open the menu of remote Git operations.
    ShowRemoteMenu,
    /// Ask which configured remote to fetch from.
    ChooseFetchRemote,
    /// Ask which configured remote to push to.
    ChoosePushRemote,
    /// Put this change into the index, or take it back out if it is in.
    ToggleChangeStaged(usize),
    /// Put this whole group into the index, or take the whole of it out.
    ToggleGroupStaged(Group),
    /// Put the lines of one hunk back the way they were.
    RestoreHunk(usize, bool, usize),
    /// Put one hunk of this change into the index, or take it back out.
    ///
    /// The middle word says which side of the index the hunk was read from,
    /// which is what says whether clicking it stages or unstages.
    ToggleHunkStaged(usize, bool, usize),
    /// Put the list's selection on this change, or mark it alongside.
    ///
    /// Which of the two it is comes from the modifiers held at the time: the
    /// secondary one marks the row, shift marks every row to it, and neither
    /// selects it alone and opens its diff.
    SelectChange(usize),
    /// Put what the list is acting on into the index.
    StageSelection,
    /// Take what the list is acting on back out of the index.
    UnstageSelection,
    /// Ask whether what the list is acting on should be thrown away.
    DiscardSelection,
    /// Move the review to the hunk above the one it is showing.
    PreviousHunk,
    /// Move the review to the hunk below it.
    NextHunk,
    /// Bring the review forward, at this change.
    OpenChange(usize),
    /// Open this change's diff on its own, in the pane that has the keyboard.
    OpenChangeDiff(usize),
    /// Open the file this change is to, at the first line it changed.
    OpenChangeFile(usize),
    /// Open the menu of things that can be done to this change.
    ShowChangeMenu(usize),
    /// Put this change's path on the clipboard.
    CopyChangePath(usize),
    /// Put this change's path, from the worktree down, on the clipboard.
    CopyChangeRelativePath(usize),
    /// Show the file this change is to in the desktop's file manager.
    RevealChange(usize),
    /// Show or hide the lines this change covers.
    ExpandChange(usize),
    /// Put everything the active project has changed into the index.
    StageAll,
    /// Take everything the active project has staged back out of the index.
    UnstageAll,
    /// Put the commit message's cursor where a press landed, selecting to it.
    ///
    /// A press in the message is also what gives it the keyboard, so this is
    /// the whole of how it is written in: there is nothing to focus first.
    WriteCommit(ResizePhase, Position, Position),
    /// Commit what the index holds, saying what the message field holds.
    Commit,
    /// Ask which agent to start in the active project's worktree.
    NewAgentSession,
    /// Put the prompt's cursor where a press landed, selecting to it.
    ///
    /// A press in the prompt is also what gives it the keyboard, so this is
    /// the whole of how a prompt is written in: there is nothing to focus
    /// first.
    WriteAgentPrompt(TalkId, ResizePhase, Position, Position),
    /// Send what this session's prompt holds, and empty it.
    SendPrompt(TalkId),
    /// Answer this session's permission request with the choice in this place.
    AnswerAgent(TalkId, u64, usize),
    /// Put the command this session is offering in this place into its prompt.
    TakeAgentCommand(TalkId, usize),
    /// Ask which of its agent's modes to put this session into.
    ShowAgentModes(TalkId),
    /// Put this session into the mode after the one it is in.
    CycleAgentMode(TalkId),
    /// Act on the knob in this place: ask which value, or flip the switch.
    PressKnob(TalkId, usize),
    /// Start naming one of this session's commands, in its prompt.
    StartAgentCommand(TalkId),
    /// Stop the turn this session is running.
    StopAgentTurn(TalkId),
    /// Open the menu of things that can be done to the box being written in.
    ShowInputMenu,
    /// Carry out this command in whatever box has the keyboard.
    EditText(Action),
}
