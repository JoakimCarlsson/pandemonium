//! Worktrees, sessions and the git-native session record.

mod files;
mod git;
mod project;

pub use files::{Entry, EntryId, FileTree, Row, walk};
pub use git::{
    Blame, Change, ChangeKind, Changed, FileStatus, Head, Hunk, Line, LineKind, Said, Side, Status,
    baseline, blame, changes, commit, diff, diffs, discard, discard_all, last_message, stage,
    unstage, write_index,
};
pub use project::{OpenError, Project, ProjectId, Projects};
