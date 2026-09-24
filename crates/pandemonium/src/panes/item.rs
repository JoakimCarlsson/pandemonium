//! What one tab of a pane holds.
//!
//! Every pane in the window is the same pane: a bar of tabs with one of them
//! in front. What a tab holds is this — a file, a review of everything a
//! project has changed, the diff of one of those files, the editor's own
//! settings — and the tree neither knows nor cares which, because splitting,
//! tabbing, carrying and closing are the same four things whichever it is.

use pm_core::Scope;

use crate::agent::TalkId;
use crate::editor::FileId;
use crate::image::ImageId;
use crate::review::ChangeId;

/// One thing a pane can hold in a tab.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Item {
    /// A file, open in the editor.
    File(FileId),
    /// A picture, open to be looked at.
    Image(ImageId),
    /// An open file read as it renders — markdown as its headings, lists
    /// and code — beside the text it is rendered from.
    Rendered(FileId),
    /// Everything that has changed in one worktree, gathered for review.
    ///
    /// The review is the worktree's rather than the tab's: two panes showing
    /// the same worktree's changes are two views of one thing, the way two
    /// panes showing one file are.
    Review(Scope),
    /// The diff of one changed file, which is what a row of that list opens.
    Change(Scope, ChangeId),
    /// Everything that has changed in one worktree, as excerpts of the files
    /// themselves, edited in place in one pane.
    Excerpts(Scope),
    /// One conversation held in a worktree, with everything said in it.
    Agent(Scope, TalkId),
    /// The program a worktree is debugging: its stack, its variables and
    /// its console.
    Debug(Scope),
    /// The editor's preferences, which belong to the window, not a worktree.
    Settings,
}

impl Item {
    /// The file this holds, when what it holds is a file.
    pub fn file(self) -> Option<FileId> {
        match self {
            Self::File(file) => Some(file),
            _ => None,
        }
    }

    /// The picture this holds, when what it holds is one.
    pub fn image(self) -> Option<ImageId> {
        match self {
            Self::Image(image) => Some(image),
            _ => None,
        }
    }

    /// The file this reads as it renders, when that is what it holds.
    pub fn rendered(self) -> Option<FileId> {
        match self {
            Self::Rendered(file) => Some(file),
            _ => None,
        }
    }

    /// The changed file this is the diff of, when that is what it is.
    pub fn change(self) -> Option<ChangeId> {
        match self {
            Self::Change(_, change) => Some(change),
            _ => None,
        }
    }

    /// The worktree whose changes this reviews, when that is what it is.
    pub fn review(self) -> Option<Scope> {
        match self {
            Self::Review(scope) => Some(scope),
            _ => None,
        }
    }

    /// The worktree whose changes this holds as excerpts, when it does.
    pub fn excerpts(self) -> Option<Scope> {
        match self {
            Self::Excerpts(scope) => Some(scope),
            _ => None,
        }
    }

    /// The agent session this holds, when that is what it holds.
    pub fn session(self) -> Option<TalkId> {
        match self {
            Self::Agent(_, session) => Some(session),
            _ => None,
        }
    }

    /// The worktree whose program this debugs, when that is what it holds.
    pub fn debugged(self) -> Option<Scope> {
        match self {
            Self::Debug(scope) => Some(scope),
            _ => None,
        }
    }

    /// Whether this belongs to the window rather than to one worktree, and
    /// so is drawn whichever worktree the window is showing.
    pub fn is_window_wide(self) -> bool {
        matches!(self, Self::Settings)
    }
}
