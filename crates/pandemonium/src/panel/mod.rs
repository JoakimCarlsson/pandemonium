//! Worktree tools for diagnostics, debugging and shells.
//!
//! These views supply content to ordinary pane tabs. Their models own the
//! worktree state; the shared pane tree owns grouping, placement and resizing.

mod problems;
mod view;

pub use problems::{Problem, ProblemFile};
pub use view::{Panel, panel_content};

/// Which worktree tool view to show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PanelView {
    /// What the language servers have said about the open files.
    Problems,
    /// The program the worktree is debugging, and its console.
    Debug,
    /// The shells running in the worktree.
    #[default]
    Terminal,
}

impl PanelView {
    /// What this view is called on its pane tab.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Problems => "Problems",
            Self::Debug => "Debug Console",
            Self::Terminal => "Terminal",
        }
    }
}
