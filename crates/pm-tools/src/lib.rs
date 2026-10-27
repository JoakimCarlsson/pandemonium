//! Finding the programs a machine has installed, for a process that was not started from a shell.

mod find;

pub use find::{generated, installed, path_beside, set_directories, set_generated};
