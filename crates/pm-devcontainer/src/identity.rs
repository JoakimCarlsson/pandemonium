//! Which environment a project's checkout or worktree owns.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The project and checkout an environment belongs to.
///
/// An environment is scoped by both, never by an implicit current repository:
/// two worktrees of one project have two identities, and so two containers
/// and two sets of writable volumes.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Identity {
    /// The durable name of the project, its source root as stored.
    project: String,
    /// The checkout or worktree folder on the desktop.
    checkout: PathBuf,
}

impl Identity {
    /// Names the environment of `checkout` within `project`.
    pub fn new(project: impl Into<String>, checkout: impl Into<PathBuf>) -> Self {
        Self {
            project: project.into(),
            checkout: checkout.into(),
        }
    }

    /// The durable name of the project.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// The checkout or worktree folder on the desktop.
    pub fn checkout(&self) -> &Path {
        &self.checkout
    }

    /// The sixteen hexadecimal digits that stand for this identity in names, labels and volumes.
    pub fn id(&self) -> String {
        let text = format!("{}\0{}", self.project, self.checkout.display());
        format!("{:016x}", hash(text.as_bytes()))
    }

    /// A short name safe to put in a container name, taken from the checkout folder.
    pub fn slug(&self) -> String {
        let name = self.checkout.file_name().map_or_else(
            || "workspace".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let slug = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>();
        let slug = slug.trim_matches('-');
        let slug = if slug.is_empty() { "workspace" } else { slug };
        slug.chars()
            .take(24)
            .collect::<String>()
            .trim_end_matches('-')
            .to_owned()
    }

    /// The container name this identity owns.
    pub fn container_name(&self) -> String {
        format!("pm-{}-{}", self.slug(), self.id())
    }
}

/// Sixteen hexadecimal digits standing for `bytes`, for noticing that a file changed.
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:016x}", hash(bytes))
}

/// A stable 64-bit FNV-1a hash, so identities survive toolchain upgrades.
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |state, byte| {
        (state ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
