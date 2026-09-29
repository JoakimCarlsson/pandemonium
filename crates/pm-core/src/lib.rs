//! Worktrees, sessions and the git-native session record.

mod files;
mod git;
mod project;
mod scope;
mod session;
mod task;

pub use files::{
    Disk, Entry, EntryId, FileTree, Row, Touch, Touched, Watcher, ops, walk, walk_each,
};
pub use git::{
    Blame, Branch, Change, ChangeKind, Changed, CherryPick, Commit, Edge, FileStatus, Half, Head,
    Hunk, Lanes, Line, LineKind, Merge, Operation, Rebase, Revision, Said, Side, Stash, Status,
    Summary, abort_merge, abort_operation, add_worktree, amend, baseline, blame, branches, changes,
    cherry_pick, clone, commit, committed, contents, continue_operation, create_branch, diff,
    diffs, discard, discard_all, fetch, fetch_from, force_push, history, last_message, named,
    operation, pull, push_branch, push_to, remotes, remove_worktree, since, skip_operation, stage,
    stash_apply, stash_drop, stash_pop, stash_push, stashes, switch_branch, sync, unstage,
    untracked, worktrees, write_index,
};
pub use project::{OpenError, Project, ProjectId, Projects, Repository, repositories};
pub use scope::Scope;
pub use session::{
    Bootstrap, Cutting, Found, Session, SessionId, Sessions, StartError, Started, slug,
};
pub use task::{Task, TaskSource, tasks, tasks_checked};
