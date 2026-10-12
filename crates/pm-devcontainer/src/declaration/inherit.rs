//! Bringing a project's local dev container configuration into a new worktree.

use super::discover;
use pm_host::Host;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Copies the project's local configuration and supporting files into a new worktree.
///
/// Local files replace their committed versions, including untracked Dockerfiles
/// and lifecycle scripts. The returned declaration preserves the project's choice
/// between `.devcontainer/devcontainer.json` and `.devcontainer.json`.
pub fn inherit(project: &Path, worktree: &Path) -> io::Result<Option<PathBuf>> {
    let Some(declaration) = discover(project) else {
        return Ok(None);
    };
    for name in [".devcontainer", ".devcontainer.json"] {
        let source = project.join(name);
        match fs::symlink_metadata(&source) {
            Ok(_) => copy(&source, &worktree.join(name))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(Some(
        worktree.join(
            declaration
                .strip_prefix(project)
                .map_err(io::Error::other)?,
        ),
    ))
}

/// Copies a configuration entry recursively, preserving symlinks and file permissions.
fn copy(source: &Path, destination: &Path) -> io::Result<()> {
    let kind = fs::symlink_metadata(source)?.file_type();
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_symlink() => fs::remove_file(destination)?,
        Ok(_) | Err(_) => {}
    }
    if kind.is_symlink() {
        if destination.is_file() {
            fs::remove_file(destination)?;
        }
        Host::local()
            .fs()
            .symlink(fs::read_link(source)?, destination)?;
    } else if kind.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, destination)?;
    }
    Ok(())
}
