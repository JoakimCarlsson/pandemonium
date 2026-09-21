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
    ("primary+s", Action::Save, "pane.kind == file"),
    ("primary+alt+s", Action::SaveAll, "true"),
    ("primary+,", Action::OpenSettings, "true"),
    ("escape", Action::Cancel, "true"),
];

/// What VS Code does differently.
pub const VS_CODE: &[Row] = &[
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
    ("primary+shift+a", Action::ShowCommands, "true"),
    ("primary+shift+n", Action::ShowFiles, "true"),
    ("primary+alt+shift+n", Action::NewSession, "project.focused"),
    ("primary+s", Action::SaveAll, "true"),
    ("primary+f4", Action::ClosePane, "true"),
    ("alt+right", Action::NextTab, "true"),
    ("alt+left", Action::PreviousTab, "true"),
    ("alt+f12", Action::NewTerminal, "true"),
];

/// What Vim does differently: the window commands under `ctrl+w`.
pub const VIM: &[Row] = &[
    ("ctrl+w v", Action::SplitRight, "true"),
    ("ctrl+w s", Action::SplitDown, "true"),
    ("ctrl+w c", Action::ClosePane, "true"),
    ("ctrl+w h", Action::FocusLeft, "true"),
    ("ctrl+w l", Action::FocusRight, "true"),
    ("ctrl+w k", Action::FocusUp, "true"),
    ("ctrl+w j", Action::FocusDown, "true"),
    ("ctrl+w t", Action::NewTerminal, "true"),
    ("g t", Action::NextTab, "pane.kind != input"),
    ("g shift+t", Action::PreviousTab, "pane.kind != input"),
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
    ("primary+alt+2", Action::SplitRight, "true"),
    ("primary+alt+shift+2", Action::SplitDown, "true"),
    ("ctrl+tab", Action::NextTab, "true"),
    ("ctrl+shift+tab", Action::PreviousTab, "true"),
    ("primary+k primary+b", Action::ShowProjects, "true"),
];
