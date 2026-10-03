//! Workspace tools hosted as ordinary tabs in the pane tree.
//!
//! A tool supplies its identity, label and icon; the pane tree supplies its
//! placement, tab group, splits, resizing and persistence. Navigators follow
//! the active worktree while keeping their position across project changes.

use pm_ui::IconName;
use serde::{Deserialize, Serialize};

use crate::panel::PanelView;
use crate::panes::SplitDirection;

/// A workspace tool that can be opened in any pane.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    /// Open projects and their sessions.
    Projects,
    /// Files of the active worktree.
    Files,
    /// Source control for the active worktree.
    Changes,
    /// Shells of the active worktree.
    Terminal,
    /// Diagnostics of open files in the active worktree.
    Problems,
    /// Debugger and console of the active worktree.
    Debug,
}

impl Tool {
    /// Registered tools, in the order menus offer them.
    pub const ALL: [Self; 6] = [
        Self::Projects,
        Self::Files,
        Self::Changes,
        Self::Terminal,
        Self::Problems,
        Self::Debug,
    ];

    /// The side used by the default layout preset, expressed as an ordinary split.
    pub const fn default_split(self) -> SplitDirection {
        match self {
            Self::Projects => SplitDirection::Left,
            Self::Files | Self::Changes => SplitDirection::Right,
            Self::Terminal | Self::Problems | Self::Debug => SplitDirection::Down,
        }
    }

    /// Whether resetting the layout reopens this tool when it was closed.
    pub const fn opens_by_default(self) -> bool {
        matches!(self, Self::Projects | Self::Files | Self::Changes)
    }

    /// The name shown on this tool's tab and in menus.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Projects => "Projects",
            Self::Files => "Files",
            Self::Changes => "Changes",
            Self::Terminal => PanelView::Terminal.label(),
            Self::Problems => PanelView::Problems.label(),
            Self::Debug => PanelView::Debug.label(),
        }
    }

    /// The icon shown on this tool's tab.
    pub const fn icon(self) -> IconName {
        match self {
            Self::Projects => IconName::Folder,
            Self::Files => IconName::File,
            Self::Changes => IconName::GitCompare,
            Self::Terminal => IconName::Terminal,
            Self::Problems => IconName::Warning,
            Self::Debug => IconName::Debug,
        }
    }
}

impl From<PanelView> for Tool {
    /// The registered tool hosting a worktree panel view.
    fn from(view: PanelView) -> Self {
        match view {
            PanelView::Terminal => Self::Terminal,
            PanelView::Problems => Self::Problems,
            PanelView::Debug => Self::Debug,
        }
    }
}
