//! The states files were saved in, kept in the git folder of the worktree they belong to so that they outlive the window that saved them.
//!
//! Each file has a folder of its own named after its path, holding one file
//! per save, named by the milliseconds since the epoch the save was made at.
//! What is kept is bounded by an age and by a size, both set in the machine
//! settings.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pm_protocol::{HistoryPreferences, LocalVersion};

use crate::core::now;

/// The folder, inside the git folder of a worktree, that holds the states of its files.
const FOLDER: &str = "pandemonium/history";

/// How many milliseconds make a day.
const DAY: u64 = 24 * 60 * 60 * 1000;

/// The longest the name of the folder of a file may be, in bytes, which keeps it a name the file system takes.
const LONGEST_NAME: usize = 200;

/// The folder that holds the states of the files of the worktree `directory` is in.
fn root_of(directory: &Path) -> Result<PathBuf, String> {
    let (git, _) = pm_git::git_folders(directory)
        .ok_or_else(|| "local history needs a folder that is in a repository".to_owned())?;
    Ok(git.join(FOLDER))
}

/// `path` written so that it is one name: a character that is not a letter, a digit, `.`, `-` or `_` is written as `%` and the two digits of each of its bytes.
fn escaped(path: &str) -> String {
    let mut name = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
            name.push(char::from(byte));
        } else {
            name.push_str(&format!("%{byte:02X}"));
        }
    }
    name
}

/// The folder that holds the states of the file `path` of the worktree `directory` is in.
fn folder_of(directory: &Path, path: &str) -> Result<PathBuf, String> {
    let name = escaped(path);
    if path.is_empty() || name.len() > LONGEST_NAME {
        return Err(format!("{path:?} cannot be kept in local history"));
    }
    Ok(root_of(directory)?.join(name))
}

/// The states kept in `folder`, with the files that hold them, newest first.
fn kept_in(folder: &Path) -> Vec<(LocalVersion, PathBuf)> {
    let mut kept: Vec<(LocalVersion, PathBuf)> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().parse().ok()?;
            let size = entry.metadata().ok()?.len();
            Some((LocalVersion { id, size }, entry.path()))
        })
        .collect();
    kept.sort_by_key(|(version, _)| std::cmp::Reverse(version.id));
    kept
}

/// Removes the states of `kept` that are older than `settings` allow.
fn expire(kept: &mut Vec<(LocalVersion, PathBuf)>, settings: &HistoryPreferences) {
    let oldest = now().saturating_sub(u64::from(settings.days) * DAY);
    kept.retain(|(version, file)| {
        let current = version.id >= oldest;
        if !current {
            let _ = fs::remove_file(file);
        }
        current
    });
}

/// Keeps `text` as the state the file `path` of the worktree `directory` is in was saved in, when `settings` keep states, and removes what has grown too old.
pub(crate) fn record(
    directory: &Path,
    path: &str,
    text: &str,
    settings: &HistoryPreferences,
) -> Result<(), String> {
    if !settings.enabled {
        return Ok(());
    }
    let folder = folder_of(directory, path)?;
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let mut id = now();
    while folder.join(id.to_string()).exists() {
        id += 1;
    }
    fs::write(folder.join(id.to_string()), text).map_err(|error| error.to_string())?;
    expire(&mut kept_in(&folder), settings);
    Ok(())
}

/// The states kept of the file `path` of the worktree `directory` is in, newest first, those that are too old removed first.
pub(crate) fn versions(
    directory: &Path,
    path: &str,
    settings: &HistoryPreferences,
) -> Result<Vec<LocalVersion>, String> {
    let mut kept = kept_in(&folder_of(directory, path)?);
    expire(&mut kept, settings);
    Ok(kept.into_iter().map(|(version, _)| version).collect())
}

/// What the file `path` of the worktree `directory` is in held in the state `version`.
pub(crate) fn read(directory: &Path, path: &str, version: u64) -> Result<String, String> {
    let file = folder_of(directory, path)?.join(version.to_string());
    fs::read(&file)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|error| format!("could not read that state of {path}: {error}"))
}

/// Removes, from everything kept for the worktree `directory` is in, the states that are too old and then the oldest of the rest until what is kept fits the size `settings` allow.
pub(crate) fn sweep(directory: &Path, settings: &HistoryPreferences) -> io::Result<()> {
    let Ok(root) = root_of(directory) else {
        return Ok(());
    };
    let mut all: Vec<(LocalVersion, PathBuf)> = Vec::new();
    for folder in fs::read_dir(&root)?.flatten() {
        let mut kept = kept_in(&folder.path());
        expire(&mut kept, settings);
        if kept.is_empty() {
            let _ = fs::remove_dir(folder.path());
        }
        all.extend(kept);
    }
    all.sort_by_key(|(version, _)| version.id);
    let mut total: u64 = all.iter().map(|(version, _)| version.size).sum();
    let allowed = u64::from(settings.megabytes) * 1_000_000;
    for (version, file) in all {
        if total <= allowed {
            break;
        }
        if fs::remove_file(&file).is_ok() {
            total -= version.size;
            if let Some(folder) = file.parent() {
                let _ = fs::remove_dir(folder);
            }
        }
    }
    Ok(())
}
