//! What git has to say about a worktree, and what it is asked to do to one.
//!
//! The rest of `pm-core` reads a repository straight off its files, because
//! opening a project is a handful of reads. What git holds is not: what has
//! changed, what a file used to look like, how it differs line by line and
//! who last touched each of them all mean asking git itself. Every question
//! is best effort — a path outside a repository, a git that is not
//! installed, a file that was never committed all come back as nothing to
//! show rather than as an error — while every change comes back as what git
//! said when it would not make it, because a stage that did not happen is
//! something the reader has to be told about.

mod blame;
mod branch;
mod changes;
mod clone;
mod commit;
mod diff;
mod graph;
mod head;
mod index;
mod operation;
mod run;
mod status;
mod worktree;

pub use blame::{Blame, blame};
pub use branch::{
    Branch, branches, create_branch, fetch, fetch_from, force_push, pull, push_branch, push_to,
    remotes, switch_branch, sync,
};
pub use changes::{Change, ChangeKind, changes};
pub use clone::{clone, named};
pub use commit::{Commit, commit, history, last_message};
pub use diff::{Hunk, Line, LineKind, Side, diff, diffs};
pub use graph::{Edge, Half, Lanes};
pub use head::Head;
pub use index::{
    Revision, baseline, committed, contents, discard, discard_all, stage, unstage, write_index,
};
pub use operation::{Merge, Operation, abort_merge, operation};
pub use run::Said;
pub use status::{Changed, FileStatus, Status};
pub use worktree::{
    Summary, add_worktree, commit_of, remember, remember_port, remembered_base, remembered_name,
    remembered_port, remove_worktree, since, worktrees,
};
