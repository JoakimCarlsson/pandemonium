//! Native conversation storage shared by accounts without sharing authentication.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::account_setup::{link, write};

/// Marks a conversation store whose rate-limit records can belong to different accounts.
const SHARED: &str = ".pandemonium-shared-history";

/// Makes existing and future native conversations available from every account home.
pub(super) fn prepare(source: &Path, selected: &Path, agent: &str) -> io::Result<()> {
    let entries: &[&str] = match agent {
        "claude-code" => &["projects", "file-history"],
        "codex" => &["sessions", "archived_sessions"],
        "grok" => &["sessions"],
        _ => return Ok(()),
    };
    let parent = selected
        .parent()
        .ok_or_else(|| io::Error::other("Missing account parent"))?;
    let profiles = fs::read_dir(parent)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(super::accounts::safe_component)
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    for entry in entries {
        let shared = source.join(entry);
        fs::create_dir_all(&shared)?;
        let shared = shared.canonicalize()?;
        for profile in &profiles {
            let local = profile.join(entry);
            if local.try_exists()? && local.canonicalize()? != shared {
                merge(&local, &shared)?;
            }
        }
        write(
            &shared.join(SHARED),
            b"Shared native conversations; authentication remains account-local.\n",
        )?;
        for profile in &profiles {
            attach(&shared, profile, entry)?;
        }
    }
    Ok(())
}

/// Adds missing native records without overwriting an existing conversation.
fn merge(source: &Path, destination: &Path) -> io::Result<()> {
    merge_directory(source, destination, &mut BTreeSet::new())
}

/// Merges native directories while rejecting symbolic links that form a cycle.
fn merge_directory(
    source: &Path,
    destination: &Path,
    visited: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    let canonical = source.canonicalize()?;
    if canonical == destination.canonicalize()? {
        return Ok(());
    }
    if !visited.insert(canonical.clone()) {
        return Err(io::Error::other(format!(
            "Cyclic conversation directory: {}",
            source.display()
        )));
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if entry.file_name() == SHARED {
            continue;
        }
        let metadata = fs::metadata(&from)?;
        if metadata.is_dir() {
            fs::create_dir_all(&to)?;
            merge_directory(&from, &to, visited)?;
        } else if metadata.is_file() {
            if to.try_exists()? {
                if from.canonicalize()? != to.canonicalize()?
                    && !identical(&from, &to)?
                    && conversation(&from)
                {
                    return Err(io::Error::other(format!(
                        "Conflicting conversation records retained at {} and {}",
                        from.display(),
                        to.display()
                    )));
                }
            } else if fs::hard_link(&from, &to).is_err() {
                fs::copy(&from, &to)?;
            }
        }
    }
    visited.remove(&canonical);
    Ok(())
}

/// Whether a conflicting file contains a native conversation rather than disposable indexing data.
fn conversation(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "jsonl" || extension == "json")
        && path
            .file_name()
            .is_none_or(|name| name != "sessions-index.json")
}

/// Compares native records in bounded memory without loading transcripts into the editor.
fn identical(left: &Path, right: &Path) -> io::Result<bool> {
    if fs::metadata(left)?.len() != fs::metadata(right)?.len() {
        return Ok(false);
    }
    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    let mut left_bytes = [0; 16 * 1024];
    let mut right_bytes = [0; 16 * 1024];
    loop {
        let count = left.read(&mut left_bytes)?;
        if count == 0 {
            return Ok(true);
        }
        right.read_exact(&mut right_bytes[..count])?;
        if left_bytes[..count] != right_bytes[..count] {
            return Ok(false);
        }
    }
}

/// Points a profile at shared storage while retaining its original records in that profile.
fn attach(shared: &Path, profile: &Path, entry: &str) -> io::Result<()> {
    let local = profile.join(entry);
    if local.try_exists()? && local.canonicalize()? == shared {
        return Ok(());
    }
    let temporary = profile.join(format!(".pandemonium-shared-{entry}"));
    let linked = match fs::symlink_metadata(&temporary) {
        Ok(_) if fs::read_link(&temporary).ok().as_deref() == Some(shared) => true,
        Ok(_) => {
            return Err(io::Error::other(format!(
                "Unfinished history migration at {}",
                temporary.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            link(shared, &temporary, true).is_ok()
        }
        Err(error) => return Err(error),
    };
    if !linked {
        fs::create_dir_all(&local)?;
        return merge(shared, &local);
    }
    let backup = backup(profile, entry)?;
    let existed = fs::symlink_metadata(&local).is_ok();
    if existed && let Err(error) = fs::rename(&local, &backup) {
        let _ = super::account_setup::unlink(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, &local) {
        if existed {
            let _ = fs::rename(&backup, &local);
        }
        let _ = super::account_setup::unlink(&temporary);
        return Err(error);
    }
    Ok(())
}

/// Reserves a backup name without replacing a previous migration's original records.
fn backup(profile: &Path, entry: &str) -> io::Result<PathBuf> {
    let directory = profile.join(".pandemonium-history-originals");
    fs::create_dir_all(&directory)?;
    for count in 0.. {
        let name = if count == 0 {
            entry.to_owned()
        } else {
            format!("{entry}-{count}")
        };
        let path = directory.join(name);
        if fs::symlink_metadata(&path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound) {
            return Ok(path);
        }
    }
    Err(io::Error::other("No history backup name available"))
}
