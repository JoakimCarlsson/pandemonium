//! What the file tree does to the disk: make, rename, move, copy and remove.
//!
//! Each of these is the one implementation of its operation, so the tree's
//! menu, its keys and a drag across it all end up here. None of them
//! overwrites anything: a name that is taken is refused, or, for a copy
//! pasted where the original already is, given a free name beside it the
//! way a file manager names a duplicate.

use pm_host::Location;
use std::io;
use std::path::{Path, PathBuf};

/// Makes an empty file at `path`, and the directories above it.
pub fn create_file(path: impl Into<Location>) -> io::Result<()> {
    let path = path.into();
    refuse_taken(&path.host, &path)?;
    if let Some(parent) = path.parent() {
        path.host.fs().create_dir_all(parent)?;
    }
    path.host.fs().create_file(&path)
}

/// Makes the directory at `path`, and the directories above it.
pub fn create_dir(path: impl Into<Location>) -> io::Result<()> {
    let path = path.into();
    refuse_taken(&path.host, &path)?;
    path.host.fs().create_dir_all(path)
}

/// Renames `from` to `to`, refusing a name that is already taken.
///
/// A rename that only changes the case of a name is let through, since on a
/// disk that ignores case the new name is "taken" by the file itself.
pub fn rename(from: impl Into<Location>, to: &Path) -> io::Result<()> {
    let from = from.into();
    if from.path == to {
        return Ok(());
    }
    let same_name = from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
    if !same_name {
        refuse_taken(&from.host, to)?;
    }
    if let Some(parent) = to.parent() {
        from.host.fs().create_dir_all(parent)?;
    }
    from.host.fs().rename(&from, to)
}

/// Moves `from` into `directory`, keeping its name, and says where it went.
///
/// A directory cannot be moved into itself or anything under it, and a move
/// onto a name the directory already holds is refused.
pub fn move_into(from: impl Into<Location>, directory: &Path) -> io::Result<PathBuf> {
    let from = from.into();
    let to = directory.join(name_of(&from)?);
    if to == from.path {
        return Ok(to);
    }
    if directory.starts_with(&from) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a directory cannot be moved into itself",
        ));
    }
    refuse_taken(&from.host, &to)?;
    match from.host.fs().rename(&from, &to) {
        Ok(()) => Ok(to),
        Err(_) => {
            copy_all(&from, &to)?;
            remove(&from)?;
            Ok(to)
        }
    }
}

/// Copies `from` into `directory`, and says where the copy went.
///
/// A name the directory already holds is not overwritten: the copy is given
/// the first free name of `name copy.ext`, `name copy 2.ext` and so on.
pub fn copy_into(from: impl Into<Location>, directory: &Path) -> io::Result<PathBuf> {
    let from = from.into();
    if directory.starts_with(&from) && from.host.fs().is_dir(&from) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a directory cannot be copied into itself",
        ));
    }
    let to = free_name(&from.host, directory, &name_of(&from)?);
    copy_all(&from, &to)?;
    Ok(to)
}

/// Moves `path` to the desktop's trash, where it can be brought back from.
pub fn trash(path: impl Into<Location>) -> io::Result<()> {
    let path = path.into();
    path.host.fs().trash(&path)
}

/// Takes `path` off the disk for good, whatever it holds.
pub fn remove(path: impl Into<Location>) -> io::Result<()> {
    let path = path.into();
    path.host.fs().remove(&path)
}

/// Whether `name` is one a file can be given: not empty, not a path upward.
pub fn is_valid_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && !Path::new(trimmed).is_absolute()
        && Path::new(trimmed)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

/// The first name in `directory` built from `name` that nothing holds yet.
fn free_name(host: &pm_host::Host, directory: &Path, name: &str) -> PathBuf {
    let plain = directory.join(name);
    if !exists(host, &plain) {
        return plain;
    }
    let (stem, extension) = split_extension(name);
    (1..)
        .map(|count| match count {
            1 => format!("{stem} copy{extension}"),
            count => format!("{stem} copy {count}{extension}"),
        })
        .map(|candidate| directory.join(candidate))
        .find(|candidate| !exists(host, candidate))
        .unwrap_or(plain)
}

/// `name` split before its extension, keeping the dot with the extension.
///
/// A name that starts with its only dot, `.gitignore`, has no extension.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(at) => name.split_at(at),
    }
}

/// Copies `from` to `to`, and everything under it if it is a directory.
fn copy_all(from: &Location, to: &Path) -> io::Result<()> {
    let metadata = from.host.fs().symlink_metadata(from)?;
    if metadata.file_type().is_symlink() {
        let target = from.host.fs().read_link(from)?;
        return from.host.fs().symlink(&target, to);
    }
    if !metadata.is_dir() {
        return from.host.fs().copy(from, to).map(drop);
    }
    from.host.fs().create_dir(to)?;
    for item in from.host.fs().read_dir(from)? {
        let item = item?;
        copy_all(&from.at(item.path()), &to.join(item.file_name()))?;
    }
    Ok(())
}

/// Whether anything, even a broken link, is at `path`.
fn exists(host: &pm_host::Host, path: &Path) -> bool {
    host.fs().symlink_metadata(path).is_ok()
}

/// Refuses `path` when something is already there.
fn refuse_taken(host: &pm_host::Host, path: &Path) -> io::Result<()> {
    match exists(host, path) {
        true => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        )),
        false => Ok(()),
    }
}

/// The last part of `path`, which is what it keeps when it moves.
fn name_of(path: &Path) -> io::Result<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a path without a name"))
}
