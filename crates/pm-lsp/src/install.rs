//! Installing a language: its folder, checked file by file against the index, its grammar, checked against its manifest, and its language server.

use std::collections::BTreeSet;
use std::path::Path;

use crate::fetch::{fetch_checked, sha256};
use crate::index::{Index, source};
use crate::manifest::Manifest;
use crate::paths::{grammar_file, language_folder, languages_root, local_root};
use crate::resolve::ensure_server;

/// The manifest of the installed language `id`, or nothing when it is not installed.
pub fn installed_manifest(id: &str) -> Option<Manifest> {
    let folder = language_folder(id).ok()?;
    if !grammar_file(id).ok()?.is_file() {
        return None;
    }
    Manifest::read(&folder).ok()
}

/// The installed language a file at `path` is written in, by folder name.
pub fn language_of(path: &str) -> Option<String> {
    installed()
        .into_iter()
        .find(|(_, manifest)| manifest.matches(path))
        .map(|(id, _)| id)
}

/// The languages installed on this machine, by folder name, each with its manifest.
pub fn installed() -> Vec<(String, Manifest)> {
    let Ok(root) = languages_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(String, Manifest)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            let manifest = installed_manifest(&id)?;
            Some((id, manifest))
        })
        .collect();
    found.sort_by(|left, right| left.0.cmp(&right.0));
    found
}

/// The language `id` and every language it needs, the needed ones first, each once.
fn with_requirements(index: &Index, id: &str) -> Result<Vec<String>, String> {
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = vec![(id.to_owned(), false)];
    while let Some((id, expanded)) = stack.pop() {
        if expanded {
            order.push(id);
            continue;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        let listing = index
            .find(&id)
            .ok_or_else(|| format!("no language is called {id}"))?;
        stack.push((id.clone(), true));
        for needed in listing.requires.iter().rev() {
            stack.push((needed.clone(), false));
        }
    }
    Ok(order)
}

/// Fetches the folder of the language `id` as `index` lists it into `staging`, every file checked, and its grammar, and returns its manifest.
fn fetch_folder(index: &Index, id: &str, staging: &Path) -> Result<Manifest, String> {
    let listing = index
        .find(id)
        .ok_or_else(|| format!("no language is called {id}"))?;
    let place = if listing.local {
        local_root()?.to_string_lossy().into_owned()
    } else {
        source()
    };
    std::fs::create_dir_all(staging).map_err(|error| error.to_string())?;
    for (name, checksum) in &listing.files {
        if name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(format!("{id} lists a file outside its folder: {name}"));
        }
        let bytes = fetch_checked(&format!("{place}/{id}/{name}"), checksum)?;
        std::fs::write(staging.join(name), bytes).map_err(|error| error.to_string())?;
    }
    let manifest = Manifest::parse(
        &std::fs::read_to_string(staging.join("config.toml"))
            .map_err(|error| format!("{id} has no config.toml: {error}"))?,
    )?;
    if staging.join("grammar.wasm").is_file() {
        return Ok(manifest);
    }
    if manifest.grammar_file.url.is_empty() {
        return Err(format!(
            "{id} has no grammar: give it a grammar_file or put grammar.wasm in its folder"
        ));
    }
    let grammar = fetch_checked(&manifest.grammar_file.url, &manifest.grammar_file.sha256)?;
    std::fs::write(staging.join("grammar.wasm"), grammar).map_err(|error| error.to_string())?;
    Ok(manifest)
}

/// Whether the installed language `id` already holds exactly the files `index` lists for it.
fn is_current(index: &Index, id: &str) -> bool {
    let (Some(listing), Ok(folder)) = (index.find(id), language_folder(id)) else {
        return false;
    };
    folder.join("grammar.wasm").is_file()
        && listing.files.iter().all(|(name, checksum)| {
            std::fs::read(folder.join(name)).is_ok_and(|bytes| sha256(&bytes) == *checksum)
        })
}

/// Installs the language `id` and the languages it needs, as the index lists them, then their language servers, telling `progress` what it is doing; returns why a language server could not be installed, if one could not, while the languages themselves are.
pub fn install(id: &str, progress: &dyn Fn(&str, &str)) -> Result<Vec<String>, String> {
    progress(id, "Reading the list of languages");
    let index = Index::load_with_local()?;
    let ids = with_requirements(&index, id)?;
    let root = languages_root()?;
    let mut manifests = Vec::new();
    for id in &ids {
        if is_current(&index, id)
            && let Some(manifest) = installed_manifest(id)
        {
            manifests.push((id.clone(), manifest));
            continue;
        }
        progress(id, "Downloading the grammar");
        let staging = root.join(format!(".{id}.partial"));
        let _ = std::fs::remove_dir_all(&staging);
        if let Err(reason) = fetch_folder(&index, id, &staging) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(reason);
        }
        let folder = language_folder(id)?;
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::rename(&staging, &folder).map_err(|error| error.to_string())?;
        manifests.push((id.clone(), Manifest::read(&folder)?));
    }
    let mut problems = Vec::new();
    for (id, manifest) in manifests {
        let Some(server) = manifest.server else {
            continue;
        };
        progress(&id, "Installing the language server");
        if let Err(reason) = ensure_server(&server, None) {
            problems.push(format!("{}: {reason}", manifest.name));
        }
    }
    Ok(problems)
}

/// Copies the extension in the folder `source` into the folder of extensions of your own, under the folder's name and in place of one of the same name, and returns that name; the folder is a language when it holds a `config.toml` that reads as one.
pub fn add_extension(source: &Path) -> Result<String, String> {
    let id = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .ok_or_else(|| format!("{} is not a folder to install", source.display()))?;
    Manifest::read(source)?;
    let root = local_root()?;
    let staging = root.join(format!(".{id}.partial"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let entries = std::fs::read_dir(source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        if entry.path().is_file() && !name.to_string_lossy().starts_with('.') {
            std::fs::copy(entry.path(), staging.join(&name))
                .map_err(|error| format!("could not copy {}: {error}", entry.path().display()))?;
        }
    }
    let folder = root.join(&id);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::rename(&staging, &folder).map_err(|error| error.to_string())?;
    Ok(id)
}

/// Takes the language `id` off this machine; its language server stays, for other languages that use it.
pub fn remove(id: &str) -> Result<(), String> {
    let folder = language_folder(id)?;
    if !folder.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&folder).map_err(|error| format!("could not remove {id}: {error}"))
}
