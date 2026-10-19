//! Finding the programs a machine has installed, for a process that was not started from a shell.

mod find;

pub use find::{installed, path_beside};
