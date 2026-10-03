//! What the window is told, in one enum every screen answers with.
//!
//! A screen is a function of the window's state and nothing else: input
//! comes back as a [`Message`], the window folds it in, and the next frame
//! is built from the result. There is no widget state anywhere in between,
//! and no screen reaches into the window behind its back.

use pm_core::{EntryId, ProjectId, Scope, SessionId};
use pm_gfx::Point;
use pm_text::Position;
use pm_ui::{ResizeEvent, ResizePhase};

use crate::agent::TalkId;
use crate::config::{FontSlot, InstallLanguageServers, Preference, Step, ThemeMode, WorktreePaths};
use crate::editor::{CursorShape, FileId, ScrollAxis, SearchField};
use crate::keymap::Action;
use crate::markdown::DiagramZoom;
use crate::notice::{NoticeId, NotificationAction};
use crate::panel::PanelView;
use crate::panes::{Item, PaneId, SplitDirection, SplitId, Tool};
use crate::review::comment::{CommentId, Side as CommentSide};
use crate::review::{ChangeId, ConflictAction, Group, RepositoryAction};
use crate::settings::{FormField, SettingsPage, SettingsSection};
use crate::terminal::ShellId;

/// A matching option on a project search pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectSearchOption {
    /// Regular expression syntax.
    Regex,
    /// Letter case.
    Case,
    /// Whole words.
    Word,
}

/// One thing the window can be told to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    /// Copies the focused reading surface’s selected text.
    CopyText,
    /// Follows a link in a rendered document.
    FollowRenderedLink(FileId, usize),
    /// Selects all placed text in the focused reading surface.
    SelectAllText,
    /// Draw in this theme mode.
    SetThemeMode(ThemeMode),
    /// Draw in this theme family, by index into `pm_ui::families`.
    SetThemeFamily(usize),
    /// Start from this keymap, by index into `keymap::keymaps`.
    SetKeymap(usize),
    /// Listen for the chords to bind this action to.
    RecordBinding(Action),
    /// Take every chord this action is pressed as away.
    UnbindAction(Action),
    /// Ask what to call the keymap being pressed, and write it down as one.
    SaveKeymap,
    /// Read the keymaps in the editor's home in again.
    ReloadKeymaps,
    /// Drag the settings page's visible scroll thumb.
    ScrollSettings(ResizeEvent, f32),
    /// Ask which language the Language Settings section shows.
    PickSettingsLanguage,
    /// Flip this switch for the language being set.
    ToggleLanguageSetting(crate::config::languages::LanguageSetting),
    /// Move this number one step for the language being set.
    StepLanguageSetting(crate::config::languages::LanguageSetting, Step),
    /// Put this setting of the language being set back to the shared preference.
    ResetLanguageSetting(crate::config::languages::LanguageSetting),
    /// Put every setting of the language being set back.
    ResetLanguageSettings,
    /// Lay the language being set out with this kind of formatter.
    SetLanguageFormatter(crate::config::languages::FormatterKind),
    /// Ask for the command line the language being set is piped through.
    AskLanguageFormatter,
    /// Open a form for a new server of the language being set.
    AddLanguageServer,
    /// Open a form for this server of the language being set.
    EditLanguageServer(usize),
    /// Remove this server of the language being set.
    RemoveLanguageServer(usize),
    /// Put the servers of the language being set back to the ones it names.
    ResetLanguageServers,
    /// Save the server form.
    SaveLanguageServer,
    /// Close the server form.
    CancelLanguageServer,
    /// Give a box of the server form the keyboard.
    FocusLanguageServerField(usize),
    /// Edit a box of the server form.
    WriteLanguageServerField(usize, ResizePhase, Position, Position),
    /// Open the source of an offered or installed language package.
    OpenLanguageSource(usize, bool),
    /// Retrieve the maintained language extension catalogue.
    RefreshLanguageCatalogue,
    /// Install or update the catalogue entry at this position.
    InstallLanguageExtension(usize),
    /// Import a local extension directory through the platform picker.
    ImportLanguageExtension,
    /// Install the remembered import at this position again from its folder.
    ReinstallLanguageExtension(usize),
    /// Remove an installed language extension.
    RemoveLanguageExtension(usize),
    /// Open or fold the group of built-in languages.
    ToggleBuiltinLanguages,
    /// Choose which extensions the list shows.
    SetLanguageFilter(crate::settings::languages::Filter),
    /// Read installed extensions again.
    ReloadExtensions,
    /// Turn this preference, which is a switch, on or off.
    TogglePreference(Preference),
    /// Move this preference, which is a number, one step this way.
    StepPreference(Preference, Step),
    /// Draw the caret in this shape.
    SetCursorShape(CursorShape),
    /// Share vim's unnamed register with the system clipboard this much.
    SetVimClipboard(pm_vim::ClipboardUse),
    /// Choose how missing language servers are installed.
    SetInstallLanguageServers(InstallLanguageServers),
    /// Install a named server from its pinned recipe.
    InstallLanguageServer(&'static str),
    /// Open the log of a language server behind the focused file.
    OpenServerLog,
    /// Open the log of a particular server, even after focus changes.
    OpenServerLogAt(usize),
    /// Draw a guide down this column, or none.
    SetWrapGuide(Option<usize>),
    /// Ask which family to set this kind of text in.
    PickFont(FontSlot),
    /// Ask what to repaint this colour, by index into the theme's tokens, in.
    EditThemeColor(usize),
    /// Ask what to call the theme being drawn in, and write it down as one.
    SaveTheme,
    /// Read the themes in the editor's home in again.
    ReloadThemes,
    /// Ask for a path to add to this list of what a new worktree is given.
    AddWorktreePath(WorktreePaths),
    /// Take the path in this place off this list of what a new worktree is
    /// given.
    RemoveWorktreePath(WorktreePaths, usize),
    /// Ask for the variable a session's port is handed to its programs in.
    EditWorktreePort,
    /// Put this preference back to what a first launch starts from.
    ResetPreference(Preference),
    /// Leave the setup flow.
    Finish,
    /// Open the settings pane, or bring it forward where it is open.
    OpenSettings,
    /// Close the window preferences modal and return to the workspace.
    CloseSettings,
    /// Copies the running version and source commit to the system clipboard.
    CopyVersion,
    /// Open the repository the editor is published from.
    OpenRepository,
    /// Put the caret of the agent search box where a press landed, selecting to it.
    WriteAgentSearch(ResizePhase, Position, Position),
    /// Install the registry's agent in this place of the list on offer.
    InstallAgent(usize),
    /// Open or fold the list of installed agents.
    ToggleAgentsInstalled,
    /// Open or fold the list of agents on offer.
    ToggleAgentsAvailable,
    /// Start describing an agent to run beside the shipped ones.
    AddAgentServer,
    /// Describe the agent the reader added in this place of the list again.
    EditAgentServer(usize),
    /// Take the agent the reader added in this place of the list away.
    RemoveAgentServer(usize),
    /// Open the menu of what can be done to the agent the reader added in this place.
    ShowAgentServerMenu(usize),
    /// Start describing a tool server to add to every agent.
    AddMcpServer,
    /// Put the caret of this box of the tool server form where a press landed, selecting to it.
    WriteFormField(FormField, ResizePhase, Position, Position),
    /// Add an empty variable to the tool server form.
    AddFormVariable,
    /// Add the variable the registry lists in this place to the tool server form.
    SuggestFormVariable(usize),
    /// Take this variable out of the tool server form.
    RemoveFormVariable(usize),
    /// Write the tool server form down.
    SaveServerForm,
    /// Let go of the tool server form.
    CancelServerForm,
    /// Switch the tool server in this place of the list on or off.
    ToggleMcpServer(usize),
    /// Put the configuration of the tool server in this place on the clipboard.
    CopyMcpConfiguration(usize),
    /// Open the page describing the tool server in this place.
    OpenMcpWebsite(usize),
    /// Show the file the tool servers are written to.
    RevealSettingsFile,
    /// Open the menu of what can be done to the tool server in this place.
    ShowMcpServerMenu(usize),
    /// Install the registry's server in this place of the list on offer.
    InstallMcpServer(usize),
    /// Open or fold the list of installed MCP servers.
    ToggleMcpInstalled,
    /// Open or fold the list of MCP servers on offer.
    ToggleMcpAvailable,
    /// Open the page that says what MCP servers are.
    OpenMcpDocs,
    /// Put the caret of the MCP search box where a press landed, selecting to it.
    WriteMcpSearch(ResizePhase, Position, Position),
    /// Describe the tool server in this place of the list again.
    EditMcpServer(usize),
    /// Take the tool server in this place of the list away.
    RemoveMcpServer(usize),
    /// Show this page of the settings pane, from its top.
    ShowSettingsPage(SettingsPage),
    /// Show the page of the settings pane this section is on, scrolled to it.
    ShowSettingsSection(SettingsSection),
    /// Open this page's sections out in the settings sidebar, or fold them.
    ToggleSettingsPage(SettingsPage),
    /// Open the menu of ways a project is added to the window.
    AddProjectMenu,
    /// Ask for a repository and add it to the window as a project.
    OpenProject,
    /// Ask for a repository URL, clone it, and add it as a project.
    CloneProject,
    /// Take this project out of the window.
    CloseProject(ProjectId),
    /// Open the menu of things that can be done to this project.
    ProjectMenu(ProjectId),
    /// Name a new project group, optionally placing a project in it.
    NewProjectGroup(Option<ProjectId>),
    /// Name an existing project group.
    RenameProjectGroup(usize),
    /// Remove a group while keeping its projects open.
    RemoveProjectGroup(usize),
    /// Fold or unfold a project group.
    ToggleProjectGroup(usize),
    /// Open the actions for a project group.
    ProjectGroupMenu(usize),
    /// Move a project to a group, or leave it ungrouped.
    AssignProjectGroup(ProjectId, Option<usize>),
    /// Press, drag or let go of this project's row in the projects sidebar.
    DragProject(ProjectId, ResizeEvent),
    /// Cut a session of the active project from the branch it has out.
    NewSession,
    /// Runs health checks in the named worktree.
    RunChecks(Scope),
    /// Shows the retained output of a worktree check.
    ShowCheckOutput(Scope),
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
    /// Minimize the application window.
    MinimizeWindow,
    /// Toggle whether the application window is maximized.
    ToggleMaximizedWindow,
    /// Close the application window.
    CloseWindow,
    /// Close a registered tool, or reopen it when it is closed.
    ToggleTool(Tool),
    /// Bring this view of the bottom panel to the front, opening the panel.
    ShowPanelView(PanelView),
    /// Close the bottom panel when this view is in front of it, and bring
    /// the view to the front of the opened panel otherwise.
    TogglePanelView(PanelView),
    /// Go to where this problem of this open file is.
    OpenProblem(FileId, Position),
    /// Press or drag over the terminal's grid, from one cell to another.
    PointTerminal(ResizePhase, pm_vt::Place, pm_vt::Place),
    /// Open the menu of things that can be done to what the terminal shows.
    ShowScreenMenu,
    /// Carry this command out, from a control that stands for it.
    Act(Action),
    /// Carry this command out on the terminal, giving it the keyboard.
    ActOnTerminal(Action),
    /// Start another shell in the active project's worktree.
    NewTerminal,
    /// Show this shell in the terminal pane.
    SelectTerminal(ShellId),
    /// End this shell.
    CloseTerminal(ShellId),
    /// Ask what to call this shell.
    RenameTerminal(ShellId),
    /// Drag the terminal's scrollbar, so many lines to a pixel of travel.
    ScrollTerminal(ResizeEvent, f32),
    /// Resize the Source Control graph.
    ResizeHistoryGraph(ResizeEvent),
    /// Show or hide the Source Control graph.
    ToggleHistoryGraph,
    /// Expand or collapse the Source Control changes section.
    ToggleChangesSection,
    /// Show the available Source Control commit actions.
    ShowCommitMenu,
    /// Show the Source Control action menu.
    ShowSourceControlMenu,
    /// Show the menu of the window's agents that stand this way.
    ShowAgentsMenu(crate::agent::Standing),
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
    /// Open a rendered view or editable source beside this pane's file.
    PreviewFile(PaneId),
    /// Open the outline for the file named by a tab's context menu.
    OpenOutline(PaneId, FileId),
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
    /// Put the cursor of this pane of excerpts in this file, where a press
    /// landed, selecting to where it reached.
    SelectExcerpt(PaneId, ResizePhase, FileId, Position, Position),
    /// Open the file in this place of this pane's excerpts on its own.
    OpenExcerptFile(PaneId, usize),
    /// Scroll this pane so the line a press on its minimap reached is in the
    /// middle of it.
    ScrollEditorTo(PaneId, usize),
    /// Put this pane's cursor at this place, the way a jump does.
    JumpTo(PaneId, Position),
    /// Select and jump to a symbol in the worktree's outline.
    OutlineSelect(Scope, usize),
    /// Expand or collapse one symbol in the worktree's outline.
    OutlineToggle(Scope, usize),
    /// Give the outline filter its caret at this character.
    OutlineFilterFocus(Scope, usize),
    /// Select every line a drag down this pane's gutter reaches.
    SelectLines(PaneId, Position, Position),
    /// Open the menu of things that can be done to the text in this pane.
    ShowEditorMenu(PaneId),
    /// Fold what this line of this pane holds, or unfold it.
    ToggleFold(PaneId, Position),
    /// Set a breakpoint on this line of the file in this pane, or clear it.
    ToggleBreakpoint(PaneId, Position),
    /// Open the menu for a breakpoint column's source line.
    ShowBreakpointMenu(PaneId, Position),
    /// Open a field prompt for one source breakpoint.
    EditBreakpoint(PaneId, usize, crate::picker::Kind),
    /// Remove a watch expression from the active worktree.
    RemoveWatch(usize),
    /// Edit a watch expression in the active worktree.
    EditWatch(usize),
    /// Open or close the watch section.
    ToggleWatchSection,
    /// Carry this debugging command out on the worktree's program.
    ActOnDebugger(Action),
    /// Look at the frame of the paused program's stack this names.
    SelectFrame(i64),
    /// Open the variable of the paused program this names, or close it.
    ToggleVariable(i64),
    /// Open the scope in this place of the selected frame's, or close it.
    ToggleDebugScope(usize),
    /// Put the debug console's cursor where a press landed, selecting to it.
    WriteDebugConsole(ResizePhase, Position, Position),
    /// Give this pane the keyboard, then carry out this command in it.
    PaneAction(PaneId, Action),
    /// Send later keystrokes to this field of this pane's search bar.
    FocusSearch(PaneId, SearchField, usize),
    /// Focuses one field of a worktree search pane.
    FocusProjectSearch(PaneId, SearchField, usize),
    /// Flips a matching option in a worktree search pane.
    ToggleProjectSearch(PaneId, ProjectSearchOption),
    /// Confirms replacing all matches across multiple files.
    ConfirmProjectReplace,
    /// Turn matching upper case against upper case on or off.
    ToggleSearchCase(PaneId),
    /// Turn matching whole words only on or off.
    ToggleSearchWord(PaneId),
    /// Toggles regular expression matching in the search bar.
    ToggleSearchRegex(PaneId),
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
    /// Press, drag or let go of this row of the file tree.
    ///
    /// A press that goes nowhere is a click: it selects the row, opens a
    /// file and opens or closes a directory. One that travels carries what
    /// is selected, to drop into a directory or onto a pane.
    PressEntry(EntryId, ResizeEvent),
    /// Press the file tree below its last row, letting go of the selection.
    PressTreeSpace,
    /// Open the menu of things that can be done to this entry of the tree.
    ShowEntryMenu(EntryId),
    /// Open the menu of the tree itself, from the space below its rows.
    ShowTreeMenu,
    /// Start typing the name of a file to make where the tree is pointed.
    NewTreeFile,
    /// Start typing the name of a directory to make where the tree is pointed.
    NewTreeFolder,
    /// Start typing a new name for the row the tree's keyboard is on.
    RenameTreeEntry,
    /// Put the caret of the name being typed into the tree this far in.
    PlaceTreeEdit(usize),
    /// Ask whether what the tree is acting on should go to the trash.
    TrashTreeEntries,
    /// Ask whether what the tree is acting on should come off the disk.
    DeleteTreeEntries,
    /// Move what the tree is acting on to the trash, having been told to.
    ConfirmTrash,
    /// Take what the tree is acting on off the tree's clipboard to move it.
    CutTreeEntries,
    /// Put what the tree is acting on on the tree's clipboard to copy it.
    CopyTreeEntries,
    /// Copy or move what is on the tree's clipboard where the tree points.
    PasteTreeEntries,
    /// Copy what the tree is acting on beside itself.
    DuplicateTreeEntries,
    /// Open what the tree is acting on in a pane of its own, to the side.
    OpenTreeEntriesToSide,
    /// Put the paths of what the tree is acting on on the clipboard.
    CopyTreePaths,
    /// Put those paths, from the worktree down, on the clipboard.
    CopyTreeRelativePaths,
    /// Show the row the tree's keyboard is on in the desktop's file manager.
    RevealTreeEntry,
    /// Start a shell in the directory of the row the tree's keyboard is on.
    OpenTreeEntryInTerminal,
    /// Close every directory of the tree.
    CollapseTree,
    /// Read the tree's worktree again.
    RefreshTree,
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
    /// Show a registered workspace tool wherever its tab lives.
    ShowTool(Tool),
    /// Move a registered workspace tool into this pane.
    MoveTool(PaneId, Tool),
    /// Show the menu of workspace tools.
    ShowToolsMenu,
    /// Restore the default pane arrangement without closing live tabs.
    ResetWindowLayout,
    /// Open the active project's changes for review, in a pane.
    OpenReview,
    /// Open the active worktree's changes as excerpts of their files, to be
    /// edited in one pane.
    OpenExcerpts,
    /// Ask git again what it makes of every open worktree.
    RefreshChanges,
    /// Open the branch selector from the window-wide status bar.
    ShowStatusBranches,
    /// Create the branch currently typed into the branch selector.
    CreateTypedBranch,
    /// Push the active branch, publishing it first when it has no upstream.
    PushBranch,
    /// Pull the active branch, then push it, as one step.
    SyncBranch,
    /// Fetch updates from every remote of the active project.
    Fetch,
    /// Pull the active branch with a merge.
    Pull,
    /// Pull the active branch by rebasing its local commits.
    PullRebase,
    /// Push the active branch with a force-with-lease safeguard.
    ForcePush,
    /// Show the Graph history-reference filter.
    ShowHistoryRefsMenu,
    /// Set the Graph history-reference filter to Auto or All.
    SetHistoryFilter(bool),
    /// Open actions for a commit row of one repository.
    ShowHistoryMenu(usize, usize),
    /// Apply the captured history commit to the current branch.
    CherryPickHistory,
    /// Copy the captured full commit hash.
    CopyCommitHash,
    /// Return the Graph to the checked-out commit.
    RevealCurrentHistoryItem,
    /// Ask which configured remote to fetch from.
    ChooseFetchRemote,
    /// Ask which configured remote to push to.
    ChoosePushRemote,
    /// Put this change into the index, or take it back out if it is in.
    ToggleChangeStaged(usize),
    /// Put this whole group of this repository into the index, or take the
    /// whole of it out.
    ToggleGroupStaged(usize, Group),
    /// Make this repository the active one, then do what one of its own
    /// controls asks for there.
    InRepository(usize, RepositoryAction),
    /// Put the lines of one hunk back the way they were.
    RestoreHunk(usize, bool, usize),
    /// Put one hunk of this change into the index, or take it back out.
    ///
    /// The middle word says which side of the index the hunk was read from,
    /// which is what says whether clicking it stages or unstages.
    ToggleHunkStaged(usize, bool, usize),
    /// Apply an inline action to the conflict starting on this file's line.
    ConflictAction(FileId, usize, ConflictAction),
    /// Start a comment on this hunk of this change, given which side of the
    /// index the hunk was read from.
    CommentOnHunk(usize, bool, usize),
    /// Carry a gesture down the numbers of the change in this place, from
    /// this line counted on this side, to the lines it reaches.
    DragComment(Option<ChangeId>, usize, CommentSide, usize, ResizeEvent),
    /// Start a comment on this line of this file in a pane of excerpts,
    /// counted from zero, or on the selection when the line is inside it.
    CommentExcerpt(FileId, usize),
    /// Start a comment on what the focused pane has selected.
    AddComment,
    /// Write the comment being written down.
    SaveComment,
    /// Stop writing the comment, throwing away what was written.
    CancelComment,
    /// Answer a press, a drag or a release of the pointer in the box a
    /// comment is written in.
    WriteComment(ResizePhase, Position, Position),
    /// Rewrite this comment.
    EditComment(CommentId),
    /// Take this comment away.
    DeleteComment(CommentId),
    /// Have the next line pressed take this comment, whose lines are gone.
    MoveComment(CommentId),
    /// Send every pending comment to the session's agent as one prompt.
    SendReview,
    /// Take away every comment that has not been sent.
    DiscardReview,
    /// Hide the comments already sent, or draw them again.
    ToggleSentComments,
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
    /// Show or hide the lines this change covers, or mark it when a modifier
    /// is held.
    ExpandChange(usize),
    /// Put everything the active project has changed into the index.
    StageAll,
    /// Take everything the active project has staged back out of the index.
    UnstageAll,
    /// Put this repository's commit message cursor where a press landed,
    /// selecting to it.
    ///
    /// A press in the message is also what gives it the keyboard, and makes
    /// its repository the active one, so this is the whole of how it is
    /// written in: there is nothing to focus first.
    WriteCommit(usize, ResizePhase, Position, Position),
    /// Commit what the index holds, saying what the message field holds.
    Commit,
    /// Commit what the index holds, then push the active branch.
    CommitAndPush,
    /// Rewrite the active repository's last commit.
    Amend,
    /// Open a prompt to save changes in a stash.
    StashPush,
    /// Choose a stash for this action.
    ShowStashes(crate::review::StashAction),
    /// Confirm dropping this stash.
    DropStash(usize),
    /// Act on the stash after confirmation.
    ConfirmDropStash(usize),
    /// Ask before rewriting a pushed commit.
    ConfirmAmend,
    /// Ask before skipping the stopped commit.
    SkipOperation,
    /// Skip the stopped commit after confirmation.
    ConfirmSkipOperation,
    /// Ask before discarding the active repository's merge resolution.
    AbortMerge,
    /// Abort the active repository's merge after confirmation.
    ConfirmAbortMerge,
    /// Ask which agent to start in the active project's worktree.
    NewAgentSession,
    /// Open the agent selector beside the status-bar control.
    ShowStatusAgents,
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
    /// Refuse this session's permission request under this ticket.
    DenyAgent(TalkId, u64),
    /// Give the keyboard to the box the field in this place of the form the
    /// agent put under this ticket is written in, picking it where it is the
    /// reader's own answer to a choice.
    TypeAnswer(TalkId, u64, usize),
    /// Put the caret of the box the field in this place of the form the
    /// agent put under this ticket is written in where a press landed,
    /// selecting to it.
    WriteAnswer(TalkId, u64, usize, ResizePhase, Position, Position),
    /// Send the form the agent put under this ticket, filled in as it is.
    SendAnswer(TalkId, u64),
    /// Choose the alternative in the last place, of the field in the place
    /// before it, of the form the agent put under this ticket.
    ChooseAnswer(TalkId, u64, usize, usize),
    /// Show the page in this place of the form the agent put under this ticket.
    ShowAnswerPage(TalkId, u64, usize),
    /// Fold the form the agent put under this ticket down to its tabs, or open it again.
    FoldAnswer(TalkId, u64),
    /// Walk away from the question the agent put under this ticket.
    CancelAnswer(TalkId, u64),
    /// Open the page the agent sent the reader to under this ticket.
    OpenAnswerLink(TalkId, u64),
    /// Log this session's agent in by the way it offered in this place.
    LogInAgent(TalkId, usize),
    /// Pick out this session's transcript from where a press landed to where
    /// the pointer has been dragged since.
    SelectAgentText(TalkId, ResizePhase, Point, Point),
    /// Drag this session's scrollbar, so many pixels of the conversation to
    /// a pixel of travel.
    ScrollAgent(TalkId, ResizeEvent, f32),
    /// Drag the scrollbar of this session's offered commands, so many pixels
    /// of the list to a pixel of travel.
    ScrollAgentCommands(TalkId, ResizeEvent, f32),
    /// Open or close tool or thinking details in this session's transcript.
    ToggleAgentDetails(TalkId, usize),
    /// Toggles the full output and descendants of one tool call.
    ToggleAgentCard(TalkId, usize),
    /// Follow the link this session's pane drew in this place: open the
    /// file it names, or the address in the browser.
    FollowAgentLink(TalkId, usize),
    /// Choose files to add to this agent's next prompt.
    AttachAgentFiles(TalkId),
    /// Remove an attachment from this agent's next prompt.
    RemoveAgentAttachment(TalkId, usize),
    /// Put the command this session is offering in this place into its prompt.
    TakeAgentCommand(TalkId, usize),
    /// Ask which of its agent's modes to put this session into.
    ShowAgentModes(TalkId),
    /// Put this session into the mode after the one it is in.
    CycleAgentMode(TalkId),
    /// Open the list of MCP servers this session's agent was given.
    ShowAgentMcp(TalkId),
    /// Open the settings page where MCP servers are managed.
    ManageMcpServers,
    /// Open the menu of what can be done to the text of this session's transcript.
    ShowAgentTextMenu(TalkId, Option<usize>),
    /// Copy a reply, choosing formatting explicitly or from the modifiers at the press.
    CopyAgentReply(TalkId, usize, Option<bool>),
    /// Copy what is picked out of this session's transcript.
    CopyAgentText(TalkId),
    /// Pick out the whole of this session's transcript.
    SelectAllAgentText(TalkId),
    /// Start this session's agent again, carrying on the conversation it was in.
    ReconnectAgent(TalkId),
    /// Log this session's agent out.
    LogOutAgent(TalkId),
    /// List this session's saved conversations to choose one to forget.
    ShowAgentDeletions(TalkId),
    /// Act on the knob in this place: ask which value, or flip the switch.
    PressKnob(TalkId, usize),
    /// Set the knob in this place to the value in that place, from inside the
    /// choices it is shown among, which stay open.
    SetAgentKnob(TalkId, usize, usize),
    /// Drag the scrollbar of this session's prompt, with the rows one pixel
    /// of the drag is worth.
    DragAgentPrompt(TalkId, ResizeEvent, f32),
    /// Flip the switch in this place, from inside the choices it is shown
    /// among, which stay open.
    FlipAgentKnob(TalkId, usize),
    /// Start naming one of this session's commands, in its prompt.
    StartAgentCommand(TalkId),
    /// Start naming a locally installed skill in this session's prompt.
    StartAgentSkill(TalkId),
    /// List saved sessions from this agent for the current worktree.
    ShowAgentHistory(TalkId),
    /// Stop the turn this session is running.
    StopAgentTurn(TalkId),
    /// Open the pane this session is read in, in whichever worktree it is.
    ShowAgent(TalkId),
    /// Go to what this notice is about, and let go of it.
    FollowNotice(NoticeId),
    /// Activate an explicit installation notification control.
    ActOnNotification(NoticeId, NotificationAction),
    /// Scroll a notification message through its shared viewport.
    ScrollNotification(NoticeId, ResizeEvent, f32),
    /// Let go of this notice without going anywhere.
    DismissNotice(NoticeId),
    /// Open the menu of things that can be done to the box being written in.
    ShowInputMenu,
    /// Carry out this command in whatever box has the keyboard.
    EditText(Action),
    /// Drag the magnified diagram at this place in this rendered file.
    PanDiagram(FileId, usize, ResizeEvent),
    /// Take this step on the zoom of the diagram at this place in this
    /// rendered file.
    ZoomDiagram(FileId, usize, DiagramZoom),
}
