//! What a binding does: the named actions the window can carry out.
//!
//! An action is a name, not a closure. A keymap, a palette entry and a menu
//! item all resolve to the same [`Action`], and the window is the one place
//! that carries one out. The catalogue below is the whole vocabulary: an
//! action's name, the id a keymap binds it by and the title a palette shows
//! are written down once, together, so there is no way for the three to
//! drift apart.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// Something the window can be asked to do.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Action {
    /// Open the command palette.
    ShowCommands,
    /// Open the file palette, over every open project.
    ShowFiles,
    /// Open the project palette.
    ShowProjects,
    /// Open the session palette.
    ShowSessions,
    /// Open the palette of symbols in the focused file.
    ShowSymbols,
    /// Open the palette of symbols across the focused file's workspace.
    ShowWorkspaceSymbols,
    /// Open the list of every error and warning in the open files.
    ShowProblems,
    /// Add a repository to the window as a project.
    AddProject,
    /// Take the focused project out of the window.
    RemoveProject,
    /// Start a session on the focused project, in a worktree of its own.
    NewSession,
    /// Open the focused session's diff against the project.
    ReviewSession,
    /// Open the focused session's agent.
    FocusAgent,
    /// End the focused session and tear its worktree down.
    EndSession,
    /// Split the focused pane to its right.
    SplitRight,
    /// Split the focused pane below it.
    SplitDown,
    /// Close the focused pane's current tab.
    ClosePane,
    /// Open again the tab that was closed last.
    ReopenTab,
    /// Move to the next tab in the focused pane.
    NextTab,
    /// Move to the previous tab in the focused pane.
    PreviousTab,
    /// Move focus to the pane on the left.
    FocusLeft,
    /// Move focus to the pane on the right.
    FocusRight,
    /// Move focus to the pane above.
    FocusUp,
    /// Move focus to the pane below.
    FocusDown,
    /// Open a terminal in a new pane.
    NewTerminal,
    /// Write the focused buffer to disk.
    Save,
    /// Write every changed buffer to disk.
    SaveAll,
    /// Take back the last change to the focused buffer.
    Undo,
    /// Put back the change that was taken back last.
    Redo,
    /// Put the selection on the clipboard and take it out of the buffer.
    Cut,
    /// Put the selection on the clipboard.
    Copy,
    /// Put what is on the clipboard into the buffer.
    Paste,
    /// Select everything the focused buffer holds.
    SelectAll,
    /// Select the whole of the line the cursor is on.
    SelectLine,
    /// Grow the selection to the word under the cursor, then to its line.
    ExpandSelection,
    /// Put another cursor on the line above the topmost one.
    AddCursorAbove,
    /// Put another cursor on the line below the lowest one.
    AddCursorBelow,
    /// Select the word under the cursor, then the next place it appears.
    AddNextMatch,
    /// Put a cursor at every place the selected text appears.
    SelectAllMatches,
    /// Leave one cursor where the primary one is.
    CollapseCursors,
    /// Fold what the line the cursor is on holds, or unfold it.
    ToggleFold,
    /// Fold everything that holds something.
    FoldAll,
    /// Unfold everything.
    UnfoldAll,
    /// Put another copy of the selected lines below them.
    DuplicateLine,
    /// Take the selected lines out.
    DeleteLine,
    /// Move the selected lines one line up.
    MoveLineUp,
    /// Move the selected lines one line down.
    MoveLineDown,
    /// Join the line below the cursor onto the line it is on.
    JoinLines,
    /// Put an empty line below the one the cursor is on, and go to it.
    InsertLineBelow,
    /// Put an empty line above the one the cursor is on, and go to it.
    InsertLineAbove,
    /// Comment the selected lines, or take their comments off.
    ToggleComment,
    /// Indent the selected lines by one step.
    Indent,
    /// Take one step of indentation off the selected lines.
    Outdent,
    /// Open the search bar over the focused pane.
    Find,
    /// Open the search bar with its replacement field showing.
    Replace,
    /// Go to the next match of what is being looked for.
    FindNext,
    /// Go to the previous match of what is being looked for.
    FindPrevious,
    /// Look for what is selected.
    FindSelection,
    /// Replace the match being looked at.
    ReplaceMatch,
    /// Replace every match at once.
    ReplaceAll,
    /// Search every file of every open project.
    SearchProject,
    /// Go to a line of the focused file by number.
    GoToLine,
    /// Go to where the symbol under the cursor is defined.
    GoToDefinition,
    /// Go to where the type of the symbol under the cursor is defined.
    GoToTypeDefinition,
    /// Go to what implements the symbol under the cursor.
    GoToImplementation,
    /// Go to where the symbol under the cursor is declared.
    GoToDeclaration,
    /// List everywhere the symbol under the cursor is used.
    FindReferences,
    /// List whatever calls the symbol under the cursor.
    ShowIncomingCalls,
    /// List whatever the symbol under the cursor calls.
    ShowOutgoingCalls,
    /// Go back to where the cursor was before the last jump.
    GoBack,
    /// Go forward again to where the cursor was before going back.
    GoForward,
    /// Go to the next error or warning in the focused file.
    NextDiagnostic,
    /// Go to the previous error or warning in the focused file.
    PreviousDiagnostic,
    /// Show what the language server says about what is under the cursor.
    ShowHover,
    /// Offer the completions the language server has for here.
    ShowCompletions,
    /// Show the signature of the call the cursor is inside.
    ShowSignature,
    /// Offer the fixes the language server has for here.
    ShowCodeActions,
    /// Rename the symbol under the cursor everywhere it appears.
    Rename,
    /// Lay the focused buffer out the way its formatter would.
    Format,
    /// Show who last changed each line of the focused file.
    ToggleBlame,
    /// Go to the next place the focused file differs from the index.
    NextChange,
    /// Go to the previous place the focused file differs from the index.
    PreviousChange,
    /// Put the change under the cursor back the way the index has it.
    RevertChange,
    /// Show what the active project has changed, in the sidebar.
    ShowChanges,
    /// Open those changes for review, in a pane.
    OpenReview,
    /// Start an agent in the active project's worktree, in a pane.
    NewAgentSession,
    /// Finish the session in hand, which takes its worktree away.
    FinishSession,
    /// Choose which mode to put the agent in hand into.
    ChangeAgentMode,
    /// Put it into the mode after the one it is in.
    CycleAgentMode,
    /// Choose which model that agent is to talk to.
    ChangeAgentModel,
    /// Put what the list of changes is acting on into the index.
    StageSelectedChanges,
    /// Take what it is acting on back out of the index.
    UnstageSelectedChanges,
    /// Throw away what it is acting on, having asked first.
    DiscardSelectedChanges,
    /// Put everything that has changed into the index.
    StageAllChanges,
    /// Take everything back out of the index.
    UnstageAllChanges,
    /// Commit what the index holds, saying what the message field holds.
    CommitChanges,
    /// Ask git again what it makes of the open worktrees.
    RefreshChanges,
    /// Choose a local branch to check out in the active project.
    SwitchBranch,
    /// Create and check out a local branch in the active project.
    CreateBranch,
    /// Draw the editor's text one step larger.
    ZoomIn,
    /// Draw the editor's text one step smaller.
    ZoomOut,
    /// Draw the editor's text at the size it was set at.
    ZoomReset,
    /// Open the settings screen.
    OpenSettings,
    /// Dismiss whatever is open on top: a palette, a prompt, a search.
    Cancel,
}

/// Every action, the name a keymap binds it by and the title a palette shows.
///
/// The order is the order a palette lists them in, which is why related
/// commands sit together rather than alphabetically.
const CATALOGUE: &[(Action, &str, &str)] = &[
    (Action::ShowCommands, "palette.commands", "Show Commands"),
    (Action::ShowFiles, "palette.files", "Go to File"),
    (Action::ShowProjects, "palette.projects", "Go to Project"),
    (Action::ShowSessions, "palette.sessions", "Go to Session"),
    (Action::ShowSymbols, "palette.symbols", "Go to Symbol"),
    (
        Action::ShowWorkspaceSymbols,
        "palette.workspace_symbols",
        "Go to Symbol in Workspace",
    ),
    (Action::ShowProblems, "palette.problems", "Go to Problem"),
    (Action::AddProject, "project.add", "Add Project"),
    (Action::RemoveProject, "project.remove", "Remove Project"),
    (Action::NewSession, "session.new", "New Session"),
    (Action::ReviewSession, "session.review", "Review Session"),
    (Action::FocusAgent, "session.agent", "Focus Agent"),
    (Action::EndSession, "session.end", "End Session"),
    (Action::SplitRight, "pane.split_right", "Split Right"),
    (Action::SplitDown, "pane.split_down", "Split Down"),
    (Action::ClosePane, "pane.close", "Close Tab"),
    (Action::ReopenTab, "pane.reopen", "Reopen Closed Tab"),
    (Action::NextTab, "pane.next_tab", "Next Tab"),
    (Action::PreviousTab, "pane.previous_tab", "Previous Tab"),
    (Action::FocusLeft, "pane.focus_left", "Focus Pane Left"),
    (Action::FocusRight, "pane.focus_right", "Focus Pane Right"),
    (Action::FocusUp, "pane.focus_up", "Focus Pane Up"),
    (Action::FocusDown, "pane.focus_down", "Focus Pane Down"),
    (Action::NewTerminal, "terminal.new", "New Terminal"),
    (Action::Save, "file.save", "Save"),
    (Action::SaveAll, "file.save_all", "Save All"),
    (Action::Undo, "edit.undo", "Undo"),
    (Action::Redo, "edit.redo", "Redo"),
    (Action::Cut, "edit.cut", "Cut"),
    (Action::Copy, "edit.copy", "Copy"),
    (Action::Paste, "edit.paste", "Paste"),
    (Action::SelectAll, "edit.select_all", "Select All"),
    (Action::SelectLine, "edit.select_line", "Select Line"),
    (
        Action::ExpandSelection,
        "edit.expand_selection",
        "Expand Selection",
    ),
    (
        Action::AddCursorAbove,
        "edit.cursor_above",
        "Add Cursor Above",
    ),
    (
        Action::AddCursorBelow,
        "edit.cursor_below",
        "Add Cursor Below",
    ),
    (
        Action::AddNextMatch,
        "edit.add_next_match",
        "Add Selection to Next Match",
    ),
    (
        Action::SelectAllMatches,
        "edit.select_all_matches",
        "Select All Occurrences",
    ),
    (
        Action::CollapseCursors,
        "edit.collapse_cursors",
        "Collapse Cursors",
    ),
    (Action::ToggleFold, "view.toggle_fold", "Toggle Fold"),
    (Action::FoldAll, "view.fold_all", "Fold All"),
    (Action::UnfoldAll, "view.unfold_all", "Unfold All"),
    (
        Action::DuplicateLine,
        "edit.duplicate_line",
        "Duplicate Line",
    ),
    (Action::DeleteLine, "edit.delete_line", "Delete Line"),
    (Action::MoveLineUp, "edit.move_line_up", "Move Line Up"),
    (
        Action::MoveLineDown,
        "edit.move_line_down",
        "Move Line Down",
    ),
    (Action::JoinLines, "edit.join_lines", "Join Lines"),
    (
        Action::InsertLineBelow,
        "edit.line_below",
        "Insert Line Below",
    ),
    (
        Action::InsertLineAbove,
        "edit.line_above",
        "Insert Line Above",
    ),
    (
        Action::ToggleComment,
        "edit.toggle_comment",
        "Toggle Comment",
    ),
    (Action::Indent, "edit.indent", "Indent"),
    (Action::Outdent, "edit.outdent", "Outdent"),
    (Action::Find, "search.find", "Find"),
    (Action::Replace, "search.replace", "Replace"),
    (Action::FindNext, "search.next", "Find Next"),
    (Action::FindPrevious, "search.previous", "Find Previous"),
    (Action::FindSelection, "search.selection", "Find Selection"),
    (
        Action::ReplaceMatch,
        "search.replace_match",
        "Replace Match",
    ),
    (Action::ReplaceAll, "search.replace_all", "Replace All"),
    (Action::SearchProject, "search.project", "Search Project"),
    (Action::GoToLine, "go.line", "Go to Line"),
    (Action::GoToDefinition, "go.definition", "Go to Definition"),
    (
        Action::GoToTypeDefinition,
        "go.type_definition",
        "Go to Type Definition",
    ),
    (
        Action::GoToImplementation,
        "go.implementation",
        "Go to Implementation",
    ),
    (
        Action::GoToDeclaration,
        "go.declaration",
        "Go to Declaration",
    ),
    (
        Action::FindReferences,
        "go.references",
        "Find All References",
    ),
    (
        Action::ShowIncomingCalls,
        "go.incoming_calls",
        "Show Incoming Calls",
    ),
    (
        Action::ShowOutgoingCalls,
        "go.outgoing_calls",
        "Show Outgoing Calls",
    ),
    (Action::GoBack, "go.back", "Go Back"),
    (Action::GoForward, "go.forward", "Go Forward"),
    (Action::NextDiagnostic, "go.next_problem", "Next Problem"),
    (
        Action::PreviousDiagnostic,
        "go.previous_problem",
        "Previous Problem",
    ),
    (Action::ShowHover, "language.hover", "Show Hover"),
    (
        Action::ShowCompletions,
        "language.completions",
        "Show Completions",
    ),
    (
        Action::ShowSignature,
        "language.signature",
        "Show Signature Help",
    ),
    (
        Action::ShowCodeActions,
        "language.code_actions",
        "Show Code Actions",
    ),
    (Action::Rename, "language.rename", "Rename Symbol"),
    (Action::Format, "language.format", "Format Document"),
    (Action::ToggleBlame, "git.blame", "Toggle Git Blame"),
    (Action::NextChange, "git.next_change", "Next Change"),
    (
        Action::PreviousChange,
        "git.previous_change",
        "Previous Change",
    ),
    (Action::RevertChange, "git.revert_change", "Revert Change"),
    (Action::ShowChanges, "git.changes", "Show Source Control"),
    (Action::OpenReview, "git.review", "Review Changes"),
    (Action::NewAgentSession, "agent.new", "New Agent Session"),
    (Action::FinishSession, "session.finish", "Finish Session"),
    (Action::ChangeAgentMode, "agent.mode", "Change Agent Mode"),
    (
        Action::CycleAgentMode,
        "agent.cycle_mode",
        "Cycle Agent Mode",
    ),
    (Action::StageSelectedChanges, "git.stage", "Stage Changes"),
    (
        Action::UnstageSelectedChanges,
        "git.unstage",
        "Unstage Changes",
    ),
    (
        Action::DiscardSelectedChanges,
        "git.discard",
        "Discard Changes",
    ),
    (
        Action::StageAllChanges,
        "git.stage_all",
        "Stage All Changes",
    ),
    (
        Action::UnstageAllChanges,
        "git.unstage_all",
        "Unstage All Changes",
    ),
    (Action::CommitChanges, "git.commit", "Commit"),
    (Action::RefreshChanges, "git.refresh", "Refresh Changes"),
    (
        Action::SwitchBranch,
        "git.switch_branch",
        "Git: Switch Branch",
    ),
    (
        Action::CreateBranch,
        "git.create_branch",
        "Git: Create Branch",
    ),
    (Action::ZoomIn, "view.zoom_in", "Zoom In"),
    (Action::ZoomOut, "view.zoom_out", "Zoom Out"),
    (Action::ZoomReset, "view.zoom_reset", "Reset Zoom"),
    (Action::OpenSettings, "window.settings", "Open Settings"),
    (Action::Cancel, "window.cancel", "Cancel"),
];

impl Action {
    /// Every action, in the order a palette lists them.
    pub fn all() -> impl Iterator<Item = Self> {
        CATALOGUE.iter().map(|(action, _, _)| *action)
    }

    /// The name a keymap binds the action by.
    pub fn id(self) -> &'static str {
        self.entry().1
    }

    /// The action's title, as the palette and the keymap screen show it.
    pub fn title(self) -> &'static str {
        self.entry().2
    }

    /// Whether the action is one a pane showing a file carries out.
    ///
    /// The palette greys out what does not apply where the keyboard is, and
    /// a file command asked for with a terminal focused would otherwise go
    /// quietly nowhere.
    pub fn needs_buffer(self) -> bool {
        matches!(
            self,
            Self::Save
                | Self::Undo
                | Self::Redo
                | Self::Cut
                | Self::Copy
                | Self::Paste
                | Self::SelectAll
                | Self::SelectLine
                | Self::ExpandSelection
                | Self::AddCursorAbove
                | Self::AddCursorBelow
                | Self::AddNextMatch
                | Self::SelectAllMatches
                | Self::CollapseCursors
                | Self::ToggleFold
                | Self::FoldAll
                | Self::UnfoldAll
                | Self::DuplicateLine
                | Self::DeleteLine
                | Self::MoveLineUp
                | Self::MoveLineDown
                | Self::JoinLines
                | Self::InsertLineBelow
                | Self::InsertLineAbove
                | Self::ToggleComment
                | Self::Indent
                | Self::Outdent
                | Self::Find
                | Self::Replace
                | Self::FindNext
                | Self::FindPrevious
                | Self::FindSelection
                | Self::ReplaceMatch
                | Self::ReplaceAll
                | Self::GoToLine
                | Self::ShowSymbols
                | Self::ShowWorkspaceSymbols
                | Self::GoToDefinition
                | Self::GoToTypeDefinition
                | Self::GoToImplementation
                | Self::GoToDeclaration
                | Self::FindReferences
                | Self::ShowIncomingCalls
                | Self::ShowOutgoingCalls
                | Self::NextDiagnostic
                | Self::PreviousDiagnostic
                | Self::ShowHover
                | Self::ShowCompletions
                | Self::ShowSignature
                | Self::ShowCodeActions
                | Self::Rename
                | Self::Format
                | Self::ToggleBlame
                | Self::NextChange
                | Self::PreviousChange
                | Self::RevertChange
        )
    }

    /// This action's row of the catalogue.
    ///
    /// # Panics
    ///
    /// Panics if the action is not in the catalogue, which is a table
    /// compiled into the binary missing a variant compiled into the binary.
    fn entry(self) -> &'static (Self, &'static str, &'static str) {
        CATALOGUE
            .iter()
            .find(|(action, _, _)| *action == self)
            .expect("every action is in the catalogue")
    }
}

impl Display for Action {
    /// Writes the name a keymap binds the action by.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

impl FromStr for Action {
    type Err = UnknownAction;

    /// Reads an action by the name a keymap binds it by.
    fn from_str(id: &str) -> Result<Self, Self::Err> {
        CATALOGUE
            .iter()
            .find(|(_, name, _)| *name == id)
            .map(|(action, _, _)| *action)
            .ok_or_else(|| UnknownAction { id: id.to_owned() })
    }
}

/// A name that binds to no action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownAction {
    /// The name that bound to nothing.
    pub id: String,
}

impl Display for UnknownAction {
    /// Names the action that could not be found.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown action `{}`", self.id)
    }
}

impl std::error::Error for UnknownAction {}
