//! What a project can be built, tested and run with, and the toolchains it is run with.
//!
//! A [`Task`] is a value: a name, a command, a folder, an environment and a
//! group. [`detect`] reads the tasks a folder's manifests imply, [`tasks`]
//! adds the ones the project writes in `.pandemonium.tasks.json`, and
//! [`toolchains`] lists the toolchains installed on the machine. The server
//! starts a task with [`Launch`], in a shell it keeps for the client or, when
//! a debug scenario needs it first, to the end with [`Launch::finish`]. This
//! crate knows nothing of panes, sessions or protocols.

mod cargo;
mod detect;
mod file;
mod go;
mod launch;
mod make;
mod npm;
mod python;
mod task;
mod toolchain;

pub use detect::{Found, detect, tasks};
pub use file::FILE;
pub use launch::Launch;
pub use task::Task;
pub use toolchain::{Toolchain, family, toolchains};
