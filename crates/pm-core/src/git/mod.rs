//! What git has to say about a file: what it held, and who last wrote it.
//!
//! The rest of `pm-core` reads a repository straight off its files, because
//! opening a project is a handful of reads. These two questions are not:
//! working out what a file used to look like and who last touched each line
//! means asking git itself. Both are best effort — a path outside a
//! repository, a git that is not installed, a file that was never committed
//! all come back as nothing to show rather than as an error.

mod blame;
mod changes;
mod index;
mod status;

pub use blame::{Blame, blame};
pub use changes::{Change, ChangeKind, changes};
pub use index::baseline;
pub use status::{FileStatus, status};
