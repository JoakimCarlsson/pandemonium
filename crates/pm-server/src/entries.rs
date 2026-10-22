//! The files and folders of a project folder, made, renamed, copied and removed for a client.
//!
//! Every path is taken from the root of the folder the client names and is
//! refused when it would leave it. Nothing is overwritten: a name that is
//! taken is an error, except for a copy, which is given a free name beside
//! the original.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Fails unless the nearest existing folder at or above `start`, with its links followed, is inside `root`.
fn contained(root: &Path, start: &Path) -> Result<(), String> {
    let real_root = root
        .canonicalize()
        .map_err(|error| refused("open", root, &error))?;
    let nearest = start
        .ancestors()
        .find_map(|ancestor| ancestor.canonicalize().ok())
        .ok_or_else(|| format!("{} is not inside the folder", start.display()))?;
    if nearest.starts_with(&real_root) {
        Ok(())
    } else {
        Err(format!("{} leads outside the folder", start.display()))
    }
}

/// `relative` joined onto `root`, refused when it is empty, absolute, steps upward or goes through a link that leaves `root`; a link at its last part is not followed.
fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    let clean = !relative.is_empty()
        && !relative.contains(['\0', '\n', '\r'])
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    if !clean {
        return Err(format!("{relative:?} is not a path inside the folder"));
    }
    let joined = root.join(path);
    contained(root, joined.parent().unwrap_or(root))?;
    Ok(joined)
}

/// `relative` joined onto `root`, where an empty `relative` is the root itself, refused like [`inside`] and when the folder it names is a link that leaves `root`.
fn inside_or_root(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty() {
        return Ok(root.to_path_buf());
    }
    let joined = inside(root, relative)?;
    contained(root, &joined)?;
    Ok(joined)
}

/// A failure of `action` on `path`, worded for the reader.
fn refused(action: &str, path: &Path, error: &io::Error) -> String {
    format!("could not {action} {}: {error}", path.display())
}

/// Fails when something is already at `path`.
fn refuse_taken(path: &Path) -> Result<(), String> {
    if path.symlink_metadata().is_ok() {
        Err(format!("{} already exists", path.display()))
    } else {
        Ok(())
    }
}

/// Makes an empty file at `relative`, or a folder when `folder`, and the folders above it.
pub fn create(root: &Path, relative: &str, folder: bool) -> Result<(), String> {
    let path = inside(root, relative)?;
    refuse_taken(&path)?;
    let made = match (folder, path.parent()) {
        (true, _) => fs::create_dir_all(&path),
        (false, Some(parent)) => fs::create_dir_all(parent).and_then(|()| fs::write(&path, b"")),
        (false, None) => fs::write(&path, b""),
    };
    made.map_err(|error| refused("create", &path, &error))
}

/// Renames `from` to `to`, refusing a name that is taken and a folder moved into itself.
pub fn rename(root: &Path, from: &str, to: &str) -> Result<(), String> {
    let (from, to) = (inside(root, from)?, inside(root, to)?);
    if from == to {
        return Ok(());
    }
    if to.starts_with(&from) {
        return Err("a folder cannot be moved into itself".to_owned());
    }
    let only_case = from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
    if !only_case {
        refuse_taken(&to)?;
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| refused("create", parent, &error))?;
    }
    fs::rename(&from, &to).map_err(|error| refused("rename", &from, &error))
}

/// Splits `name` before its extension, keeping the dot with the extension; a leading dot starts no extension.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(at) => name.split_at(at),
    }
}

/// The first name in `directory` built from `name` that nothing holds yet.
fn free_name(directory: &Path, name: &str) -> PathBuf {
    let plain = directory.join(name);
    if plain.symlink_metadata().is_err() {
        return plain;
    }
    let (stem, extension) = split_extension(name);
    (1..)
        .map(|count| match count {
            1 => format!("{stem} copy{extension}"),
            count => format!("{stem} copy {count}{extension}"),
        })
        .map(|candidate| directory.join(candidate))
        .find(|candidate| candidate.symlink_metadata().is_err())
        .unwrap_or(plain)
}

/// Copies `from` to `to`, and everything under it when it is a folder; a link is copied as a link.
fn copy_all(from: &Path, to: &Path) -> io::Result<()> {
    let metadata = from.symlink_metadata()?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(from)?;
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, to);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_file(target, to);
    }
    if !metadata.is_dir() {
        return fs::copy(from, to).map(drop);
    }
    fs::create_dir_all(to)?;
    for child in fs::read_dir(from)? {
        let child = child?;
        copy_all(&child.path(), &to.join(child.file_name()))?;
    }
    Ok(())
}

/// Copies `from` into the folder `into`, under a name nothing there holds yet.
pub fn copy(root: &Path, from: &str, into: &str) -> Result<(), String> {
    let from = inside(root, from)?;
    let directory = inside_or_root(root, into)?;
    if directory.starts_with(&from) && from.is_dir() {
        return Err("a folder cannot be copied into itself".to_owned());
    }
    let name = from
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no name", from.display()))?;
    let to = free_name(&directory, name);
    copy_all(&from, &to).map_err(|error| refused("copy", &from, &error))
}

/// Takes `relative` off the disk, and everything in it when it is a folder.
pub fn remove(root: &Path, relative: &str) -> Result<(), String> {
    let path = inside(root, relative)?;
    let metadata = path
        .symlink_metadata()
        .map_err(|error| refused("find", &path, &error))?;
    let removed = if metadata.is_dir() {
        fs::remove_dir_all(&path)
    } else {
        fs::remove_file(&path)
    };
    removed.map_err(|error| refused("remove", &path, &error))
}

/// Replaces the contents of the file at `relative` with `text`, making it when it is not there and refusing a folder.
pub fn write(root: &Path, relative: &str, text: &str) -> Result<(), String> {
    let path = inside(root, relative)?;
    if path.is_dir() {
        return Err(format!("{} is a folder", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| refused("create", parent, &error))?;
    }
    fs::write(&path, text).map_err(|error| refused("write", &path, &error))
}

/// Lists one level of `relative`, without following links outside the project or walking subfolders.
pub(crate) fn children(root: &Path, relative: &str) -> Vec<(String, bool)> {
    let Ok(folder) = inside_or_root(root, relative) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let kind = entry.file_type().ok()?;
            let path = format!("{relative}/{}", entry.file_name().to_string_lossy());
            Some((path, kind.is_dir()))
        })
        .collect()
}

/// Checks shallow ignored listings and their project boundaries.
#[cfg(test)]
mod tests {
    use super::*;

    /// Ignored contents are listed one level at a time without consuming the tracked file budget.
    #[test]
    fn ignored_children_are_shallow_and_refreshable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build")
            .join(format!("ignored-children-{}", std::process::id()));
        fs::create_dir_all(root.join("server/prod")).unwrap();
        fs::create_dir_all(root.join("server/stage")).unwrap();
        fs::write(root.join("server/prod/connect.sh"), "").unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-q"])
                .arg(&root)
                .status()
                .unwrap()
                .success()
        );
        fs::write(root.join(".gitignore"), "server\n").unwrap();
        fs::write(root.join("tracked.rs"), "").unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["add", ".gitignore", "tracked.rs"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        assert!(pm_git::files(&root, 2).contains(&"tracked.rs".to_owned()));
        assert_eq!(pm_git::ignored(&root, 2), vec!["server/".to_owned()]);
        assert_eq!(children(&root, "server").len(), 2);
        assert_eq!(
            children(&root, "server/prod"),
            vec![("server/prod/connect.sh".to_owned(), false)]
        );
        fs::write(root.join("server/prod/new.sh"), "echo ready\n").unwrap();
        assert_eq!(children(&root, "server/prod").len(), 2);
        rename(&root, "server/prod/new.sh", "server/prod/renamed.sh").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("server/prod/renamed.sh")).unwrap(),
            "echo ready\n"
        );
        remove(&root, "server/prod/renamed.sh").unwrap();
        assert_eq!(children(&root, "server/prod").len(), 1);
        for index in 0..20_001 {
            fs::write(root.join(format!("server/stage/{index}")), "").unwrap();
        }
        assert_eq!(children(&root, "server/stage").len(), 20_001);
        assert!(pm_git::files(&root, 2).contains(&"tracked.rs".to_owned()));
        assert!(children(&root, "../").is_empty());
        assert!(children(&root, "missing").is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
