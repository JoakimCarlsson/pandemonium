//! A path together with the machine on which it exists.

use crate::Host;
use std::path::{Path, PathBuf};

/// A file or directory on a particular machine.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Location {
    /// The machine holding this path.
    pub host: Host,
    /// The machine's absolute path.
    pub path: PathBuf,
}

impl Location {
    /// Names `path` on `host`.
    pub fn new(host: Host, path: impl Into<PathBuf>) -> Self {
        Self {
            host,
            path: path.into(),
        }
    }

    /// Names a local path.
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::new(Host::local(), path)
    }

    /// Names another path on the same machine.
    pub fn at(&self, path: impl Into<PathBuf>) -> Self {
        Self::new(self.host.clone(), path)
    }

    /// The path without its machine identity, for pure path calculations.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// The preference representation, preserving the SSH alias.
    pub fn stored(&self) -> PathBuf {
        match self.host.name() {
            Some(host) => PathBuf::from(format!("ssh://{host}{}", self.path.display())),
            None => self.path.clone(),
        }
    }
}

impl std::ops::Deref for Location {
    /// The path used for host independent calculations.
    type Target = Path;
    /// Borrows the path for calculations that do not perform I/O.
    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for Location {
    /// Borrows the path for a command argument.
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl From<PathBuf> for Location {
    /// Converts an explicitly local path.
    fn from(path: PathBuf) -> Self {
        Self::local(path)
    }
}

impl From<&Path> for Location {
    /// Converts an explicitly local path.
    fn from(path: &Path) -> Self {
        Self::local(path)
    }
}

impl From<&PathBuf> for Location {
    /// Converts an explicitly local path.
    fn from(path: &PathBuf) -> Self {
        Self::local(path)
    }
}

impl From<&Location> for Location {
    /// Retains the machine when borrowing a location.
    fn from(location: &Location) -> Self {
        location.clone()
    }
}
