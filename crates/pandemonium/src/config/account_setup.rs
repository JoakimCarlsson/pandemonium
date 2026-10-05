//! Shared global setup for isolated provider homes, leaving login and session state in place.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Write};
#[cfg(windows)]
use std::os::windows::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pm_acp::Agent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{account_settings, accounts, paths};

/// The inheritance record contains only shared preferences, link targets and copy fingerprints.
const RECORD: &str = ".pandemonium-shared-setup.json";

/// Claude's authored configuration and local marketplace installations, excluding synced account data.
const CLAUDE: &[&str] = &[
    "CLAUDE.md",
    "AGENTS.md",
    "rules",
    "skills",
    "commands",
    "agents",
    "hooks",
    "output-styles",
    "workflows",
    "plugins/cache",
    "plugins/marketplaces",
];

/// Codex's authored instructions and extensions, excluding authentication and session databases.
const CODEX: &[&str] = &[
    "AGENTS.md",
    "AGENTS.override.md",
    "skills",
    "rules",
    "agents",
    "plugins/cache",
    "plugins/marketplaces",
];

/// Grok's authored configuration, excluding cloud-managed policy, memory and account caches.
const GROK: &[&str] = &[
    "AGENTS.md",
    "GROK.md",
    "skills",
    "commands",
    "agents",
    "rules",
    "hooks",
    "plugins",
    "installed-plugins",
];

/// The last shared setup allows later launches to preserve edits made inside the profile.
#[derive(Default, Deserialize, Serialize)]
struct Inherited {
    /// Sanitized settings inherited on the previous launch.
    #[serde(default)]
    settings: Value,
    /// Local marketplace records inherited without account-synced installs.
    #[serde(default)]
    plugins: Value,
    /// Links created by the editor, so unrelated links are never replaced.
    #[serde(default)]
    links: BTreeMap<PathBuf, PathBuf>,
    /// Copies used when symbolic links are unavailable, tracked without retaining their content.
    #[serde(default)]
    copies: BTreeMap<PathBuf, u64>,
}

/// Refreshes a named account's shared setup before any ACP process starts or reconnects.
pub fn prepare(agent: Agent, environment: &[(String, String)]) -> io::Result<()> {
    let Some(variable) = accounts::variable(agent) else {
        return Ok(());
    };
    let Some((_, selected)) = environment.iter().rev().find(|(key, _)| key == variable) else {
        return Ok(());
    };
    let Some(accounts) = paths::home().map(|home| home.join("accounts")) else {
        return Ok(());
    };
    let directory = Path::new(selected).canonicalize()?;
    let parent = accounts.join(agent.id);
    let Ok(parent) = parent.canonicalize() else {
        return Ok(());
    };
    if directory.parent() != Some(parent.as_path()) {
        return Ok(());
    }
    let Some(source) = global_home(agent, variable) else {
        return Ok(());
    };
    let source = std::path::absolute(source)?;
    if source.canonicalize().is_ok_and(|source| {
        parent
            .parent()
            .is_some_and(|accounts| source.starts_with(accounts))
    }) {
        return Err(io::Error::other(
            "An account profile cannot supply another account's global setup",
        ));
    }
    let record = directory.join(RECORD);
    let mut inherited: Inherited = match fs::read(&record) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Inherited::default(),
        Err(error) => return Err(error),
    };
    let entries = match agent.id {
        "claude-code" => CLAUDE,
        "codex" => CODEX,
        _ => GROK,
    };
    validate(&inherited, entries)?;
    for entry in entries {
        share(
            &source.join(entry),
            &directory,
            Path::new(entry),
            &mut inherited,
            &mut BTreeSet::new(),
        )?;
    }
    remove_missing(&source, &directory, &mut inherited)?;
    inherited.settings =
        account_settings::refresh(&source, &directory, agent.id, &inherited.settings)?;
    if agent.id == "claude-code" {
        inherited.plugins = account_settings::plugins(&source, &directory, &inherited.plugins)?;
    }
    let bytes = serde_json::to_vec_pretty(&inherited).map_err(io::Error::other)?;
    write(&record, &bytes)
}

/// Resolves the provider home before the profile's launch environment replaces it.
fn global_home(agent: Agent, variable: &str) -> Option<PathBuf> {
    agent
        .env
        .iter()
        .rev()
        .find(|(key, _)| *key == variable)
        .map(|(_, value)| PathBuf::from(value))
        .or_else(|| {
            env::var_os(variable)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            Some(env::home_dir()?.join(match agent.id {
                "claude-code" => ".claude",
                "codex" => ".codex",
                _ => ".grok",
            }))
        })
}

/// Fills absent setup paths, preserving profile-authored files and directories.
fn share(
    source: &Path,
    directory: &Path,
    relative: &Path,
    inherited: &mut Inherited,
    visited: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    let metadata = match fs::metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let canonical = source.canonicalize()?;
    if canonical.starts_with(directory) {
        return Err(io::Error::other(
            "Shared setup cannot point into the account profile",
        ));
    }
    let target = directory.join(relative);
    if let Some(previous) = inherited.links.get(relative) {
        if fs::read_link(&target).ok().as_ref() == Some(previous) {
            if previous == source {
                return Ok(());
            }
            unlink(&target)?;
        }
        inherited.links.remove(relative);
    }
    if let Ok(local) = fs::symlink_metadata(&target) {
        if local.is_symlink() {
            return Ok(());
        }
        if !metadata.is_dir() {
            if inherited.copies.contains_key(relative) {
                copy(source, &target, relative, inherited)?;
            }
            return Ok(());
        }
        if !local.is_dir() {
            return Ok(());
        }
    } else {
        fs::create_dir_all(
            target
                .parent()
                .ok_or_else(|| io::Error::other("Missing setup parent"))?,
        )?;
        if link(source, &target, metadata.is_dir()).is_ok() {
            inherited
                .links
                .insert(relative.to_path_buf(), source.to_path_buf());
            return Ok(());
        }
        if !metadata.is_dir() {
            return copy(source, &target, relative, inherited);
        }
        fs::create_dir_all(&target)?;
    }
    if !visited.insert(canonical.clone()) {
        return Ok(());
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        share(
            &entry.path(),
            directory,
            &relative.join(entry.file_name()),
            inherited,
            visited,
        )?;
    }
    visited.remove(&canonical);
    Ok(())
}

/// Copies shared files only while their profile copy remains unedited.
fn copy(
    source: &Path,
    target: &Path,
    relative: &Path,
    inherited: &mut Inherited,
) -> io::Result<()> {
    let local = match fs::read(target) {
        Ok(bytes) => Some(fingerprint(&bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if local.is_some() && local != inherited.copies.get(relative).copied() {
        inherited.copies.remove(relative);
        return Ok(());
    }
    let bytes = fs::read(source)?;
    let digest = fingerprint(&bytes);
    if local != Some(digest) {
        write(target, &bytes)?;
    }
    inherited.copies.insert(relative.to_path_buf(), digest);
    Ok(())
}

/// Removes editor-created links whose global file disappeared, without touching local overrides.
fn remove_missing(source: &Path, directory: &Path, inherited: &mut Inherited) -> io::Result<()> {
    let links = inherited.links.clone();
    for (relative, source) in links {
        if source.try_exists()? {
            continue;
        }
        let target = directory.join(&relative);
        if fs::read_link(&target).ok().as_ref() == Some(&source) {
            unlink(&target)?;
        }
        inherited.links.remove(&relative);
    }
    let copies = inherited.copies.clone();
    for (relative, previous) in copies {
        if source.join(&relative).try_exists()? {
            continue;
        }
        let target = directory.join(&relative);
        if fs::read(&target).is_ok_and(|bytes| fingerprint(&bytes) == previous) {
            fs::remove_file(target)?;
        }
        inherited.copies.remove(&relative);
    }
    Ok(())
}

/// Fingerprints copied setup so provider or user edits become profile overrides.
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = DefaultHasher::new();
    bytes.hash(&mut hash);
    hash.finish()
}

/// Atomically replaces an editor-owned setup file with private permissions.
pub(super) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing setup parent"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temporary = parent.join(format!(".pandemonium-setup-{}-{stamp}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// Links shared setup directly on Unix.
#[cfg(unix)]
fn link(source: &Path, target: &Path, _directory: bool) -> io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

/// Uses Windows links when available, allowing the caller to fall back to copying.
#[cfg(windows)]
fn link(source: &Path, target: &Path, directory: bool) -> io::Result<()> {
    if directory {
        std::os::windows::fs::symlink_dir(source, target)
    } else {
        std::os::windows::fs::symlink_file(source, target)
    }
}

/// Requests copy fallback on platforms without symbolic link support.
#[cfg(not(any(unix, windows)))]
fn link(_source: &Path, _target: &Path, _directory: bool) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Symbolic links unavailable",
    ))
}

/// Restricts recorded setup paths to the provider's explicitly shared assets.
fn validate(inherited: &Inherited, entries: &[&str]) -> io::Result<()> {
    for relative in inherited.links.keys().chain(inherited.copies.keys()) {
        if !relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
            || !entries.iter().any(|entry| relative.starts_with(entry))
        {
            return Err(io::Error::other("Invalid shared setup record path"));
        }
    }
    Ok(())
}

/// Removes an owned symbolic link without following its target.
fn unlink(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if fs::symlink_metadata(path)?.file_type().is_symlink_dir() {
        return fs::remove_dir(path);
    }
    fs::remove_file(path)
}
