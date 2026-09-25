//! Worktrees, sessions and the git-native session record.

mod files;
mod git;
mod project;
mod scope;
mod session;

pub use files::{Disk, Entry, EntryId, FileTree, Row, Touch, Touched, Watcher, ops, walk};
pub use git::{
    Blame, Branch, Change, ChangeKind, Changed, Commit, Edge, FileStatus, Half, Head, Hunk, Lanes,
    Line, LineKind, Merge, Operation, Said, Side, Status, Summary, abort_merge, add_worktree,
    baseline, blame, branches, changes, clone, commit, committed, create_branch, diff, diffs,
    discard, discard_all, fetch, fetch_from, force_push, history, last_message, named, operation,
    pull, push_branch, push_to, remotes, remove_worktree, since, stage, switch_branch, sync,
    unstage, worktrees, write_index,
};
pub use project::{OpenError, Project, ProjectId, Projects, Repository, repositories};
pub use scope::Scope;
pub use session::{Bootstrap, Session, SessionId, Sessions, StartError, Started, slug};
