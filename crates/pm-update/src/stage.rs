//! Fetching an update and getting it ready to install: download, check against the checksums, unpack.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::asset::Chosen;
use crate::{checksum, download, unpack};

/// Why an update was not staged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Failure {
    /// The reader cancelled it.
    Cancelled,
    /// It could not be done, for this reason.
    Failed(String),
}

impl From<String> for Failure {
    /// The failure with `reason` as its cause.
    fn from(reason: String) -> Self {
        Self::Failed(reason)
    }
}

/// Downloads the `chosen` files into the cache folder, checks the archive against the checksums, and unpacks it; returns what is installed from: the app bundle on macOS, the program elsewhere. `progress` hears how many bytes of the archive have arrived of how many, and the download stops once `cancel` is set. Whatever fails or is cancelled leaves nothing behind.
pub fn stage(
    chosen: &Chosen,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, Failure> {
    let root = pm_store::cache_directory()
        .ok_or_else(|| "the desktop has no cache directory".to_owned())?
        .join("updates");
    let _ = std::fs::remove_dir_all(&root);
    let outcome = fetch_into(&root, chosen, cancel, progress);
    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    outcome
}

/// What [`stage`] does, in `root`, which it leaves as it is when it fails.
fn fetch_into(
    root: &std::path::Path,
    chosen: &Chosen,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, Failure> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let sums = download::text(&chosen.sums.url)?;
    let expected = checksum::expected(&sums, &chosen.archive.name)
        .ok_or_else(|| format!("SHA256SUMS does not list {}", chosen.archive.name))?;
    let archive = root.join(&chosen.archive.name);
    let found = download::file(
        &chosen.archive.url,
        &archive,
        chosen.archive.size,
        cancel,
        progress,
    )?;
    if found != expected {
        return Err(Failure::Failed(format!(
            "{} does not match its checksum: expected {expected}, got {found}",
            chosen.archive.name
        )));
    }
    let unpacked = root.join("unpacked");
    unpack::unpack(&archive, &unpacked)?;
    let program = unpack::find(&unpacked, &chosen.program)
        .ok_or_else(|| format!("the download holds no {}", chosen.program))?;
    #[cfg(unix)]
    if program.is_file() {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }
    Ok(program)
}
