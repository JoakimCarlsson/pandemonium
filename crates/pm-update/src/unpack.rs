//! Unpacking a downloaded archive and finding the program in it.

use std::fs::File;
use std::path::{Path, PathBuf};

/// Unpacks `archive`, a `.tar.gz` or a `.zip`, into `into`.
pub(crate) fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let name = archive.to_string_lossy();
    let file = File::open(archive).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(into).map_err(|error| error.to_string())?;
    if name.ends_with(".zip") {
        zip::ZipArchive::new(file)
            .and_then(|mut zip| zip.extract(into))
            .map_err(|error| format!("could not unpack the download: {error}"))
    } else {
        tar::Archive::new(flate2::read::GzDecoder::new(file))
            .unpack(into)
            .map_err(|error| format!("could not unpack the download: {error}"))
    }
}

/// What an unpacked update is installed from: the app bundle on macOS, the program named `program` elsewhere, wherever below `root` it is.
pub(crate) fn find(root: &Path, program: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if cfg!(target_os = "macos") && name.ends_with(".app") {
            return Some(path);
        }
        if !cfg!(target_os = "macos") && name == program && path.is_file() {
            return Some(path);
        }
        if path.is_dir()
            && !name.ends_with(".app")
            && let Some(found) = find(&path, program)
        {
            return Some(found);
        }
    }
    None
}
