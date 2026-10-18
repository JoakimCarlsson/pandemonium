//! The extensions installed on this machine: where they live and how they are read.

use std::path::{Component, Path, PathBuf};

use crate::manifest::Manifest;

/// An extension folder that was read: its name, where it is and what its manifest says.
#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    /// Its folder's name, which requests name it by.
    pub id: String,
    /// Where it is.
    pub folder: PathBuf,
    /// What its manifest says.
    pub manifest: Manifest,
}

/// The folder installed extensions are kept in.
pub(crate) fn root() -> Result<PathBuf, String> {
    pm_store::data_directory()
        .map(|data| data.join("extensions"))
        .ok_or_else(|| "this machine has no data folder".to_owned())
}

/// Whether `id` can name an extension's folder.
pub(crate) fn is_name(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\'])
}

/// The path `relative` leads to inside `folder`, or why it does not stay inside.
pub(crate) fn inside(folder: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!("{relative} does not stay inside the extension"));
    }
    Ok(folder.join(path))
}

/// The name of the language folder `folder` of the extension `extension` in the language index.
pub fn language_id(extension: &str, folder: &str) -> String {
    let last = folder.rsplit(['/', '\\']).next().unwrap_or(folder);
    format!("{extension}-{last}")
}

impl Package {
    /// The extension in `folder`, named by the folder.
    pub fn read(folder: &Path) -> Result<Self, String> {
        let id = folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| is_name(name))
            .ok_or_else(|| format!("{} is not a folder to install", folder.display()))?;
        let text = std::fs::read_to_string(folder.join(crate::MANIFEST))
            .map_err(|error| format!("{} has no {}: {error}", folder.display(), crate::MANIFEST))?;
        Ok(Self {
            id,
            folder: folder.to_path_buf(),
            manifest: Manifest::parse(&text)?,
        })
    }

    /// The names of the languages it adds in the language index, with the folder each is read from inside the extension.
    pub fn languages(&self) -> Vec<(String, PathBuf)> {
        self.manifest
            .languages
            .iter()
            .filter_map(|entry| {
                let folder = inside(&self.folder, &entry.folder).ok()?;
                Some((language_id(&self.id, &entry.folder), folder))
            })
            .collect()
    }

    /// What its files are written to under `relative`, when that stays inside it.
    pub(crate) fn file(&self, relative: &str) -> Result<PathBuf, String> {
        inside(&self.folder, relative)
    }
}

/// The extensions installed on this machine, by name; a folder that does not read as one is left out.
pub fn installed() -> Vec<Package> {
    let Ok(root) = root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<Package> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| Package::read(&entry.path()).ok())
        .collect();
    found.sort_by(|left, right| left.id.cmp(&right.id));
    found
}

/// The installed extension called `id`.
pub fn named(id: &str) -> Option<Package> {
    if !is_name(id) {
        return None;
    }
    Package::read(&root().ok()?.join(id)).ok()
}
