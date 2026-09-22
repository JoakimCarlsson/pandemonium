//! Fetching a repository the window has not got a copy of yet.
//!
//! Every other question here is asked of a repository that is already on
//! disk. This is the one that makes one: a URL and a directory to put it
//! under, and what comes back is the checkout a project is then opened from.

use std::path::{Path, PathBuf};

use crate::git::Said;
use crate::git::run::git;

/// The suffix a repository URL carries that its directory does not.
const GIT_SUFFIX: &str = ".git";

/// Clones `url` into a directory of its own under `under`.
///
/// The directory is the repository's own name, which is what git would have
/// picked itself, and the clone is refused rather than merged into whatever
/// is already there: a name already taken is the reader's to resolve, and
/// cloning on top of it is not a resolution.
///
/// Neither the URL nor the name git is handed may begin with a dash, and
/// both come after `--`: a URL is a place to fetch from, and one written so
/// that git reads it as an option — `--upload-pack=…` names a program to run
/// — is not a repository at all.
pub fn clone(url: &str, under: &Path) -> Result<PathBuf, String> {
    let url = url.trim();
    let name = named(url).ok_or_else(|| format!("{url} does not name a repository"))?;
    if flag(url) || flag(&name) {
        return Err(format!("{url} is an option, not a repository"));
    }
    let root = under.join(&name);
    if root.exists() {
        return Err(format!("{} is already there", root.display()));
    }

    let said: Said = git(under, ["clone", "--", url, &name]);
    said.map(|_| root)
}

/// Whether `argument` would be read as an option rather than as itself.
fn flag(argument: &str) -> bool {
    argument.starts_with('-')
}

/// What the repository at `url` is called, which is what it is cloned into.
///
/// A URL is written a handful of ways — with a scheme, over ssh with a colon,
/// with or without the `.git` — and all of them end in the repository's own
/// name. That last part is the whole of what is needed.
pub fn named(url: &str) -> Option<String> {
    let name = url
        .trim()
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()?
        .trim_end_matches(GIT_SUFFIX);

    match name.is_empty() {
        true => None,
        false => Some(name.to_owned()),
    }
}
