//! Pinned language server recipes and atomic installation into the editor's home.

mod recipes;
mod worker;

pub use recipes::recipe;
pub use worker::{install, platform, prune_older};

/// A platform release asset and its expected SHA-256 digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Build {
    /// Operating system and CPU architecture.
    pub platform: &'static str,
    /// The pinned HTTPS asset URL.
    pub url: &'static str,
    /// The expected hexadecimal SHA-256 digest.
    pub sha256: &'static str,
}

/// One way to fetch and install a server at a pinned version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recipe {
    /// A checked single executable or archive from a release.
    Release {
        version: &'static str,
        builds: &'static [Build],
    },
    /// A package installed by npm, possibly with extra packages.
    Npm {
        package: &'static str,
        version: &'static str,
        extra: &'static [&'static str],
    },
    /// A Go module installed into GOBIN.
    Go {
        module: &'static str,
        version: &'static str,
    },
    /// A Python package installed in a private virtual environment.
    Pip {
        package: &'static str,
        version: &'static str,
    },
}

impl Recipe {
    /// The version pinned to this build of the editor.
    pub const fn version(self) -> &'static str {
        match self {
            Self::Release { version, .. }
            | Self::Npm { version, .. }
            | Self::Go { version, .. }
            | Self::Pip { version, .. } => version,
        }
    }
}
