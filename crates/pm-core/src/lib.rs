//! Worktrees, sessions and the git-native session record.

mod files;
mod git;
mod project;
mod scope;
mod session;

pub use files::{Entry, EntryId, FileTree, Row, walk};
pub use git::{
    Blame, Branch, Change, ChangeKind, Changed, FileStatus, Head, Hunk, Line, LineKind, Said, Side,
    Status, Summary, add_worktree, baseline, blame, branches, changes, clone, commit,
    create_branch, diff, diffs, discard, discard_all, fetch, fetch_from, force_push, last_message,
    named, pull, push_branch, push_to, remotes, remove_worktree, since, stage, switch_branch,
    unstage, worktrees, write_index,
};
pub use project::{OpenError, Project, ProjectId, Projects};
pub use scope::Scope;
pub use session::{Bootstrap, Session, SessionId, Sessions, StartError, Started, slug};
