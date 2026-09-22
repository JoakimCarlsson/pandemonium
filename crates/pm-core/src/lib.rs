//! Worktrees, sessions and the git-native session record.

mod files;
mod git;
mod project;

pub use files::{Entry, EntryId, FileTree, Row, walk};
pub use git::{Blame, Change, ChangeKind, FileStatus, baseline, blame, changes, status};
pub use project::{OpenError, Project, ProjectId, Projects};
