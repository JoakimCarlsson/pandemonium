//! What one tab of a pane holds.
//!
//! Every pane in the window is the same pane: a bar of tabs with one of them
//! in front. What a tab holds is this — a file, a review of everything a
//! project has changed, the diff of one of those files — and the tree neither
//! knows nor cares which, because splitting, tabbing, carrying and closing are
//! the same four things whichever it is.

use pm_core::ProjectId;

use crate::editor::FileId;
use crate::review::ChangeId;

/// One thing a pane can hold in a tab.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Item {
    /// A file, open in the editor.
    File(FileId),
    /// Everything that has changed in one project, gathered for review.
    ///
    /// The review is the project's rather than the tab's: two panes showing
    /// the same project's changes are two views of one thing, the way two
    /// panes showing one file are.
    Review(ProjectId),
    /// The diff of one changed file, which is what a row of that list opens.
    Change(ProjectId, ChangeId),
}

impl Item {
    /// The file this holds, when what it holds is a file.
    pub fn file(self) -> Option<FileId> {
        match self {
            Self::File(file) => Some(file),
            Self::Review(_) | Self::Change(..) => None,
        }
    }

    /// The changed file this is the diff of, when that is what it is.
    pub fn change(self) -> Option<ChangeId> {
        match self {
            Self::Change(_, change) => Some(change),
            Self::File(_) | Self::Review(_) => None,
        }
    }

    /// The project whose changes this reviews, when that is what it is.
    pub fn review(self) -> Option<ProjectId> {
        match self {
            Self::Review(project) => Some(project),
            Self::File(_) | Self::Change(..) => None,
        }
    }
}
