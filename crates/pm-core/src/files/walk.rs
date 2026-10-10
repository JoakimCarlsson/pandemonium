//! Listing project files through their machine's walk operation.

use pm_host::Location;
use std::path::PathBuf;

/// Every file under the location, using the machine's ignore rules.
pub fn walk(root: impl Into<Location>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    walk_each(root, |path| {
        paths.push(path);
        true
    });
    paths
}

/// Streams files from a walk performed on the machine holding the root.
pub fn walk_each(root: impl Into<Location>, found: impl FnMut(PathBuf) -> bool) {
    let root = root.into();
    root.host.walk(&root.path, found);
}
