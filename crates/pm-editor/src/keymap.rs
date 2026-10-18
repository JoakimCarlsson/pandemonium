//! The keys that work the editor: what each action is, the presets that bind keys to them as other editors do, the reader's own bindings over a preset, and finding the action a press of keys asks for.
//!
//! A binding is written as the chords it takes, one after another, each its
//! modifiers and its key joined by `+`: `ctrl+shift+k`, or `ctrl+k ctrl+i`
//! for two in a row. Keys are written as they are on a keyboard laid out in
//! US English, so `ctrl+shift+[` is the binding a press of Control, Shift
//! and the key that types `[` makes, whatever that key types with Shift.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Something the keys can ask the editor to do.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Action {
    /// Lists every action, to carry one out by typing part of its name.
    CommandPalette,
    /// Opens the settings.
    OpenSettings,
    /// Lists the sessions, to go to one by typing part of its name.
    GoToSession,
    /// Starts a new session in the project shown.
    NewSession,
    /// Adds a project to the window.
    AddProject,
    /// Shows every session of every project.
    ShowHistory,
    /// Shows the extensions there are to install.
    ShowExtensions,
    /// Goes back to the page shown before.
    WorkspaceBack,
    /// Goes forward to the page gone back from.
    WorkspaceForward,
    /// Closes the window.
    Quit,
    /// Shows the conversation of the session.
    ShowChat,
    /// Shows what the session changed.
    ShowDiff,
    /// Stops the agent of the session.
    StopAgent,
    /// Shows or hides the terminal.
    ToggleTerminal,
    /// Opens another terminal.
    NewTerminal,
    /// Shows or hides the source control panel.
    ToggleSourceControl,
    /// Lists the models of the agent, to pick one.
    ChangeModel,
    /// Lists the modes of the agent, to pick one.
    ChangeMode,
    /// Lists the settings of the agent, to change one.
    ChangeAgentSetting,
    /// Switches the agent to its next mode.
    CycleMode,
    /// Allows what the agent asked to do, this once.
    AllowRequest,
    /// Allows what the agent asked to do, from now on.
    AlwaysAllowRequest,
    /// Refuses what the agent asked to do.
    RejectRequest,
    /// Gives the session a new title.
    RenameSession,
    /// Archives the session.
    ArchiveSession,
    /// Copies the folder the session works in.
    CopySessionPath,
    /// Copies the branch the session is on.
    CopyBranchName,
    /// Shows the session's folder in the file manager.
    OpenSessionFolder,
    /// Lists the reader's messages, to take the conversation back to before one.
    RewindConversation,
    /// Lists the agent's replies, to start a session branching off after one.
    ForkConversation,
    /// Lists the reader's messages, to put the files back as they were before one.
    RestoreFiles,
    /// Gives the keyboard to the prompt.
    FocusPrompt,
    /// Closes the tab shown.
    CloseTab,
    /// Closes every tab but the one shown.
    CloseOtherTabs,
    /// Closes every tab without unsaved changes.
    CloseSavedTabs,
    /// Closes every tab.
    CloseAllTabs,
    /// Pins the tab shown, or unpins it.
    PinTab,
    /// Makes the file shown read-only, or editable again.
    ToggleReadOnly,
    /// Copies the path of the file shown on its machine.
    CopyPath,
    /// Copies the path of the file shown from the root of the project.
    CopyRelativePath,
    /// Shows the file shown in the project's files.
    RevealInProject,
    /// Splits the pane worked in to the right.
    SplitRight,
    /// Splits the pane worked in to the left.
    SplitLeft,
    /// Splits the pane worked in upwards.
    SplitUp,
    /// Splits the pane worked in downwards.
    SplitDown,
    /// Works in the next pane.
    FocusNextPane,
    /// Works in the pane before.
    FocusPreviousPane,
    /// Lists the themes, to pick one.
    ChangeTheme,
    /// Switches between the light and the dark theme.
    ToggleTheme,
    /// Lists the keymaps, to pick one.
    ChangeKeymap,
    /// Makes the text bigger.
    ZoomIn,
    /// Makes the text smaller.
    ZoomOut,
    /// Puts the text back to its usual size.
    ResetZoom,
    /// Copies a link to the line on the repository's host.
    CopyPermalink,
    /// Opens the line on the repository's host.
    OpenPermalink,
    /// Lists the commits that changed the file.
    FileHistory,
    /// Opens a terminal in the file's folder.
    OpenTerminalHere,
    /// Lists the functions it calls.
    OutgoingCalls,
    /// Sets or clears the breakpoint on the caret's line.
    ToggleBreakpoint,
    /// Lists what the project can be debugged as, or runs the paused program on.
    StartDebugging,
    /// Lists the programs running, to attach a debugger to one.
    AttachDebugger,
    /// Ends the program being debugged.
    StopDebugging,
    /// Pauses the program being debugged.
    PauseDebugging,
    /// Runs the paused program to the next line.
    StepOver,
    /// Runs the paused program into the call on its line.
    StepInto,
    /// Runs the paused program out of the frame it is in.
    StepOut,
    /// Copies the selection without the indentation its lines share.
    CopyWithoutIndentation,
    /// Undoes the last change to the file.
    Undo,
    /// Does again what was undone.
    Redo,
    /// Commits the staged changes.
    Commit,
    /// Commits the staged changes and pushes them.
    CommitAndPush,
    /// Pulls from the remote.
    Pull,
    /// Pushes to the remote.
    Push,
    /// Fetches from every remote.
    Fetch,
    /// Stashes the changes.
    StashChanges,
    /// Lists the branches, to check one out.
    CheckoutBranch,
    /// Clones a repository as a new project.
    CloneRepository,
    /// Reads the changes and the graph again.
    RefreshSourceControl,
    /// Pulls from the remote, replaying the branch's own commits on top.
    PullRebase,
    /// Pushes to the remote, overwriting only what was seen.
    ForcePush,
    /// Pulls from the remote and then pushes to it.
    Sync,
    /// Stages every change.
    StageAll,
    /// Unstages every change.
    UnstageAll,
    /// Rewrites the last commit with the staged changes.
    CommitAmend,
    /// Finishes the merge, rebase or cherry-pick that stopped.
    ContinueOperation,
    /// Leaves out the commit the rebase or cherry-pick stopped on.
    SkipCommit,
    /// Gives up the merge, rebase or cherry-pick that stopped.
    AbortOperation,
    /// Lists the stashes, to apply one and keep it.
    StashApply,
    /// Lists the stashes, to apply one and remove it.
    StashPop,
    /// Lists the stashes, to remove one.
    StashDrop,
    /// Makes a branch by the name typed and checks it out.
    CreateBranch,
    /// Shows or hides the graph of commits.
    ToggleGraph,
    /// Goes to where the name at the caret is defined.
    GoToDefinition,
    /// Goes to where it is declared.
    GoToDeclaration,
    /// Goes to where its type is defined.
    GoToTypeDefinition,
    /// Goes to where it is implemented.
    GoToImplementation,
    /// Lists where it is used.
    FindReferences,
    /// Lists the functions that call it.
    IncomingCalls,
    /// Gives it a new name across the project.
    Rename,
    /// Formats the file.
    Format,
    /// Lists the changes the language server offers.
    CodeActions,
    /// Shows what the language server says about it.
    Hover,
    /// Lists what could be written at the caret.
    Complete,
    /// Shows the signature of the call the caret is in.
    SignatureHelp,
    /// Goes back to where the caret was before a jump.
    GoBack,
    /// Goes forward again.
    GoForward,
    /// Comments the lines out, or back in.
    ToggleComment,
    /// Indents the lines.
    Indent,
    /// Outdents the lines.
    Outdent,
    /// Moves the lines up.
    MoveLineUp,
    /// Moves the lines down.
    MoveLineDown,
    /// Writes the lines again above.
    DuplicateLineUp,
    /// Writes the lines again below.
    DuplicateLineDown,
    /// Deletes the lines.
    DeleteLine,
    /// Grows the selection along the syntax tree.
    ExpandSelection,
    /// Shrinks it back.
    ShrinkSelection,
    /// Goes to the next problem.
    NextProblem,
    /// Goes to the problem before.
    PreviousProblem,
    /// Selects the next place the selection comes as well.
    AddNextOccurrence,
    /// Selects every place it comes.
    SelectAllOccurrences,
    /// Adds a caret on the line above.
    AddCaretAbove,
    /// Adds a caret on the line below.
    AddCaretBelow,
    /// Selects the line.
    SelectLine,
    /// Joins the line with the next.
    JoinLines,
    /// Goes to the matching bracket.
    MatchingBracket,
    /// Starts a new line below.
    NewlineBelow,
    /// Starts a new line above.
    NewlineAbove,
    /// Folds the block around the caret.
    Fold,
    /// Unfolds it.
    Unfold,
    /// Finds in the file.
    Find,
    /// Finds and replaces in the file.
    Replace,
    /// Selects the next match.
    FindNext,
    /// Selects the match before.
    FindPrevious,
    /// Goes to a line by its number.
    GoToLine,
    /// Lists the items of the file.
    Outline,
    /// Lists the symbols of the project.
    ProjectSymbols,
    /// Finds a file of the project.
    FileFinder,
    /// Searches the files of the project.
    SearchProject,
    /// Lists the problems of the project.
    Problems,
    /// Shows the file in the file manager.
    RevealInFileManager,
    /// Saves the file.
    Save,
    /// Puts the text marked in the shell into the prompt of the chat.
    TerminalAddToChat,
    /// Copies the text marked in the shell.
    TerminalCopy,
    /// Pastes the clipboard into the shell.
    TerminalPaste,
    /// Marks every line of the shell.
    TerminalSelectAll,
    /// Clears the shell's window and its history, keeping the line the cursor is on.
    TerminalClear,
    /// Ends the shell.
    KillTerminal,
}

impl Action {
    /// Every action, in the order the keymap lists them.
    pub const ALL: [Self; 147] = [
        Self::CommandPalette,
        Self::OpenSettings,
        Self::GoToSession,
        Self::NewSession,
        Self::AddProject,
        Self::ShowHistory,
        Self::ShowExtensions,
        Self::WorkspaceBack,
        Self::WorkspaceForward,
        Self::Quit,
        Self::ShowChat,
        Self::ShowDiff,
        Self::StopAgent,
        Self::ToggleTerminal,
        Self::NewTerminal,
        Self::ToggleSourceControl,
        Self::ChangeModel,
        Self::ChangeMode,
        Self::ChangeAgentSetting,
        Self::CycleMode,
        Self::AllowRequest,
        Self::AlwaysAllowRequest,
        Self::RejectRequest,
        Self::RenameSession,
        Self::ArchiveSession,
        Self::CopySessionPath,
        Self::CopyBranchName,
        Self::OpenSessionFolder,
        Self::RewindConversation,
        Self::ForkConversation,
        Self::RestoreFiles,
        Self::FocusPrompt,
        Self::CloseTab,
        Self::CloseOtherTabs,
        Self::CloseSavedTabs,
        Self::CloseAllTabs,
        Self::PinTab,
        Self::ToggleReadOnly,
        Self::CopyPath,
        Self::CopyRelativePath,
        Self::RevealInProject,
        Self::SplitRight,
        Self::SplitLeft,
        Self::SplitUp,
        Self::SplitDown,
        Self::FocusNextPane,
        Self::FocusPreviousPane,
        Self::ChangeTheme,
        Self::ToggleTheme,
        Self::ChangeKeymap,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ResetZoom,
        Self::CopyPermalink,
        Self::OpenPermalink,
        Self::FileHistory,
        Self::OpenTerminalHere,
        Self::OutgoingCalls,
        Self::ToggleBreakpoint,
        Self::StartDebugging,
        Self::AttachDebugger,
        Self::StopDebugging,
        Self::PauseDebugging,
        Self::StepOver,
        Self::StepInto,
        Self::StepOut,
        Self::CopyWithoutIndentation,
        Self::Undo,
        Self::Redo,
        Self::Commit,
        Self::CommitAndPush,
        Self::Pull,
        Self::Push,
        Self::Fetch,
        Self::StashChanges,
        Self::CheckoutBranch,
        Self::CloneRepository,
        Self::RefreshSourceControl,
        Self::PullRebase,
        Self::ForcePush,
        Self::Sync,
        Self::StageAll,
        Self::UnstageAll,
        Self::CommitAmend,
        Self::ContinueOperation,
        Self::SkipCommit,
        Self::AbortOperation,
        Self::StashApply,
        Self::StashPop,
        Self::StashDrop,
        Self::CreateBranch,
        Self::ToggleGraph,
        Self::GoToDefinition,
        Self::GoToDeclaration,
        Self::GoToTypeDefinition,
        Self::GoToImplementation,
        Self::FindReferences,
        Self::IncomingCalls,
        Self::GoBack,
        Self::GoForward,
        Self::Rename,
        Self::Format,
        Self::CodeActions,
        Self::Hover,
        Self::Complete,
        Self::SignatureHelp,
        Self::ToggleComment,
        Self::Indent,
        Self::Outdent,
        Self::MoveLineUp,
        Self::MoveLineDown,
        Self::DuplicateLineUp,
        Self::DuplicateLineDown,
        Self::DeleteLine,
        Self::JoinLines,
        Self::NewlineBelow,
        Self::NewlineAbove,
        Self::ExpandSelection,
        Self::ShrinkSelection,
        Self::SelectLine,
        Self::AddNextOccurrence,
        Self::SelectAllOccurrences,
        Self::AddCaretAbove,
        Self::AddCaretBelow,
        Self::MatchingBracket,
        Self::Fold,
        Self::Unfold,
        Self::NextProblem,
        Self::PreviousProblem,
        Self::Find,
        Self::Replace,
        Self::FindNext,
        Self::FindPrevious,
        Self::GoToLine,
        Self::Outline,
        Self::ProjectSymbols,
        Self::FileFinder,
        Self::SearchProject,
        Self::Problems,
        Self::RevealInFileManager,
        Self::Save,
        Self::TerminalAddToChat,
        Self::TerminalCopy,
        Self::TerminalPaste,
        Self::TerminalSelectAll,
        Self::TerminalClear,
        Self::KillTerminal,
    ];

    /// The name it is kept under in the settings.
    pub fn key(self) -> &'static str {
        match self {
            Self::CommandPalette => "command_palette",
            Self::OpenSettings => "open_settings",
            Self::GoToSession => "go_to_session",
            Self::NewSession => "new_session",
            Self::AddProject => "add_project",
            Self::ShowHistory => "show_history",
            Self::ShowExtensions => "show_extensions",
            Self::WorkspaceBack => "workspace_back",
            Self::WorkspaceForward => "workspace_forward",
            Self::Quit => "quit",
            Self::ShowChat => "show_chat",
            Self::ShowDiff => "show_diff",
            Self::StopAgent => "stop_agent",
            Self::ToggleTerminal => "toggle_terminal",
            Self::NewTerminal => "new_terminal",
            Self::ToggleSourceControl => "toggle_source_control",
            Self::ChangeModel => "change_model",
            Self::ChangeMode => "change_mode",
            Self::ChangeAgentSetting => "change_agent_setting",
            Self::CycleMode => "cycle_mode",
            Self::AllowRequest => "allow_request",
            Self::AlwaysAllowRequest => "always_allow_request",
            Self::RejectRequest => "reject_request",
            Self::RenameSession => "rename_session",
            Self::ArchiveSession => "archive_session",
            Self::CopySessionPath => "copy_session_path",
            Self::CopyBranchName => "copy_branch_name",
            Self::OpenSessionFolder => "open_session_folder",
            Self::RewindConversation => "rewind_conversation",
            Self::ForkConversation => "fork_conversation",
            Self::RestoreFiles => "restore_files",
            Self::FocusPrompt => "focus_prompt",
            Self::CloseTab => "close_tab",
            Self::CloseOtherTabs => "close_other_tabs",
            Self::CloseSavedTabs => "close_saved_tabs",
            Self::CloseAllTabs => "close_all_tabs",
            Self::PinTab => "pin_tab",
            Self::ToggleReadOnly => "toggle_read_only",
            Self::CopyPath => "copy_path",
            Self::CopyRelativePath => "copy_relative_path",
            Self::RevealInProject => "reveal_in_project",
            Self::SplitRight => "split_right",
            Self::SplitLeft => "split_left",
            Self::SplitUp => "split_up",
            Self::SplitDown => "split_down",
            Self::FocusNextPane => "focus_next_pane",
            Self::FocusPreviousPane => "focus_previous_pane",
            Self::ChangeTheme => "change_theme",
            Self::ToggleTheme => "toggle_theme",
            Self::ChangeKeymap => "change_keymap",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::ResetZoom => "reset_zoom",
            Self::CopyPermalink => "copy_permalink",
            Self::OpenPermalink => "open_permalink",
            Self::FileHistory => "file_history",
            Self::OpenTerminalHere => "open_terminal_here",
            Self::OutgoingCalls => "outgoing_calls",
            Self::ToggleBreakpoint => "toggle_breakpoint",
            Self::StartDebugging => "start_debugging",
            Self::AttachDebugger => "attach_debugger",
            Self::StopDebugging => "stop_debugging",
            Self::PauseDebugging => "pause_debugging",
            Self::StepOver => "step_over",
            Self::StepInto => "step_into",
            Self::StepOut => "step_out",
            Self::CopyWithoutIndentation => "copy_without_indentation",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Commit => "git_commit",
            Self::CommitAndPush => "git_commit_and_push",
            Self::Pull => "git_pull",
            Self::Push => "git_push",
            Self::Fetch => "git_fetch",
            Self::StashChanges => "git_stash",
            Self::CheckoutBranch => "git_checkout",
            Self::CloneRepository => "git_clone",
            Self::RefreshSourceControl => "git_refresh",
            Self::PullRebase => "git_pull_rebase",
            Self::ForcePush => "git_force_push",
            Self::Sync => "git_sync",
            Self::StageAll => "git_stage_all",
            Self::UnstageAll => "git_unstage_all",
            Self::CommitAmend => "git_amend",
            Self::ContinueOperation => "git_continue",
            Self::SkipCommit => "git_skip",
            Self::AbortOperation => "git_abort",
            Self::StashApply => "git_stash_apply",
            Self::StashPop => "git_stash_pop",
            Self::StashDrop => "git_stash_drop",
            Self::CreateBranch => "git_create_branch",
            Self::ToggleGraph => "git_toggle_graph",
            Self::GoToDefinition => "go_to_definition",
            Self::GoToDeclaration => "go_to_declaration",
            Self::GoToTypeDefinition => "go_to_type_definition",
            Self::GoToImplementation => "go_to_implementation",
            Self::FindReferences => "find_references",
            Self::IncomingCalls => "incoming_calls",
            Self::Rename => "rename",
            Self::Format => "format",
            Self::CodeActions => "code_actions",
            Self::Hover => "hover",
            Self::Complete => "complete",
            Self::SignatureHelp => "signature_help",
            Self::GoBack => "go_back",
            Self::GoForward => "go_forward",
            Self::ToggleComment => "toggle_comment",
            Self::Indent => "indent",
            Self::Outdent => "outdent",
            Self::MoveLineUp => "move_line_up",
            Self::MoveLineDown => "move_line_down",
            Self::DuplicateLineUp => "duplicate_line_up",
            Self::DuplicateLineDown => "duplicate_line_down",
            Self::DeleteLine => "delete_line",
            Self::ExpandSelection => "expand_selection",
            Self::ShrinkSelection => "shrink_selection",
            Self::NextProblem => "next_problem",
            Self::PreviousProblem => "previous_problem",
            Self::AddNextOccurrence => "add_next_occurrence",
            Self::SelectAllOccurrences => "select_all_occurrences",
            Self::AddCaretAbove => "add_caret_above",
            Self::AddCaretBelow => "add_caret_below",
            Self::SelectLine => "select_line",
            Self::JoinLines => "join_lines",
            Self::MatchingBracket => "matching_bracket",
            Self::NewlineBelow => "newline_below",
            Self::NewlineAbove => "newline_above",
            Self::Fold => "fold",
            Self::Unfold => "unfold",
            Self::Find => "find",
            Self::Replace => "replace",
            Self::FindNext => "find_next",
            Self::FindPrevious => "find_previous",
            Self::GoToLine => "go_to_line",
            Self::Outline => "outline",
            Self::ProjectSymbols => "project_symbols",
            Self::FileFinder => "file_finder",
            Self::SearchProject => "search_project",
            Self::Problems => "problems",
            Self::RevealInFileManager => "reveal_in_file_manager",
            Self::Save => "save",
            Self::TerminalAddToChat => "terminal_add_to_chat",
            Self::TerminalCopy => "terminal_copy",
            Self::TerminalPaste => "terminal_paste",
            Self::TerminalSelectAll => "terminal_select_all",
            Self::TerminalClear => "terminal_clear",
            Self::KillTerminal => "kill_terminal",
        }
    }

    /// What the keymap calls it.
    pub fn title(self) -> &'static str {
        match self {
            Self::CommandPalette => "Show All Commands",
            Self::OpenSettings => "Open Settings",
            Self::GoToSession => "Go to Session",
            Self::NewSession => "New Session",
            Self::AddProject => "New Workspace",
            Self::ShowHistory => "Show History",
            Self::ShowExtensions => "Show Extensions",
            Self::WorkspaceBack => "Go Back to Previous Page",
            Self::WorkspaceForward => "Go Forward to Next Page",
            Self::Quit => "Quit",
            Self::ShowChat => "Show Chat",
            Self::ShowDiff => "Show Diff",
            Self::StopAgent => "Stop Agent",
            Self::ToggleTerminal => "Toggle Terminal",
            Self::NewTerminal => "New Terminal",
            Self::ToggleSourceControl => "Toggle Source Control",
            Self::ChangeModel => "Change Model",
            Self::ChangeMode => "Change Mode",
            Self::ChangeAgentSetting => "Change Agent Setting",
            Self::CycleMode => "Cycle Mode",
            Self::AllowRequest => "Allow Request",
            Self::AlwaysAllowRequest => "Always Allow Request",
            Self::RejectRequest => "Reject Request",
            Self::RenameSession => "Rename Session",
            Self::ArchiveSession => "Archive Session",
            Self::CopySessionPath => "Copy Session Path",
            Self::CopyBranchName => "Copy Branch Name",
            Self::OpenSessionFolder => "Open Session Folder",
            Self::RewindConversation => "Rewind Conversation",
            Self::ForkConversation => "Fork Conversation",
            Self::RestoreFiles => "Restore Files",
            Self::FocusPrompt => "Focus Prompt",
            Self::CloseTab => "Close Tab",
            Self::CloseOtherTabs => "Close Other Tabs",
            Self::CloseSavedTabs => "Close Saved Tabs",
            Self::CloseAllTabs => "Close All Tabs",
            Self::PinTab => "Pin Tab",
            Self::ToggleReadOnly => "Toggle Read-Only",
            Self::CopyPath => "Copy Path",
            Self::CopyRelativePath => "Copy Relative Path",
            Self::RevealInProject => "Reveal in Project",
            Self::SplitRight => "Split Right",
            Self::SplitLeft => "Split Left",
            Self::SplitUp => "Split Up",
            Self::SplitDown => "Split Down",
            Self::FocusNextPane => "Focus Next Pane",
            Self::FocusPreviousPane => "Focus Previous Pane",
            Self::ChangeTheme => "Change Theme",
            Self::ToggleTheme => "Toggle Light and Dark Theme",
            Self::ChangeKeymap => "Change Keymap",
            Self::ZoomIn => "Increase Text Size",
            Self::ZoomOut => "Decrease Text Size",
            Self::ResetZoom => "Reset Text Size",
            Self::CopyPermalink => "Copy Permalink",
            Self::OpenPermalink => "Open Permalink",
            Self::FileHistory => "Show File History",
            Self::OpenTerminalHere => "Open Terminal at File",
            Self::OutgoingCalls => "Show Outgoing Calls",
            Self::ToggleBreakpoint => "Toggle Breakpoint",
            Self::StartDebugging => "Start Debugging",
            Self::AttachDebugger => "Attach Debugger to Process",
            Self::StopDebugging => "Stop Debugging",
            Self::PauseDebugging => "Pause Debugging",
            Self::StepOver => "Step Over",
            Self::StepInto => "Step Into",
            Self::StepOut => "Step Out",
            Self::CopyWithoutIndentation => "Copy Without Indentation",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Commit => "Commit",
            Self::CommitAndPush => "Commit and Push",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Fetch => "Fetch",
            Self::StashChanges => "Stash Changes",
            Self::CheckoutBranch => "Checkout Branch",
            Self::CloneRepository => "Clone Repository",
            Self::RefreshSourceControl => "Refresh Source Control",
            Self::PullRebase => "Pull (Rebase)",
            Self::ForcePush => "Push (Force)",
            Self::Sync => "Sync",
            Self::StageAll => "Stage All Changes",
            Self::UnstageAll => "Unstage All Changes",
            Self::CommitAmend => "Commit (Amend)",
            Self::ContinueOperation => "Continue Merge or Rebase",
            Self::SkipCommit => "Skip Commit",
            Self::AbortOperation => "Abort Merge or Rebase",
            Self::StashApply => "Apply Stash",
            Self::StashPop => "Pop Stash",
            Self::StashDrop => "Drop Stash",
            Self::CreateBranch => "Create Branch",
            Self::ToggleGraph => "Toggle Commit Graph",
            Self::GoToDefinition => "Go to Definition",
            Self::GoToDeclaration => "Go to Declaration",
            Self::GoToTypeDefinition => "Go to Type Definition",
            Self::GoToImplementation => "Go to Implementation",
            Self::FindReferences => "Find All References",
            Self::IncomingCalls => "Show Incoming Calls",
            Self::Rename => "Rename Symbol",
            Self::Format => "Format Buffer",
            Self::CodeActions => "Show Code Actions",
            Self::Hover => "Show Hover",
            Self::Complete => "Show Completions",
            Self::SignatureHelp => "Show Signature Help",
            Self::GoBack => "Go Back",
            Self::GoForward => "Go Forward",
            Self::ToggleComment => "Toggle Comment",
            Self::Indent => "Indent",
            Self::Outdent => "Outdent",
            Self::MoveLineUp => "Move Line Up",
            Self::MoveLineDown => "Move Line Down",
            Self::DuplicateLineUp => "Duplicate Line Up",
            Self::DuplicateLineDown => "Duplicate Line Down",
            Self::DeleteLine => "Delete Line",
            Self::ExpandSelection => "Expand Selection",
            Self::ShrinkSelection => "Shrink Selection",
            Self::NextProblem => "Next Problem",
            Self::PreviousProblem => "Previous Problem",
            Self::AddNextOccurrence => "Add Next Occurrence",
            Self::SelectAllOccurrences => "Select All Occurrences",
            Self::AddCaretAbove => "Add Caret Above",
            Self::AddCaretBelow => "Add Caret Below",
            Self::SelectLine => "Select Line",
            Self::JoinLines => "Join Lines",
            Self::MatchingBracket => "Go to Matching Bracket",
            Self::NewlineBelow => "New Line Below",
            Self::NewlineAbove => "New Line Above",
            Self::Fold => "Fold",
            Self::Unfold => "Unfold",
            Self::Find => "Find",
            Self::Replace => "Replace",
            Self::FindNext => "Find Next",
            Self::FindPrevious => "Find Previous",
            Self::GoToLine => "Go to Line",
            Self::Outline => "Go to Symbol in File",
            Self::ProjectSymbols => "Go to Symbol in Project",
            Self::FileFinder => "Go to File",
            Self::SearchProject => "Search in Project",
            Self::Problems => "Show Problems",
            Self::RevealInFileManager => "Reveal in File Manager",
            Self::Save => "Save",
            Self::TerminalAddToChat => "Add Terminal Selection to Chat",
            Self::TerminalCopy => "Copy",
            Self::TerminalPaste => "Paste",
            Self::TerminalSelectAll => "Select All",
            Self::TerminalClear => "Clear",
            Self::KillTerminal => "Kill Terminal",
        }
    }

    /// What the list of commands calls it: its title, after `Git: ` for the commands of source control.
    pub fn palette_title(self) -> String {
        match self.group() {
            "Source Control" => format!("Git: {}", self.title()),
            _ => self.title().to_owned(),
        }
    }

    /// Other words it is known by, which find it in the list of commands too.
    pub fn keywords(self) -> &'static str {
        match self {
            Self::AddProject => "add project open folder repository",
            Self::NewSession => "start agent chat",
            Self::GoToSession => "switch project workspace",
            Self::CloneRepository => "new workspace project",
            Self::OpenSettings => "preferences configure",
            Self::CommandPalette => "command palette",
            _ => "",
        }
    }

    /// The group the keymap lists it under.
    pub fn group(self) -> &'static str {
        match self {
            Self::ChangeModel
            | Self::ChangeMode
            | Self::ChangeAgentSetting
            | Self::CycleMode
            | Self::AllowRequest
            | Self::AlwaysAllowRequest
            | Self::RejectRequest
            | Self::RenameSession
            | Self::ArchiveSession
            | Self::CopySessionPath
            | Self::CopyBranchName
            | Self::OpenSessionFolder
            | Self::RewindConversation
            | Self::ForkConversation
            | Self::RestoreFiles
            | Self::FocusPrompt => "Session",
            Self::CloseTab
            | Self::CloseOtherTabs
            | Self::CloseSavedTabs
            | Self::CloseAllTabs
            | Self::PinTab
            | Self::ToggleReadOnly
            | Self::CopyPath
            | Self::CopyRelativePath
            | Self::RevealInProject
            | Self::SplitRight
            | Self::SplitLeft
            | Self::SplitUp
            | Self::SplitDown
            | Self::FocusNextPane
            | Self::FocusPreviousPane => "Tabs",
            Self::ChangeTheme
            | Self::ToggleTheme
            | Self::ChangeKeymap
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ResetZoom => "Window",
            Self::CopyPermalink
            | Self::OpenPermalink
            | Self::FileHistory
            | Self::OpenTerminalHere => "File",
            Self::OutgoingCalls => "Navigation",
            Self::ToggleBreakpoint
            | Self::StartDebugging
            | Self::AttachDebugger
            | Self::StopDebugging
            | Self::PauseDebugging
            | Self::StepOver
            | Self::StepInto
            | Self::StepOut => "Debug",
            Self::CopyWithoutIndentation | Self::Undo | Self::Redo => "Editing",
            Self::CommandPalette | Self::OpenSettings => "Window",
            Self::GoToSession
            | Self::NewSession
            | Self::AddProject
            | Self::ShowHistory
            | Self::ShowExtensions
            | Self::WorkspaceBack
            | Self::WorkspaceForward
            | Self::Quit => "Window",
            Self::ShowChat
            | Self::ShowDiff
            | Self::StopAgent
            | Self::ToggleTerminal
            | Self::NewTerminal
            | Self::ToggleSourceControl => "Session",
            Self::Commit
            | Self::CommitAndPush
            | Self::Pull
            | Self::Push
            | Self::Fetch
            | Self::StashChanges
            | Self::CheckoutBranch
            | Self::CloneRepository
            | Self::RefreshSourceControl
            | Self::PullRebase
            | Self::ForcePush
            | Self::Sync
            | Self::StageAll
            | Self::UnstageAll
            | Self::CommitAmend
            | Self::ContinueOperation
            | Self::SkipCommit
            | Self::AbortOperation
            | Self::StashApply
            | Self::StashPop
            | Self::StashDrop
            | Self::CreateBranch
            | Self::ToggleGraph => "Source Control",
            Self::GoToDefinition
            | Self::GoToDeclaration
            | Self::GoToTypeDefinition
            | Self::GoToImplementation
            | Self::FindReferences
            | Self::IncomingCalls
            | Self::GoBack
            | Self::GoForward
            | Self::NextProblem
            | Self::PreviousProblem
            | Self::MatchingBracket
            | Self::GoToLine
            | Self::Outline
            | Self::ProjectSymbols
            | Self::FileFinder
            | Self::Problems => "Navigation",
            Self::Rename
            | Self::Format
            | Self::CodeActions
            | Self::Hover
            | Self::Complete
            | Self::SignatureHelp => "Code Intelligence",
            Self::AddNextOccurrence
            | Self::SelectAllOccurrences
            | Self::AddCaretAbove
            | Self::AddCaretBelow
            | Self::SelectLine
            | Self::ExpandSelection
            | Self::ShrinkSelection => "Selection",
            Self::Find
            | Self::Replace
            | Self::FindNext
            | Self::FindPrevious
            | Self::SearchProject => "Search",
            Self::RevealInFileManager | Self::Save => "File",
            action if action.in_terminal() => "Terminal",
            _ => "Editing",
        }
    }

    /// Whether its keys work anywhere in the window, not only while a file has the keyboard.
    pub fn everywhere(self) -> bool {
        matches!(
            self,
            Self::CommandPalette
                | Self::OpenSettings
                | Self::GoToSession
                | Self::NewSession
                | Self::AddProject
                | Self::ShowHistory
                | Self::ShowExtensions
                | Self::WorkspaceBack
                | Self::WorkspaceForward
                | Self::Quit
                | Self::ShowChat
                | Self::ShowDiff
                | Self::StopAgent
                | Self::ToggleTerminal
                | Self::NewTerminal
                | Self::ToggleSourceControl
                | Self::StartDebugging
                | Self::AttachDebugger
                | Self::StopDebugging
                | Self::PauseDebugging
                | Self::StepOver
                | Self::StepInto
                | Self::StepOut
                | Self::Commit
                | Self::CommitAndPush
                | Self::Pull
                | Self::Push
                | Self::Fetch
                | Self::StashChanges
                | Self::CheckoutBranch
                | Self::CloneRepository
                | Self::RefreshSourceControl
                | Self::PullRebase
                | Self::ForcePush
                | Self::Sync
                | Self::StageAll
                | Self::UnstageAll
                | Self::CommitAmend
                | Self::ContinueOperation
                | Self::SkipCommit
                | Self::AbortOperation
                | Self::StashApply
                | Self::StashPop
                | Self::StashDrop
                | Self::CreateBranch
                | Self::ToggleGraph
                | Self::ChangeModel
                | Self::ChangeMode
                | Self::ChangeAgentSetting
                | Self::CycleMode
                | Self::AllowRequest
                | Self::AlwaysAllowRequest
                | Self::RejectRequest
                | Self::RenameSession
                | Self::ArchiveSession
                | Self::CopySessionPath
                | Self::CopyBranchName
                | Self::OpenSessionFolder
                | Self::RewindConversation
                | Self::ForkConversation
                | Self::RestoreFiles
                | Self::FocusPrompt
                | Self::CloseTab
                | Self::CloseOtherTabs
                | Self::CloseSavedTabs
                | Self::CloseAllTabs
                | Self::PinTab
                | Self::ToggleReadOnly
                | Self::CopyPath
                | Self::CopyRelativePath
                | Self::RevealInProject
                | Self::SplitRight
                | Self::SplitLeft
                | Self::SplitUp
                | Self::SplitDown
                | Self::FocusNextPane
                | Self::FocusPreviousPane
                | Self::ChangeTheme
                | Self::ToggleTheme
                | Self::ChangeKeymap
                | Self::ZoomIn
                | Self::ZoomOut
                | Self::ResetZoom
                | Self::FileFinder
                | Self::Outline
                | Self::ProjectSymbols
                | Self::SearchProject
                | Self::Problems
        )
    }

    /// Whether its keys work while a shell has the keyboard, and only then.
    pub fn in_terminal(self) -> bool {
        matches!(
            self,
            Self::TerminalAddToChat
                | Self::TerminalCopy
                | Self::TerminalPaste
                | Self::TerminalSelectAll
                | Self::TerminalClear
                | Self::KillTerminal
        )
    }

    /// The action kept under the name `key`.
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.key() == key)
    }
}

/// A set of bindings as another editor has them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Pandemonium's own.
    #[default]
    Pandemonium,
    /// As Visual Studio Code has them.
    VsCode,
    /// As the JetBrains editors have them.
    JetBrains,
    /// As Sublime Text has them.
    Sublime,
}

impl Preset {
    /// Every preset, in the order they are offered.
    pub const ALL: [Self; 4] = [
        Self::Pandemonium,
        Self::VsCode,
        Self::JetBrains,
        Self::Sublime,
    ];

    /// What it is called.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Pandemonium => "Pandemonium",
            Self::VsCode => "VS Code",
            Self::JetBrains => "JetBrains",
            Self::Sublime => "Sublime Text",
        }
    }

    /// The bindings it puts over Pandemonium's own; an action with none listed is left without keys.
    fn changes(self) -> &'static [(Action, &'static [&'static str])] {
        use Action::*;
        match self {
            Self::Pandemonium => &[],
            Self::VsCode => &[
                (ZoomOut, &["ctrl+-"]),
                (GoToTypeDefinition, &[]),
                (GoToImplementation, &["ctrl+f12"]),
                (FindReferences, &["shift+f12", "alt+shift+f12"]),
                (GoBack, &["ctrl+alt+-"]),
                (DuplicateLineUp, &["ctrl+alt+shift+up"]),
                (DuplicateLineDown, &["ctrl+alt+shift+down"]),
                (AddCaretAbove, &["alt+shift+up"]),
                (AddCaretBelow, &["alt+shift+down"]),
                (JoinLines, &[]),
                (MatchingBracket, &["ctrl+shift+\\"]),
            ],
            Self::JetBrains => &[
                (CommandPalette, &["ctrl+shift+a"]),
                (OpenSettings, &["ctrl+alt+s"]),
                (GoToSession, &["ctrl+e"]),
                (CloseTab, &["ctrl+f4"]),
                (ZoomIn, &[]),
                (GoToDefinition, &["ctrl+b"]),
                (GoToTypeDefinition, &["ctrl+shift+b"]),
                (GoToImplementation, &["ctrl+alt+b"]),
                (FindReferences, &["alt+f7"]),
                (IncomingCalls, &["ctrl+alt+h"]),
                (Rename, &["shift+f6"]),
                (Format, &["ctrl+alt+l"]),
                (CodeActions, &["alt+enter"]),
                (Hover, &["ctrl+q"]),
                (SignatureHelp, &["ctrl+p"]),
                (GoBack, &["ctrl+alt+left"]),
                (GoForward, &["ctrl+alt+right"]),
                (MoveLineUp, &["alt+shift+up"]),
                (MoveLineDown, &["alt+shift+down"]),
                (DuplicateLineUp, &[]),
                (DuplicateLineDown, &["ctrl+d"]),
                (DeleteLine, &["ctrl+y"]),
                (ExpandSelection, &["ctrl+w"]),
                (ShrinkSelection, &["ctrl+shift+w"]),
                (NextProblem, &["f2"]),
                (PreviousProblem, &["shift+f2"]),
                (AddNextOccurrence, &["alt+j"]),
                (SelectAllOccurrences, &["ctrl+alt+shift+j"]),
                (SelectLine, &[]),
                (JoinLines, &["ctrl+shift+j"]),
                (MatchingBracket, &["ctrl+shift+m"]),
                (NewlineBelow, &["shift+enter"]),
                (NewlineAbove, &["ctrl+alt+enter"]),
                (Fold, &["ctrl+-"]),
                (Unfold, &["ctrl+="]),
                (Replace, &["ctrl+r"]),
                (Outline, &["ctrl+f12"]),
                (ProjectSymbols, &["ctrl+alt+shift+n"]),
                (FileFinder, &["ctrl+shift+n"]),
                (Problems, &["alt+6"]),
            ],
            Self::Sublime => &[
                (GoToSession, &["ctrl+alt+p"]),
                (GoToImplementation, &[]),
                (FindReferences, &["shift+f12"]),
                (MoveLineUp, &["ctrl+shift+up"]),
                (MoveLineDown, &["ctrl+shift+down"]),
                (DuplicateLineUp, &[]),
                (DuplicateLineDown, &["ctrl+shift+d"]),
                (ExpandSelection, &["ctrl+shift+a"]),
                (SelectAllOccurrences, &["alt+f3"]),
                (Outline, &["ctrl+r"]),
                (ProjectSymbols, &["ctrl+shift+r"]),
            ],
        }
    }
}

/// Pandemonium's own bindings.
fn own() -> &'static [(Action, &'static [&'static str])] {
    use Action::*;
    &[
        (CommandPalette, &["ctrl+shift+p", "f1"]),
        (OpenSettings, &["ctrl+,"]),
        (GoToSession, &["ctrl+r"]),
        (NewSession, &["ctrl+n"]),
        (AddProject, &[]),
        (ShowHistory, &[]),
        (ShowExtensions, &["ctrl+shift+x"]),
        (WorkspaceBack, &["alt+left"]),
        (WorkspaceForward, &["alt+right"]),
        (Quit, &["ctrl+q"]),
        (ShowChat, &[]),
        (ShowDiff, &[]),
        (StopAgent, &[]),
        (ToggleTerminal, &["ctrl+`"]),
        (NewTerminal, &["ctrl+shift+`"]),
        (ToggleSourceControl, &["ctrl+shift+g"]),
        (Commit, &[]),
        (CommitAndPush, &[]),
        (Pull, &[]),
        (Push, &[]),
        (Fetch, &[]),
        (StashChanges, &[]),
        (CheckoutBranch, &[]),
        (CloneRepository, &[]),
        (RefreshSourceControl, &[]),
        (PullRebase, &[]),
        (ForcePush, &[]),
        (Sync, &[]),
        (StageAll, &[]),
        (UnstageAll, &[]),
        (CommitAmend, &[]),
        (ContinueOperation, &[]),
        (SkipCommit, &[]),
        (AbortOperation, &[]),
        (StashApply, &[]),
        (StashPop, &[]),
        (StashDrop, &[]),
        (CreateBranch, &[]),
        (ToggleGraph, &[]),
        (ChangeModel, &[]),
        (ChangeMode, &[]),
        (ChangeAgentSetting, &[]),
        (CycleMode, &[]),
        (AllowRequest, &[]),
        (AlwaysAllowRequest, &[]),
        (RejectRequest, &[]),
        (RenameSession, &[]),
        (ArchiveSession, &[]),
        (CopySessionPath, &[]),
        (CopyBranchName, &[]),
        (OpenSessionFolder, &[]),
        (RewindConversation, &[]),
        (ForkConversation, &[]),
        (RestoreFiles, &[]),
        (FocusPrompt, &[]),
        (CloseTab, &["ctrl+w"]),
        (CloseOtherTabs, &[]),
        (CloseSavedTabs, &[]),
        (CloseAllTabs, &["ctrl+k ctrl+w"]),
        (PinTab, &[]),
        (ToggleReadOnly, &[]),
        (CopyPath, &[]),
        (CopyRelativePath, &[]),
        (RevealInProject, &[]),
        (SplitRight, &["ctrl+\\"]),
        (SplitLeft, &[]),
        (SplitUp, &[]),
        (SplitDown, &[]),
        (FocusNextPane, &["ctrl+k ctrl+right"]),
        (FocusPreviousPane, &["ctrl+k ctrl+left"]),
        (ChangeTheme, &["ctrl+k ctrl+t"]),
        (ToggleTheme, &[]),
        (ChangeKeymap, &[]),
        (ZoomIn, &["ctrl+="]),
        (ZoomOut, &[]),
        (ResetZoom, &["ctrl+0"]),
        (CopyPermalink, &[]),
        (OpenPermalink, &[]),
        (FileHistory, &[]),
        (OpenTerminalHere, &[]),
        (OutgoingCalls, &[]),
        (ToggleBreakpoint, &["f9"]),
        (StartDebugging, &["f5"]),
        (AttachDebugger, &[]),
        (StopDebugging, &["shift+f5"]),
        (PauseDebugging, &["f6"]),
        (StepOver, &["f10"]),
        (StepInto, &["f11"]),
        (StepOut, &["shift+f11"]),
        (CopyWithoutIndentation, &[]),
        (Undo, &[]),
        (Redo, &[]),
        (GoToDefinition, &["f12"]),
        (GoToDeclaration, &[]),
        (GoToTypeDefinition, &["ctrl+f12"]),
        (GoToImplementation, &["shift+f12"]),
        (FindReferences, &["alt+shift+f12"]),
        (IncomingCalls, &["alt+shift+h"]),
        (Rename, &["f2"]),
        (Format, &["ctrl+shift+i", "alt+shift+f"]),
        (CodeActions, &["ctrl+."]),
        (Hover, &["ctrl+k ctrl+i"]),
        (Complete, &["ctrl+space"]),
        (SignatureHelp, &["ctrl+shift+space"]),
        (GoBack, &["ctrl+-"]),
        (GoForward, &["ctrl+shift+-"]),
        (ToggleComment, &["ctrl+/"]),
        (Indent, &["ctrl+]"]),
        (Outdent, &["ctrl+["]),
        (MoveLineUp, &["alt+up"]),
        (MoveLineDown, &["alt+down"]),
        (DuplicateLineUp, &["alt+shift+up"]),
        (DuplicateLineDown, &["alt+shift+down"]),
        (DeleteLine, &["ctrl+shift+k"]),
        (ExpandSelection, &["alt+shift+right"]),
        (ShrinkSelection, &["alt+shift+left"]),
        (NextProblem, &["f8"]),
        (PreviousProblem, &["shift+f8"]),
        (AddNextOccurrence, &["ctrl+d"]),
        (SelectAllOccurrences, &["ctrl+shift+l"]),
        (AddCaretAbove, &["ctrl+alt+up"]),
        (AddCaretBelow, &["ctrl+alt+down"]),
        (SelectLine, &["ctrl+l"]),
        (JoinLines, &["ctrl+j"]),
        (MatchingBracket, &["ctrl+m", "ctrl+shift+\\"]),
        (NewlineBelow, &["ctrl+enter"]),
        (NewlineAbove, &["ctrl+shift+enter"]),
        (Fold, &["ctrl+shift+["]),
        (Unfold, &["ctrl+shift+]"]),
        (Find, &["ctrl+f"]),
        (Replace, &["ctrl+h"]),
        (FindNext, &["f3"]),
        (FindPrevious, &["shift+f3"]),
        (GoToLine, &["ctrl+g"]),
        (Outline, &["ctrl+shift+o"]),
        (ProjectSymbols, &["ctrl+t"]),
        (FileFinder, &["ctrl+p"]),
        (SearchProject, &["ctrl+shift+f"]),
        (Problems, &["ctrl+shift+m"]),
        (RevealInFileManager, &["ctrl+k r"]),
        (Save, &["ctrl+s"]),
        (TerminalAddToChat, &[]),
        (TerminalCopy, &["ctrl+shift+c"]),
        (TerminalPaste, &["ctrl+shift+v"]),
        (TerminalSelectAll, &["ctrl+shift+a"]),
        (TerminalClear, &[]),
        (KillTerminal, &[]),
    ]
}

/// The key a press of Shift and the key that types `shifted` makes, on a keyboard laid out in US English, written as the key types without Shift; any other key as it is.
fn unshifted(shifted: char) -> char {
    let pairs = [
        ('{', '['),
        ('}', ']'),
        ('|', '\\'),
        ('_', '-'),
        ('+', '='),
        (':', ';'),
        ('"', '\''),
        ('<', ','),
        ('>', '.'),
        ('?', '/'),
        ('~', '`'),
        ('!', '1'),
        ('@', '2'),
        ('#', '3'),
        ('$', '4'),
        ('%', '5'),
        ('^', '6'),
        ('&', '7'),
        ('*', '8'),
        ('(', '9'),
        (')', '0'),
    ];
    pairs
        .iter()
        .find(|(held, _)| *held == shifted)
        .map_or(shifted.to_ascii_lowercase(), |(_, plain)| *plain)
}

/// One press of keys: the modifiers held and the key.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Chord {
    /// Control, or Command.
    pub control: bool,
    /// Alt, or Option.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// The key, as its name is written: a letter, digit or sign as the key types it without Shift, or `enter`, `f12`, `up` and the like.
    pub key: String,
}

impl Chord {
    /// The chord of the key named `key`, typing `typed` when it is one that types, pressed with the modifiers given.
    pub fn new(key: &str, control: bool, alt: bool, shift: bool) -> Self {
        let mut characters = key.chars();
        let key = match (characters.next(), characters.next()) {
            (Some(single), None) => unshifted(single).to_string(),
            _ => key.to_ascii_lowercase(),
        };
        Self {
            control,
            alt,
            shift,
            key,
        }
    }

    /// The chord written as `text`, such as `ctrl+shift+k`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase();
        let (modifiers, key) = match text.rsplit_once('+') {
            Some((modifiers, "")) => (modifiers.strip_suffix('+').unwrap_or(modifiers), "+"),
            Some((modifiers, key)) => (modifiers, key),
            None => ("", text.as_str()),
        };
        if key.is_empty() {
            return None;
        }
        let held = |name: &str| modifiers.split('+').any(|part| part == name);
        Some(Self::new(
            key,
            held("ctrl") || held("cmd"),
            held("alt"),
            held("shift"),
        ))
    }

    /// The chord as it is written in the settings, such as `ctrl+shift+k`.
    pub fn written(&self) -> String {
        let mut parts = Vec::new();
        if self.control {
            parts.push("ctrl".to_owned());
        }
        if self.alt {
            parts.push("alt".to_owned());
        }
        if self.shift {
            parts.push("shift".to_owned());
        }
        parts.push(self.key.clone());
        parts.join("+")
    }

    /// The chord as it is shown, such as `Ctrl+Shift+K`.
    pub fn shown(&self) -> String {
        let mut parts = Vec::new();
        if self.control {
            parts.push("Ctrl".to_owned());
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        let key = match self.key.as_str() {
            "up" => "Up".to_owned(),
            "down" => "Down".to_owned(),
            "left" => "Left".to_owned(),
            "right" => "Right".to_owned(),
            "enter" => "Enter".to_owned(),
            "space" => "Space".to_owned(),
            "tab" => "Tab".to_owned(),
            "escape" => "Escape".to_owned(),
            "backspace" => "Backspace".to_owned(),
            "delete" => "Delete".to_owned(),
            "home" => "Home".to_owned(),
            "end" => "End".to_owned(),
            "pageup" => "Page Up".to_owned(),
            "pagedown" => "Page Down".to_owned(),
            key if key.len() > 1 && key.starts_with('f') => key.to_ascii_uppercase(),
            key => key.to_ascii_uppercase(),
        };
        parts.push(key);
        parts.join("+")
    }
}

/// A binding as it is shown, its chords one after another, such as `Ctrl+K Ctrl+I`.
pub fn shown(binding: &str) -> String {
    binding
        .split_whitespace()
        .filter_map(Chord::parse)
        .map(|chord| chord.shown())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What pressing a chord after the ones pressed before it comes to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pressed {
    /// The action the chords are bound to.
    Action(Action),
    /// The start of a binding of more chords; the next press finishes it.
    Waiting,
    /// No binding.
    Nothing,
}

/// The keys bound to each action: a preset with the reader's own bindings over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keymap {
    /// Each binding, as its chords, with the action it is bound to.
    bindings: Vec<(Vec<Chord>, Action)>,
}

impl Keymap {
    /// The keymap of `preset` with `own`, the reader's own bindings by the name each action is kept under, over it.
    pub fn new(preset: Preset, own: &BTreeMap<String, Vec<String>>) -> Self {
        let mut by_action: BTreeMap<Action, Vec<String>> = BTreeMap::new();
        for (action, keys) in self::own().iter().chain(preset.changes()) {
            by_action.insert(*action, keys.iter().map(|key| (*key).to_owned()).collect());
        }
        for (name, keys) in own {
            if let Some(action) = Action::of_key(name) {
                by_action.insert(action, keys.clone());
            }
        }
        let bindings = by_action
            .into_iter()
            .flat_map(|(action, keys)| {
                keys.into_iter().filter_map(move |key| {
                    let chords: Option<Vec<Chord>> =
                        key.split_whitespace().map(Chord::parse).collect();
                    chords
                        .filter(|chords| !chords.is_empty())
                        .map(|chords| (chords, action))
                })
            })
            .collect();
        Self { bindings }
    }

    /// The bindings of `action`, as they are written.
    pub fn keys_of(&self, action: Action) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, held)| *held == action)
            .map(|(chords, _)| {
                chords
                    .iter()
                    .map(Chord::written)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    /// The first binding of `action` as it is shown, when it has one.
    pub fn label(&self, action: Action) -> Option<String> {
        self.keys_of(action).first().map(|binding| shown(binding))
    }

    /// What pressing `chord` after `before` comes to.
    pub fn press(&self, before: &[Chord], chord: &Chord) -> Pressed {
        self.press_among(before, chord, |_| true)
    }

    /// What pressing `chord` after `before` comes to among the bindings of the actions `among` lets through.
    pub fn press_among(
        &self,
        before: &[Chord],
        chord: &Chord,
        among: impl Fn(Action) -> bool,
    ) -> Pressed {
        let mut sequence = before.to_vec();
        sequence.push(chord.clone());
        let mut bindings = self.bindings.iter().filter(|(_, action)| among(*action));
        if let Some((_, action)) = bindings.clone().find(|(chords, _)| *chords == sequence) {
            return Pressed::Action(*action);
        }
        let waiting = bindings
            .any(|(chords, _)| chords.len() > sequence.len() && chords.starts_with(&sequence));
        if waiting {
            Pressed::Waiting
        } else {
            Pressed::Nothing
        }
    }

    /// The actions other than `action` bound to `binding` too that work where it does, in a shell or out of one.
    pub fn clashes(&self, action: Action, binding: &str) -> Vec<Action> {
        let chords: Vec<Chord> = binding
            .split_whitespace()
            .filter_map(Chord::parse)
            .collect();
        self.bindings
            .iter()
            .filter(|(held, other)| {
                *held == chords && *other != action && other.in_terminal() == action.in_terminal()
            })
            .map(|(_, other)| *other)
            .collect()
    }
}
