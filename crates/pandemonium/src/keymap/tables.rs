//! The keymap tables the editor ships with.
//!
//! [`BASE`] is the whole vocabulary: every action that has a chord at all has
//! one here. Each of the other tables is an overlay — only what that editor
//! does differently — laid over [`BASE`] by [`BaseKeymap::keymap`], so a
//! binding lives in one place and the tables stay readable next to each other.
//!
//! `primary` is command on macOS and control everywhere else.
//!
//! [`BaseKeymap::keymap`]: crate::keymap::base::BaseKeymap::keymap

use crate::keymap::action::Action;
use crate::keymap::binding::Row;

/// The bindings every keymap starts from.
pub const BASE: &[Row] = &[
    ("primary+shift+p", Action::ShowCommands, "true"),
    ("primary+p", Action::ShowFiles, "true"),
    ("primary+shift+o", Action::ShowProjects, "true"),
    ("primary+shift+j", Action::ShowSessions, "true"),
    ("primary+k primary+o", Action::AddProject, "true"),
    (
        "primary+k primary+shift+o",
        Action::RemoveProject,
        "project.focused",
    ),
    ("primary+shift+n", Action::NewSession, "project.focused"),
    ("primary+shift+d", Action::ReviewSession, "session.focused"),
    ("primary+shift+enter", Action::FocusAgent, "session.focused"),
    (
        "primary+shift+m",
        Action::ChangeAgentMode,
        "pane.kind == agent || pane.kind == prompt",
    ),
    (
        "shift+tab",
        Action::CycleAgentMode,
        "pane.kind == agent || pane.kind == prompt",
    ),
    (
        "primary+alt+/",
        Action::ChangeAgentModel,
        "pane.kind == agent || pane.kind == prompt",
    ),
    (
        "primary+k primary+shift+e",
        Action::EndSession,
        "session.focused",
    ),
    ("primary+\\", Action::SplitRight, "true"),
    ("primary+shift+\\", Action::SplitDown, "true"),
    ("primary+w", Action::ClosePane, "true"),
    ("primary+alt+right", Action::NextTab, "true"),
    ("primary+alt+left", Action::PreviousTab, "true"),
    ("primary+k left", Action::FocusLeft, "true"),
    ("primary+k right", Action::FocusRight, "true"),
    ("primary+k up", Action::FocusUp, "true"),
    ("primary+k down", Action::FocusDown, "true"),
    ("primary+`", Action::NewTerminal, "true"),
    ("primary+shift+g", Action::ShowChanges, "true"),
    ("primary+shift+e", Action::FocusFiles, "true"),
    ("primary+k primary+g", Action::OpenReview, "project.focused"),
    (
        "primary+k primary+e",
        Action::EditChanges,
        "project.focused",
    ),
    (
        "primary+k v",
        Action::OpenMarkdownPreview,
        "pane.kind == file",
    ),
    ("primary+s", Action::Save, "pane.kind == file"),
    ("primary+alt+s", Action::SaveAll, "true"),
    ("primary+,", Action::OpenSettings, "true"),
    ("escape", Action::Cancel, "true"),
    ("primary+shift+t", Action::ReopenTab, "true"),
    (
        "primary+z",
        Action::Undo,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit",
    ),
    (
        "primary+shift+z",
        Action::Redo,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit",
    ),
    (
        "primary+y",
        Action::Redo,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit",
    ),
    (
        "primary+x",
        Action::Cut,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit",
    ),
    (
        "primary+c",
        Action::Copy,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit || pane.kind == terminal",
    ),
    (
        "primary+v",
        Action::Paste,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit || pane.kind == terminal",
    ),
    (
        "primary+a",
        Action::SelectAll,
        "pane.kind == file || pane.kind == prompt || pane.kind == commit || pane.kind == terminal",
    ),
    ("primary+l", Action::SelectLine, "pane.kind == file"),
    (
        "shift+alt+right",
        Action::ExpandSelection,
        "pane.kind == file",
    ),
    ("shift+alt+down", Action::DuplicateLine, "pane.kind == file"),
    ("primary+shift+k", Action::DeleteLine, "pane.kind == file"),
    ("alt+up", Action::MoveLineUp, "pane.kind == file"),
    ("alt+down", Action::MoveLineDown, "pane.kind == file"),
    (
        "primary+shift+alt+j",
        Action::JoinLines,
        "pane.kind == file",
    ),
    (
        "primary+enter",
        Action::InsertLineBelow,
        "pane.kind == file",
    ),
    (
        "primary+shift+enter",
        Action::InsertLineAbove,
        "pane.kind == file",
    ),
    ("primary+/", Action::ToggleComment, "pane.kind == file"),
    ("primary+]", Action::Indent, "pane.kind == file"),
    ("primary+[", Action::Outdent, "pane.kind == file"),
    ("primary+f", Action::Find, "pane.kind == file"),
    ("primary+h", Action::Replace, "pane.kind == file"),
    ("f3", Action::FindNext, "pane.kind == file"),
    ("shift+f3", Action::FindPrevious, "pane.kind == file"),
    ("primary+f3", Action::FindSelection, "pane.kind == file"),
    ("primary+shift+f", Action::SearchProject, "true"),
    ("primary+g", Action::GoToLine, "pane.kind == file"),
    ("primary+r", Action::ShowSymbols, "pane.kind == file"),
    (
        "primary+t",
        Action::ShowWorkspaceSymbols,
        "pane.kind == file",
    ),
    ("primary+shift+m", Action::ShowProblems, "true"),
    ("f12", Action::GoToDefinition, "pane.kind == file"),
    (
        "primary+f12",
        Action::GoToImplementation,
        "pane.kind == file",
    ),
    ("shift+f12", Action::FindReferences, "pane.kind == file"),
    (
        "shift+alt+h",
        Action::ShowIncomingCalls,
        "pane.kind == file",
    ),
    ("alt+left", Action::GoBack, "true"),
    ("alt+right", Action::GoForward, "true"),
    ("f8", Action::NextDiagnostic, "pane.kind == file"),
    ("shift+f8", Action::PreviousDiagnostic, "pane.kind == file"),
    (
        "primary+k primary+i",
        Action::ShowHover,
        "pane.kind == file",
    ),
    (
        "primary+space",
        Action::ShowCompletions,
        "pane.kind == file",
    ),
    (
        "primary+shift+space",
        Action::ShowSignature,
        "pane.kind == file",
    ),
    ("primary+.", Action::ShowCodeActions, "pane.kind == file"),
    ("f2", Action::Rename, "pane.kind == file"),
    ("shift+alt+f", Action::Format, "pane.kind == file"),
    ("primary+alt+b", Action::ToggleBlame, "pane.kind == file"),
    ("primary+alt+n", Action::NextChange, "pane.kind == file"),
    ("primary+alt+p", Action::PreviousChange, "pane.kind == file"),
    (
        "primary+alt+up",
        Action::AddCursorAbove,
        "pane.kind == file",
    ),
    (
        "primary+alt+down",
        Action::AddCursorBelow,
        "pane.kind == file",
    ),
    ("primary+d", Action::AddNextMatch, "pane.kind == file"),
    (
        "primary+shift+l",
        Action::SelectAllMatches,
        "pane.kind == file",
    ),
    (
        "primary+k primary+l",
        Action::ToggleFold,
        "pane.kind == file",
    ),
    ("primary+k primary+0", Action::FoldAll, "pane.kind == file"),
    (
        "primary+k primary+j",
        Action::UnfoldAll,
        "pane.kind == file",
    ),
    ("primary+=", Action::ZoomIn, "true"),
    ("primary+-", Action::ZoomOut, "true"),
    ("primary+0", Action::ZoomReset, "true"),
];

/// What VS Code does differently.
pub const VS_CODE: &[Row] = &[
    ("primary+shift+o", Action::ShowSymbols, "pane.kind == file"),
    ("primary+k primary+x", Action::Format, "pane.kind == file"),
    ("primary+k primary+left", Action::FocusLeft, "true"),
    ("primary+k primary+right", Action::FocusRight, "true"),
    ("primary+k primary+up", Action::FocusUp, "true"),
    ("primary+k primary+down", Action::FocusDown, "true"),
    ("primary+k s", Action::SaveAll, "true"),
    ("ctrl+pagedown", Action::NextTab, "true"),
    ("ctrl+pageup", Action::PreviousTab, "true"),
    ("ctrl+`", Action::NewTerminal, "true"),
];

/// What Zed does differently.
pub const ZED: &[Row] = &[
    ("primary+shift+h", Action::Replace, "pane.kind == file"),
    ("primary+shift+o", Action::ShowSymbols, "pane.kind == file"),
    (
        "primary+k primary+t",
        Action::GoToTypeDefinition,
        "pane.kind == file",
    ),
    ("primary+k right", Action::SplitRight, "true"),
    ("primary+k down", Action::SplitDown, "true"),
    ("primary+k primary+left", Action::FocusLeft, "true"),
    ("primary+k primary+right", Action::FocusRight, "true"),
    ("primary+k primary+up", Action::FocusUp, "true"),
    ("primary+k primary+down", Action::FocusDown, "true"),
    ("primary+alt+s", Action::SaveAll, "true"),
    ("ctrl+`", Action::NewTerminal, "true"),
];

/// What JetBrains IDEs do differently.
pub const JETBRAINS: &[Row] = &[
    ("primary+shift+j", Action::JoinLines, "pane.kind == file"),
    ("primary+alt+l", Action::Format, "pane.kind == file"),
    ("primary+b", Action::GoToDefinition, "pane.kind == file"),
    (
        "primary+alt+f7",
        Action::FindReferences,
        "pane.kind == file",
    ),
    ("shift+f6", Action::Rename, "pane.kind == file"),
    ("primary+shift+a", Action::ShowCommands, "true"),
    ("primary+shift+n", Action::ShowFiles, "true"),
    ("primary+alt+shift+n", Action::NewSession, "project.focused"),
    ("primary+s", Action::SaveAll, "true"),
    ("primary+f4", Action::ClosePane, "true"),
    ("alt+right", Action::NextTab, "true"),
    ("alt+left", Action::PreviousTab, "true"),
    ("alt+f12", Action::NewTerminal, "true"),
];

/// What Emacs does differently: the window commands under `ctrl+x`.
pub const EMACS: &[Row] = &[
    ("alt+x", Action::ShowCommands, "true"),
    ("ctrl+x ctrl+f", Action::ShowFiles, "true"),
    ("ctrl+x ctrl+s", Action::Save, "pane.kind == file"),
    ("ctrl+x s", Action::SaveAll, "true"),
    ("ctrl+x 3", Action::SplitRight, "true"),
    ("ctrl+x 2", Action::SplitDown, "true"),
    ("ctrl+x 0", Action::ClosePane, "true"),
    ("ctrl+x o", Action::FocusRight, "true"),
    ("ctrl+g", Action::Cancel, "true"),
];

/// What Helix does differently: the space menu and the `ctrl+w` windows.
pub const HELIX: &[Row] = &[
    ("space f", Action::ShowFiles, "true"),
    ("space ?", Action::ShowCommands, "true"),
    ("space b", Action::ShowSessions, "true"),
    ("ctrl+w v", Action::SplitRight, "true"),
    ("ctrl+w s", Action::SplitDown, "true"),
    ("ctrl+w q", Action::ClosePane, "true"),
    ("ctrl+w h", Action::FocusLeft, "true"),
    ("ctrl+w l", Action::FocusRight, "true"),
    ("ctrl+w k", Action::FocusUp, "true"),
    ("ctrl+w j", Action::FocusDown, "true"),
];

/// What Sublime Text does differently.
pub const SUBLIME: &[Row] = &[
    (
        "primary+shift+d",
        Action::DuplicateLine,
        "pane.kind == file",
    ),
    ("primary+j", Action::JoinLines, "pane.kind == file"),
    ("primary+alt+2", Action::SplitRight, "true"),
    ("primary+alt+shift+2", Action::SplitDown, "true"),
    ("ctrl+tab", Action::NextTab, "true"),
    ("ctrl+shift+tab", Action::PreviousTab, "true"),
    ("primary+k primary+b", Action::ShowProjects, "true"),
];
