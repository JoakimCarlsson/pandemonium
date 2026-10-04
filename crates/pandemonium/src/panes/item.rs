//! What one tab of a pane holds.
//!
//! Every pane in the window is the same pane: a bar of tabs with one of them
//! in front. What a tab holds is this — a file, a review of everything a
//! project has changed, the diff of one of those files, a workspace tool
//! — and the tree neither knows nor cares which, because splitting,
//! tabbing, carrying and closing are the same four things whichever it is.

use pm_core::Scope;

use crate::agent::TalkId;
use crate::editor::FileId;
use crate::image::ImageId;
use crate::panes::Tool;
use crate::review::ChangeId;

/// What a tab is for, which decides which panes it is at home in.
///
/// A pane has no role of its own: it serves the roles of the tabs open in it,
/// so a pane of files is where the next file goes, and a pane someone has
/// dragged a file and a conversation into is at home to both.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Role {
    /// Reading and editing: files, pictures, diffs, reviews and search.
    Editor,
    /// Talking to an agent: its conversations and the launcher for them.
    Agent,
    /// Shells.
    Terminal,
    /// Every other workspace tool, which keeps to the pane it was put in.
    Tool,
}

/// One thing a pane can hold in a tab.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Item {
    /// A workspace tool, hosted by the same pane as any document tab.
    Tool(Tool),
    /// A file, open in the editor.
    File(FileId),
    /// A picture, open to be looked at.
    Image(ImageId),
    /// An open file read as it renders — markdown as its headings, lists
    /// and code — beside the text it is rendered from.
    Rendered(FileId),
    /// A live tree of declarations in the last focused file of a worktree.
    Outline(Scope),
    /// Everything that has changed in one worktree, gathered for review.
    ///
    /// The review is the worktree's rather than the tab's: two panes showing
    /// the same worktree's changes are two views of one thing, the way two
    /// panes showing one file are.
    Review(Scope),
    /// A persisted, read-only comparison of worktree turns.
    Turns(Scope, TurnSpan),
    /// The diff of one changed file, which is what a row of that list opens.
    Change(Scope, ChangeId),
    /// Everything that has changed in one worktree, as excerpts of the files
    /// themselves, edited in place in one pane.
    Excerpts(Scope),
    /// Search and replace results for one worktree.
    Search(Scope),
    /// One conversation held in a worktree, with everything said in it.
    Agent(Scope, TalkId),
}

impl Item {
    /// What this is for.
    pub fn role(self) -> Role {
        match self {
            Self::Tool(Tool::Chat) | Self::Agent(..) => Role::Agent,
            Self::Tool(Tool::Terminal) => Role::Terminal,
            Self::Tool(_) => Role::Tool,
            _ => Role::Editor,
        }
    }

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
            Self::Excerpts(scope) | Self::Search(scope) => Some(scope),
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

    /// Whether this belongs to the window rather than to one worktree, and
    /// so is drawn whichever worktree the window is showing.
    pub fn is_window_wide(self) -> bool {
        matches!(self, Self::Tool(_))
    }
}

/// A comparison of completed turns, optionally using the second turn's start baseline.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct TurnSpan {
    /// The earlier completed turn.
    pub from: u64,
    /// The later completed turn.
    pub to: u64,
    /// Exclude reader edits captured before the later turn began.
    pub baseline: bool,
}
