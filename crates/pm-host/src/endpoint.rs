//! A machine reached by launching the endpoint through a local command.

use crate::Location;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A provider of a machine that is reached through a local launcher.
///
/// A dev container is the first one: its tools run where `docker exec`
/// starts them, while its workspace may be a bind mount of a desktop folder.
/// The transport, handshake and operations are the SSH machine's own; only
/// how a shell is started on the machine differs.
pub trait Endpoint: Send + Sync {
    /// The stable name of this machine within its [`Endpoint::scheme`].
    fn name(&self) -> String;

    /// What the machine is called where it is shown beside a project name.
    fn label(&self) -> String {
        self.name()
    }

    /// The prefix a stored location of this machine carries, before `://`.
    fn scheme(&self) -> &'static str;

    /// A command that runs `script` in a POSIX shell on the machine, with the
    /// standard streams left for the caller to pipe.
    fn shell(&self, script: &str) -> io::Result<Command>;

    /// Gets the machine ready to be launched into, before every connection.
    fn prepare(&self) -> io::Result<()> {
        Ok(())
    }

    /// The desktop path a machine path is a bind mount of, when it is one.
    fn native(&self, _path: &Path) -> Option<PathBuf> {
        None
    }

    /// Where the desktop folder `native` is worked on when its machine already exists, without starting anything.
    fn locate(&self, _native: &Path) -> io::Result<Location> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this machine cannot locate an environment of its own",
        ))
    }

    /// Takes away the environment of `native`, the folder it was created for, keeping its files.
    fn retire(&self, _native: &Path) -> io::Result<()> {
        Ok(())
    }

    /// Where the desktop folder `native` is worked on, in a machine of its own, set up and ready.
    fn isolate(&self, _native: &Path) -> io::Result<Location> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this machine cannot provide an environment of its own",
        ))
    }
}
