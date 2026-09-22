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
mod commit;
mod diff;
mod head;
mod index;
mod run;
mod status;

pub use blame::{Blame, blame};
pub use branch::{
    Branch, branches, create_branch, fetch, fetch_from, force_push, pull, push_branch, push_to,
    remotes, switch_branch,
};
pub use changes::{Change, ChangeKind, changes};
pub use commit::{commit, last_message};
pub use diff::{Hunk, Line, LineKind, Side, diff, diffs};
pub use head::Head;
pub use index::{baseline, discard, discard_all, stage, unstage, write_index};
pub use run::Said;
pub use status::{Changed, FileStatus, Status};
