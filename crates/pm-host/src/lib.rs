//! Machine access for local and SSH projects, with a multiplexed byte transport.

mod command;
mod filesystem;
mod flow;
mod host;
mod ignore;
mod location;
mod pty;
mod remote;
mod server;
mod walk;
mod watch;
pub mod wire;

pub use command::{Child, Command, Input, Output, Stdio};
pub use filesystem::{DirEntry, FileSystem, FileType, Metadata};
pub use host::{Host, Hosts};
pub use location::Location;
pub use pty::{CommandBuilder, ExitStatus, Pty, PtyChild, PtyControl};
pub use server::serve;
pub use watch::{Disk, Touch, Touched, Watcher};
