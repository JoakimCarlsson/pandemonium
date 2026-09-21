//! What a binding does: the named actions the window can carry out.
//!
//! An action is a name, not a closure. A keymap, a palette entry and a menu
//! item all resolve to the same [`Action`], and the window is the one place
//! that carries one out.

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
    /// Open the settings screen.
    OpenSettings,
    /// Dismiss whatever is open on top: a palette, a prompt, a search.
    Cancel,
}

impl Action {
    /// Every action, in the order they are written above.
    pub const ALL: [Self; 24] = [
        Self::ShowCommands,
        Self::ShowFiles,
        Self::ShowProjects,
        Self::ShowSessions,
        Self::AddProject,
        Self::RemoveProject,
        Self::NewSession,
        Self::ReviewSession,
        Self::FocusAgent,
        Self::EndSession,
        Self::SplitRight,
        Self::SplitDown,
        Self::ClosePane,
        Self::NextTab,
        Self::PreviousTab,
        Self::FocusLeft,
        Self::FocusRight,
        Self::FocusUp,
        Self::FocusDown,
        Self::NewTerminal,
        Self::Save,
        Self::SaveAll,
        Self::OpenSettings,
        Self::Cancel,
    ];

    /// The name a keymap binds the action by.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ShowCommands => "palette.commands",
            Self::ShowFiles => "palette.files",
            Self::ShowProjects => "palette.projects",
            Self::ShowSessions => "palette.sessions",
            Self::AddProject => "project.add",
            Self::RemoveProject => "project.remove",
            Self::NewSession => "session.new",
            Self::ReviewSession => "session.review",
            Self::FocusAgent => "session.agent",
            Self::EndSession => "session.end",
            Self::SplitRight => "pane.split_right",
            Self::SplitDown => "pane.split_down",
            Self::ClosePane => "pane.close",
            Self::NextTab => "pane.next_tab",
            Self::PreviousTab => "pane.previous_tab",
            Self::FocusLeft => "pane.focus_left",
            Self::FocusRight => "pane.focus_right",
            Self::FocusUp => "pane.focus_up",
            Self::FocusDown => "pane.focus_down",
            Self::NewTerminal => "terminal.new",
            Self::Save => "file.save",
            Self::SaveAll => "file.save_all",
            Self::OpenSettings => "window.settings",
            Self::Cancel => "window.cancel",
        }
    }

    /// The action's title, as the palette and the keymap screen show it.
    pub const fn title(self) -> &'static str {
        match self {
            Self::ShowCommands => "Show Commands",
            Self::ShowFiles => "Go to File",
            Self::ShowProjects => "Go to Project",
            Self::ShowSessions => "Go to Session",
            Self::AddProject => "Add Project",
            Self::RemoveProject => "Remove Project",
            Self::NewSession => "New Session",
            Self::ReviewSession => "Review Session",
            Self::FocusAgent => "Focus Agent",
            Self::EndSession => "End Session",
            Self::SplitRight => "Split Right",
            Self::SplitDown => "Split Down",
            Self::ClosePane => "Close Tab",
            Self::NextTab => "Next Tab",
            Self::PreviousTab => "Previous Tab",
            Self::FocusLeft => "Focus Pane Left",
            Self::FocusRight => "Focus Pane Right",
            Self::FocusUp => "Focus Pane Up",
            Self::FocusDown => "Focus Pane Down",
            Self::NewTerminal => "New Terminal",
            Self::Save => "Save",
            Self::SaveAll => "Save All",
            Self::OpenSettings => "Open Settings",
            Self::Cancel => "Cancel",
        }
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
        Self::ALL
            .into_iter()
            .find(|action| action.id() == id)
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
