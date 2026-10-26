//! The list of languages there are: a `languages.json` beside their folders, read from the repository at the tag this build was made from.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::fetch::{Fetched, fetch_tagged, sha256};
use crate::manifest::Manifest;
use crate::paths::{languages_root, local_root};

/// The repository the languages are read from, at a tag put after it.
const REPOSITORY: &str = "https://raw.githubusercontent.com/JoakimCarlsson/pandemonium";

/// The variable that points the server at another place to read the languages from: an address, or a folder on this machine.
const OVERRIDE: &str = "PANDEMONIUM_LANGUAGES";

/// The files a language folder holds, listed first when they are there; a folder may hold more, such as the lockfile that pins an npm install or, in a folder of your own, the grammar itself.
const FILES: [&str; 7] = [
    "config.toml",
    "highlights.scm",
    "indents.scm",
    "outline.scm",
    "injections.scm",
    "brackets.scm",
    "runnables.scm",
];

/// The names of the files in `folder`, the usual ones of a language folder first, those starting with a dot left out.
fn folder_files(folder: &Path) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = FILES
        .iter()
        .filter(|name| folder.join(name).is_file())
        .map(|name| (*name).to_owned())
        .collect();
    let mut more: Vec<String> = std::fs::read_dir(folder)
        .map_err(|error| format!("could not read {}: {error}", folder.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.') && !FILES.contains(&name.as_str()))
        .collect();
    more.sort();
    names.extend(more);
    Ok(names)
}

/// One language the index lists.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    /// Its folder's name.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// What kind of extension it is.
    #[serde(default)]
    pub kind: String,
    /// Its version.
    #[serde(default)]
    pub version: String,
    /// What it is for, in a line.
    #[serde(default)]
    pub description: String,
    /// Whether it is installed only as part of another.
    #[serde(default)]
    pub hidden: bool,
    /// The languages installed with it.
    #[serde(default)]
    pub requires: Vec<String>,
    /// The endings of the file names written in it.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// Whole file names that are written in it.
    #[serde(default)]
    pub names: Vec<String>,
    /// File names written in it, where `*` stands for any run of characters.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// The program of its language server, when it has one.
    #[serde(default)]
    pub server: Option<String>,
    /// The SHA-256 of each file of its folder, by file name.
    pub files: BTreeMap<String, String>,
    /// Whether it is a folder of your own in the local languages folder rather than one the list offers.
    #[serde(default)]
    pub local: bool,
}

/// Every language there is.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Index {
    /// The version of the format.
    pub version: u32,
    /// The languages, by name.
    pub languages: Vec<Listing>,
}

/// The tag of the repository this build reads the languages from: this build's own release.
fn tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// The `languages` folder of the checkout a debug build was built from, which it reads the languages from so that changes to them are seen before they are tagged.
fn checkout() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../languages");
    folder
        .join("languages.json")
        .is_file()
        .then(|| folder.to_string_lossy().into_owned())
}

/// Where the languages are read from: the folder or address `PANDEMONIUM_LANGUAGES` names, the checkout a debug build was built from, or the repository at this build's tag.
pub(crate) fn source() -> String {
    match std::env::var(OVERRIDE) {
        Ok(place) if !place.trim().is_empty() => place.trim().trim_end_matches('/').to_owned(),
        _ => checkout().unwrap_or_else(|| format!("{REPOSITORY}/{}/languages", tag())),
    }
}

impl Index {
    /// The index there is now, fetched again only when it changed since the copy kept, and that copy when it cannot be fetched.
    pub fn load() -> Result<Self, String> {
        let root = languages_root()?;
        let (kept, kept_tag) = (root.join("index.json"), root.join("index.etag"));
        let place = source();
        let etag = std::fs::read_to_string(&kept_tag).ok();
        let held = std::fs::read(&kept).ok();
        let wanted = etag.as_deref().filter(|_| held.is_some());
        let bytes = match fetch_tagged(&format!("{place}/languages.json"), wanted) {
            Ok(Fetched::Fresh(bytes, tag)) => {
                let _ = std::fs::create_dir_all(&root);
                let _ = std::fs::write(&kept, &bytes);
                match tag {
                    Some(tag) => {
                        let _ = std::fs::write(&kept_tag, tag);
                    }
                    None => {
                        let _ = std::fs::remove_file(&kept_tag);
                    }
                }
                bytes
            }
            Ok(Fetched::Unchanged) => held.ok_or("the kept index is gone")?,
            Err(reason) => held.ok_or(reason)?,
        };
        serde_json::from_slice(&bytes).map_err(|error| format!("languages.json: {error}"))
    }

    /// The list with the folders of your own in the local languages folder added, each in place of a language of the list with the same name.
    pub fn with_local(mut self) -> Self {
        for listing in local_listings() {
            self.languages.retain(|held| held.id != listing.id);
            self.languages.push(listing);
        }
        self
    }

    /// The list there is now with the folders of your own added; when the list cannot be read, only those, unless there are none.
    pub fn load_with_local() -> Result<Self, String> {
        match Self::load() {
            Ok(index) => Ok(index.with_local()),
            Err(reason) => {
                let local = Self::default().with_local();
                if local.languages.is_empty() {
                    Err(reason)
                } else {
                    Ok(local)
                }
            }
        }
    }

    /// The language whose folder is `id`.
    pub fn find(&self, id: &str) -> Option<&Listing> {
        self.languages.iter().find(|listing| listing.id == id)
    }
}

/// The listing of the language folder `folder`, with the checksum of every file in it.
fn listing_of(folder: &Path) -> Result<Listing, String> {
    let id = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let read = |name: &str| {
        std::fs::read(folder.join(name)).map_err(|error| format!("{id}/{name}: {error}"))
    };
    let manifest = Manifest::parse(&String::from_utf8_lossy(&read("config.toml")?))
        .map_err(|reason| format!("{id}/{reason}"))?;
    let mut files = BTreeMap::new();
    for name in folder_files(folder)? {
        files.insert(name.clone(), sha256(&read(&name)?));
    }
    Ok(Listing {
        id,
        name: manifest.name,
        kind: manifest.kind,
        version: manifest.version,
        description: manifest.description,
        hidden: manifest.hidden,
        requires: manifest.requires,
        suffixes: manifest.suffixes,
        names: manifest.names,
        patterns: manifest.patterns,
        server: manifest.server.map(|server| server.command),
        files,
        local: false,
    })
}

/// The folders of your own in the local languages folder, each listed as a language; a folder that cannot be read is left out.
fn local_listings() -> Vec<Listing> {
    let Ok(root) = local_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<Listing> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("config.toml").is_file())
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|entry| listing_of(&entry.path()).ok())
        .map(|listing| Listing {
            local: true,
            ..listing
        })
        .collect();
    found.sort_by(|left, right| left.id.cmp(&right.id));
    found
}

/// Writes `languages.json` for the language folders inside `folder`, with the checksum of every file of each, and returns how many it lists.
pub fn build_index(folder: &Path) -> Result<usize, String> {
    let mut entries: Vec<_> = std::fs::read_dir(folder)
        .map_err(|error| format!("could not read {}: {error}", folder.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("config.toml").is_file())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    let languages = entries
        .iter()
        .map(|entry| listing_of(&entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    let count = languages.len();
    let index = Index {
        version: 1,
        languages,
    };
    let mut text = serde_json::to_string_pretty(&index).map_err(|error| error.to_string())?;
    text.push('\n');
    std::fs::write(folder.join("languages.json"), text)
        .map_err(|error| format!("could not write languages.json: {error}"))?;
    Ok(count)
}
