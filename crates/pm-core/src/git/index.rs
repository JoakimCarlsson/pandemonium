//! What the index holds for a file: the text the working copy is read against.

use std::path::Path;
use std::process::{Command, Stdio};

/// The text the index holds for `path`, in the repository at `root`.
///
/// A file that git has never heard of has no baseline, which is what makes
/// every line of a new file read as added rather than as unchanged.
pub fn baseline(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let output = Command::new("git")
        .arg("show")
        .arg(format!(":{}", relative.display()))
        .current_dir(root)
        .stderr(Stdio::null())
        .output()
        .ok()?;

    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
