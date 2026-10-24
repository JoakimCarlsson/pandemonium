//! Folders on the machine: looking inside one and cloning a repository into one, for adding a project.

use std::fs;
use std::path::{Path, PathBuf};

use crate::run;

/// The suffix a repository's address carries that its folder does not.
const GIT_SUFFIX: &str = ".git";

/// The folders, in the order they are looked for, that repositories are cloned into.
const CLONE_FOLDERS: [&str; 4] = ["Documents/repos", "repos", "code", "projects"];

/// The reader's home folder.
fn home() -> Result<PathBuf, String> {
    std::env::home_dir().ok_or_else(|| "this machine has no home folder".to_owned())
}

/// What the repository at `url` is called, which is the folder it is cloned into.
fn named(url: &str) -> Option<String> {
    let name = url
        .trim()
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()?
        .trim_end_matches(GIT_SUFFIX);
    (!name.is_empty()).then(|| name.to_owned())
}

/// The folder clones are put in unless another is asked for: the first of the usual ones that exists, or `projects`.
fn clone_folder() -> Result<PathBuf, String> {
    let home = home()?;
    Ok(CLONE_FOLDERS
        .iter()
        .map(|folder| home.join(folder))
        .find(|folder| folder.is_dir())
        .unwrap_or_else(|| home.join("projects")))
}

/// Clones the repository at `url` into a folder of its own named after it, under `under` or the usual folder for clones, made when it is not there, with its submodules, and returns that folder.
///
/// A name that is already taken is refused rather than merged into, and an address that git would
/// read as an option is not a repository at all.
pub fn clone(url: &str, under: Option<&Path>) -> Result<PathBuf, String> {
    let url = url.trim();
    let name = named(url).ok_or_else(|| format!("{url} does not name a repository"))?;
    if url.starts_with('-') || name.starts_with('-') {
        return Err(format!("{url} is an option, not a repository"));
    }
    let under = match under {
        Some(under) => under.to_path_buf(),
        None => clone_folder()?,
    };
    fs::create_dir_all(&under).map_err(|error| error.to_string())?;
    let root = under.join(&name);
    if root.exists() {
        return Err(format!("{} is already there", root.display()));
    }
    run(&under, &["clone", "--recurse-submodules", "--", url, &name])?;
    Ok(root)
}

/// The folder `path` names, with a leading `~` taken to be the home folder and nothing at all the home folder itself.
fn resolved(path: &str) -> Result<PathBuf, String> {
    let path = path.trim();
    match (path, path.strip_prefix("~/")) {
        ("" | "~", _) => home(),
        (_, Some(rest)) => Ok(home()?.join(rest)),
        _ => Ok(PathBuf::from(path)),
    }
}

/// The folder `path` names, as an absolute path, and the names of the folders inside it, hidden ones left out, in order.
pub fn folders(path: &str) -> Result<(PathBuf, Vec<String>), String> {
    let folder = fs::canonicalize(resolved(path)?).map_err(|error| error.to_string())?;
    let mut names: Vec<String> = fs::read_dir(&folder)
        .map_err(|error| error.to_string())?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    Ok((folder, names))
}
