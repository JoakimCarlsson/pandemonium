//! The files of one worktree, as a tree the window can draw and expand.
//!
//! Directories are read when they are first expanded and remembered after, so
//! the tree costs one listing per directory a person actually opens rather
//! than a scan of the whole repository. What is expanded is state the tree
//! keeps; what is on disk it reads.

mod entry;
mod tree;

pub use entry::{Entry, EntryId, Row};
pub use tree::FileTree;
