//! The bottom panel: the views that sit under the panes, one in front.
//!
//! The panel is a row of views along its top — what the language servers
//! have found, the program being debugged and the worktree's shells — and
//! whichever is chosen beneath it. [`PanelView`] names them and
//! [`bottom_panel`] is the screen; the stores behind each view are where
//! the things themselves live.

mod problems;
mod view;

pub use problems::{Problem, ProblemFile};
pub use view::{Panel, bottom_panel};

/// Which of the panel's views is in front.
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
    /// Every view, in the order the panel's bar offers them.
    pub const ALL: [Self; 3] = [Self::Problems, Self::Debug, Self::Terminal];

    /// What the bar calls this view.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Problems => "Problems",
            Self::Debug => "Debug Console",
            Self::Terminal => "Terminal",
        }
    }
}
