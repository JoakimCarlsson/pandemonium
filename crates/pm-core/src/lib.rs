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
    Blame, Branch, CHECKPOINT_HEAD, Change, ChangeKind, Changed, Checkpoint, CheckpointStep,
    CherryPick, Commit, Edge, FileStatus, Half, Head, Hunk, Lanes, Line, LineKind, Merge,
    Operation, Rebase, Revision, Said, Side, Stash, Status, Summary, abort_merge, abort_operation,
    add_worktree, amend, baseline, begin_checkpoint, begin_checkpoint_number, between, blame,
    blame_at, branches, changes, checkpoint, checkpoint_at, checkpoint_step, checkpoint_steps,
    checkpoint_turn, checkpoints, cherry_pick, clone, commit, committed, contents,
    continue_operation, create_branch, diff, diffs, discard, discard_all, end_checkpoint, fetch,
    fetch_from, force_push, history, hunk_step, hunk_steps, last_message, named, operation, pull,
    push_branch, push_to, remember_review, remembered_review, remotes, remove_worktree, rewind,
    rewind_number, rewind_paths, since, skip_operation, snapshot, stage, stash_apply, stash_drop,
    stash_pop, stash_push, stashes, switch_branch, sync, take_rewind_context, unstage, untracked,
    worktrees, write_index,
};
pub use project::{OpenError, Project, ProjectId, Projects, Repository, repositories};
pub use scope::Scope;
pub use session::{
    Bootstrap, ConversationFork, Cutting, Delegation, Found, Session, SessionId, Sessions,
    StartError, Started, slug,
};
pub use task::{Task, TaskSource, tasks, tasks_checked};
