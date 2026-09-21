//! Worktrees, sessions and the git-native session record.

mod files;
mod project;

pub use files::{Entry, EntryId, FileTree, Row};
pub use project::{OpenError, Project, ProjectId, Projects};
