//! Catalogue metadata and atomic extension installation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::super::paths;

/// The maintained index; its entries pin packages independently of editor builds.
const CATALOGUE_URL: &str =
    "https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/extensions/catalogue.yaml";

/// An extension offered by the catalogue or installed locally.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    /// Stable directory identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Semver package version.
    pub version: String,
    /// Publisher name.
    pub publisher: String,
    /// Source repository URL.
    pub source: String,
    /// What this extension provides.
    pub description: String,
    /// Platforms supported by its servers.
    #[serde(default)]
    pub platforms: Vec<String>,
    /// External tools required by its install recipes.
    #[serde(default)]
    pub prerequisites: Vec<String>,
    /// HTTPS ZIP package location.
    #[serde(default)]
    pub url: String,
    /// SHA-256 of the ZIP package.
    #[serde(default)]
    pub sha256: String,
}

/// Fetches a versioned catalogue from its configured HTTPS source.
///
/// A source that answers 404 has published nothing yet, which is an empty catalogue.
pub fn catalogue() -> Result<Vec<Entry>, String> {
    let url =
        std::env::var("PANDEMONIUM_LANGUAGE_CATALOGUE").unwrap_or_else(|_| CATALOGUE_URL.into());
    let bytes = match pm_text::install::download(&url) {
        Err(error) if error.ends_with("404") => return Ok(Vec::new()),
        other => other?,
    };
    let entries: Vec<Entry> =
        serde_norway::from_slice(&bytes).map_err(|error| error.to_string())?;
    let mut ids = std::collections::HashSet::new();
    for entry in &entries {
        identifier(&entry.id)?;
        semver::Version::parse(&entry.version).map_err(|error| error.to_string())?;
        if !ids.insert(&entry.id)
            || !entry.url.starts_with("https://")
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            || entry.publisher.trim().is_empty()
            || entry.source.trim().is_empty()
        {
            return Err(format!("Invalid catalogue entry: {}", entry.id));
        }
    }
    Ok(entries)
}

/// Installs a checksum-verified package through the local extension validation path.
pub fn install(entry: &Entry) -> Result<(), String> {
    identifier(&entry.id)?;
    if !entry.platforms.is_empty() && !entry.platforms.contains(&pm_text::install::platform()) {
        return Err(format!(
            "{} has no server build for {}.",
            entry.name,
            pm_text::install::platform()
        ));
    }
    transaction(&entry.id, |partial| {
        let bytes = pm_text::install::download_checked(&entry.url, &entry.sha256)?;
        pm_text::install::unpack_zip(&bytes, partial)?;
        let metadata = super::loader::validate(partial, &entry.id)?;
        if metadata.version != entry.version
            || metadata.name != entry.name
            || metadata.publisher != entry.publisher
            || metadata.source != entry.source
        {
            return Err("Package metadata differs from its catalogue entry.".into());
        }
        Ok(())
    })
}

/// An extension imported from a folder, remembered so it can be installed
/// again from there once it has been removed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Imported {
    /// What the extension's manifest says about it.
    pub entry: Entry,
    /// The folder it was imported from.
    pub path: PathBuf,
}

/// The file the imported extensions are remembered in.
fn imported_file() -> Option<PathBuf> {
    paths::extensions().map(|root| root.join(".imported.yaml"))
}

/// The extensions imported from a folder, installed or not.
pub fn imported() -> Vec<Imported> {
    imported_file()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_norway::from_slice::<Vec<Imported>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|mut known| {
            known.entry.id = known.path.file_name()?.to_str()?.to_owned();
            Some(known)
        })
        .collect()
}

/// Remembers `imported`, replacing what was remembered under the same id.
fn remember(imported: Imported) {
    let Some(file) = imported_file() else {
        return;
    };
    let mut known = self::imported();
    known.retain(|other| other.entry.id != imported.entry.id);
    known.push(imported);
    if let Ok(text) = serde_norway::to_string(&known) {
        let _ = fs::write(file, text);
    }
}

/// Imports an existing directory using the same validation and activation as a download.
pub fn import(source: &Path) -> Result<(), String> {
    let id = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Choose an extension directory.")?;
    identifier(id)?;
    let mut entry = None;
    transaction(id, |partial| {
        copy_directory(source, partial)?;
        entry = Some(super::loader::validate(partial, id)?);
        Ok(())
    })?;
    if let Some(mut entry) = entry {
        entry.id = id.to_owned();
        remember(Imported {
            entry,
            path: source.to_path_buf(),
        });
    }
    Ok(())
}

/// Removes a user extension while leaving managed server binaries and user preferences intact.
pub fn remove(id: &str) -> Result<(), String> {
    identifier(id)?;
    let root = paths::extensions().ok_or("An editor home directory is required.")?;
    fs::remove_dir_all(root.join(id)).map_err(|error| error.to_string())
}

/// Validates an extension identifier before using it as a directory name.
fn identifier(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("Extension ids contain only letters, numbers, hyphens and underscores.".into());
    }
    Ok(())
}

/// Publishes a fully validated candidate, restoring the old directory on failure.
fn transaction(id: &str, prepare: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let root = paths::extensions().ok_or("An editor home directory is required.")?;
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let partial = root.join(format!(".{id}.partial"));
    let backup = root.join(format!(".{id}.backup"));
    let target = root.join(id);
    if backup.exists() && !target.exists() {
        fs::rename(&backup, &target).map_err(|error| error.to_string())?;
    }
    if partial.exists() {
        fs::remove_dir_all(&partial).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&partial).map_err(|error| error.to_string())?;
    let result = prepare(&partial).and_then(|()| {
        if target.exists() {
            if backup.exists() {
                fs::remove_dir_all(&backup).map_err(|error| error.to_string())?;
            }
            fs::rename(&target, &backup).map_err(|error| error.to_string())?;
        }
        if let Err(error) = fs::rename(&partial, &target) {
            if backup.exists() {
                let _ = fs::rename(&backup, &target);
            }
            return Err(error.to_string());
        }
        if backup.exists() {
            let _ = fs::remove_dir_all(&backup);
        }
        Ok(())
    });
    if partial.exists() {
        let _ = fs::remove_dir_all(&partial);
    }
    result
}

/// Copies local assets without following symbolic links or accepting nested links.
fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if kind.is_symlink() {
            return Err("Local extensions cannot contain symbolic links.".into());
        }
        if kind.is_dir() {
            fs::create_dir(&target).map_err(|error| error.to_string())?;
            copy_directory(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Restores interrupted transactions before the extension registry is loaded at launch.
pub(crate) fn recover() {
    let Some(root) = paths::extensions() else {
        return;
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(id) = name
            .strip_prefix('.')
            .and_then(|name| name.strip_suffix(".backup"))
        {
            if identifier(id).is_err() {
                continue;
            }
            if root.join(id).exists() {
                let _ = fs::remove_dir_all(entry.path());
            } else {
                let _ = fs::rename(entry.path(), root.join(id));
            }
        }
        if name.starts_with('.') && name.ends_with(".partial") {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}
