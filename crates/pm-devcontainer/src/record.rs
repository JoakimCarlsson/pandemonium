//! What is kept on disk so an environment can be found again after the editor restarts.

use crate::declaration::Hooks;
use crate::identity::Identity;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

/// The durable description of one owned container.
///
/// The container's labels are the source of truth for ownership; the record
/// holds what cannot be read back from them, such as the resolved tool
/// context, so that reconnecting never depends on the declaration being
/// valid or unchanged.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Record {
    /// Which project and checkout owns the environment.
    pub identity: Identity,
    /// The container name.
    pub container: String,
    /// The declaration the container was created from.
    pub declaration: PathBuf,
    /// A digest of that declaration when the container was created.
    pub digest: String,
    /// The engine program that created the container.
    pub engine: String,
    /// Where the source appears in the container.
    pub workspace: String,
    /// The user tools run as; none leaves the container's own.
    pub user: Option<String>,
    /// The variables tools run with.
    pub env: Vec<(String, String)>,
    /// The lifecycle commands that run inside the container.
    pub hooks: Hooks,
    /// The ports to forward to the desktop.
    pub forward_ports: Vec<u16>,
    /// The editor extensions the declaration asks for.
    pub extensions: Vec<String>,
    /// Whether the creation commands have completed.
    pub created: bool,
}

/// The directory holding every record and log of this editor's environments.
#[derive(Clone, Debug)]
pub struct Records {
    /// The directory itself.
    directory: PathBuf,
}

impl Records {
    /// Records kept under `directory`.
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    /// The file holding the record of `id`.
    fn file(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.json"))
    }

    /// The log of the latest attempt on `id`.
    pub fn log(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.log"))
    }

    /// Replaces the record of `id`, atomically.
    pub fn save(&self, id: &str, record: &Record) -> io::Result<()> {
        private_directory(&self.directory)?;
        let file = self.file(id);
        let staging = file.with_extension("json.new");
        let bytes = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
        io::Write::write_all(&mut private_file(&staging, true)?, &bytes)?;
        std::fs::rename(staging, file)
    }

    /// The record of `id`, if one was kept.
    pub fn load(&self, id: &str) -> Option<Record> {
        serde_json::from_slice(&std::fs::read(self.file(id)).ok()?).ok()
    }

    /// Forgets the record and the log of `id`.
    pub fn forget(&self, id: &str) {
        let _ = std::fs::remove_file(self.file(id));
        let _ = std::fs::remove_file(self.log(id));
    }

    /// Every record kept, for finding environments after a restart.
    pub fn all(&self) -> Vec<Record> {
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .filter_map(|entry| serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok())
            .collect()
    }
}

/// Creates `directory` readable by its owner alone.
pub(crate) fn private_directory(directory: &std::path::Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

/// Opens `path` for writing, readable by its owner alone, appending unless `truncate`.
pub(crate) fn private_file(path: &std::path::Path, truncate: bool) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}
