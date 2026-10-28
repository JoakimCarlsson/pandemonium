//! What a project can be built, tested and run with, and the toolchains it is run with.
//!
//! A [`Task`] is a value: a name, a command, a folder, an environment and a
//! group. A language ships [`Template`]s in its `tasks.json`: [`detect`] makes
//! the tasks a folder's manifests imply of them, [`tasks`] adds the ones the
//! project writes in `.pandemonium.tasks.json`, [`candidates`] picks what a
//! place of a file tagged by its language runs, and
//! [`toolchains`] lists the toolchains installed on the machine. The server
//! starts a task with [`Launch`], in a shell it keeps for the client or, when
//! a debug scenario needs it first, to the end with [`Launch::finish`]. This
//! crate knows nothing of panes, sessions or protocols.

mod condition;
mod detect;
mod file;
mod launch;
mod task;
mod template;
mod toolchain;
mod variables;

pub use condition::{Condition, Contains};
pub use detect::{Found, Reader, candidates, detect, tasks};
pub use file::FILE;
pub use launch::Launch;
pub use task::Task;
pub use template::{Groups, Template, parse_templates};
pub use toolchain::{Toolchain, toolchains};
pub use variables::{CUSTOM, Variables, needs_place, quote};
