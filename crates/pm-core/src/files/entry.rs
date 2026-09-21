//! One name in a worktree, and the row the window draws for it.

use std::path::{Path, PathBuf};

/// An entry's identity for as long as the tree holds it.
///
/// Handed out by the tree, stable across expanding and collapsing, and the
/// only thing a click has to carry back.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryId(pub(super) u64);

/// One name in a worktree.
#[derive(Clone, Debug)]
pub struct Entry {
    /// What this entry is called for as long as the tree holds it.
    pub(super) id: EntryId,
    /// Where the entry is on disk.
    pub(super) path: PathBuf,
    /// The entry's own name, without its parent.
    pub(super) name: String,
    /// Whether the entry holds other entries.
    pub(super) directory: bool,
}

impl Entry {
    /// The entry `path` stands for, named `id`.
    pub(super) fn at(id: EntryId, path: PathBuf, directory: bool) -> Self {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );

        Self {
            id,
            path,
            name,
            directory,
        }
    }

    /// What this entry is called for as long as the tree holds it.
    pub fn id(&self) -> EntryId {
        self.id
    }

    /// Where the entry is on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The entry's own name, without its parent.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the entry holds other entries.
    pub fn is_directory(&self) -> bool {
        self.directory
    }
}

/// One visible line of the tree, as the window draws it.
#[derive(Clone, Debug)]
pub struct Row<'a> {
    /// The entry on this line.
    pub entry: &'a Entry,
    /// How many directories deep the entry sits below the root.
    pub depth: usize,
    /// Whether this directory is showing what it holds.
    pub expanded: bool,
}
