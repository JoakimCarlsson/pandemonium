//! Worktrees, sessions and the git-native session record.

mod files;
mod git;
mod project;

pub use files::{Entry, EntryId, FileTree, Row, walk};
pub use git::{
    Blame, Branch, Change, ChangeKind, Changed, FileStatus, Head, Hunk, Line, LineKind, Said, Side,
    Status, baseline, blame, branches, changes, commit, create_branch, diff, diffs, discard,
    discard_all, fetch, fetch_from, force_push, last_message, pull, push_branch, push_to, remotes,
    stage, switch_branch, unstage, write_index,
};
pub use project::{OpenError, Project, ProjectId, Projects};
