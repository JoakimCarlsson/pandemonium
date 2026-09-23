//! The tree itself: what is read, what is expanded, and the lines that makes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::files::entry::{Entry, EntryId, Row};

/// The files of one worktree, and which of its directories are open.
#[derive(Debug)]
pub struct FileTree {
    /// The worktree this tree lists.
    root: PathBuf,
    /// Every entry read so far, by the directory that holds it.
    children: BTreeMap<PathBuf, Vec<Entry>>,
    /// The directories showing what they hold.
    expanded: Vec<PathBuf>,
    /// The id the next entry read will be given.
    next: EntryId,
}

impl FileTree {
    /// The tree of the worktree at `root`, with its top level read.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let mut tree = Self {
            root: root.into(),
            children: BTreeMap::new(),
            expanded: Vec::new(),
            next: EntryId::default(),
        };
        tree.read(&tree.root.clone());
        tree
    }

    /// The worktree this tree lists.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Shows or hides what the directory named `id` holds.
    ///
    /// A directory opened for the first time is read here; after that it is
    /// remembered, so opening it again costs nothing.
    pub fn toggle(&mut self, id: EntryId) {
        let Some(path) = self.path_of(id) else {
            return;
        };
        if let Some(index) = self.expanded.iter().position(|open| *open == path) {
            self.expanded.remove(index);
            return;
        }
        if !self.children.contains_key(&path) {
            self.read(&path);
        }
        self.expanded.push(path);
    }

    /// Reads the worktree again, keeping whatever was expanded expanded.
    ///
    /// Everything read so far is thrown away rather than reconciled: a tree
    /// is what is on disk, and the one thing that survives a change to the
    /// disk is which directories the reader had opened.
    pub fn reload(&mut self) {
        self.children.clear();
        self.read(&self.root.clone());
        for path in self.expanded.clone() {
            if path.is_dir() {
                self.read(&path);
            }
        }
        self.expanded.retain(|path| path.is_dir());
    }

    /// Whether a path made or taken away at `path` changes what the tree lists.
    ///
    /// Only a directory the tree has read can show the difference; one that
    /// has never been opened is read afresh whenever it is.
    pub fn lists_beside(&self, path: &Path) -> bool {
        path.parent()
            .is_some_and(|parent| self.children.contains_key(parent))
    }

    /// The visible lines of the tree, in the order they are drawn.
    pub fn rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        self.collect(&self.root, 0, &mut rows);
        rows
    }

    /// Appends the lines of `directory` and of whatever it has open.
    fn collect<'a>(&'a self, directory: &Path, depth: usize, rows: &mut Vec<Row<'a>>) {
        let Some(entries) = self.children.get(directory) else {
            return;
        };
        for entry in entries {
            let expanded = self.expanded.contains(&entry.path);
            rows.push(Row {
                entry,
                depth,
                expanded,
            });
            if expanded {
                self.collect(&entry.path, depth + 1, rows);
            }
        }
    }

    /// Where the entry named `id` is, while the tree still holds it.
    fn path_of(&self, id: EntryId) -> Option<PathBuf> {
        self.children
            .values()
            .flatten()
            .find(|entry| entry.id == id)
            .map(|entry| entry.path.clone())
    }

    /// Reads `directory`, directories first and each half sorted by name.
    ///
    /// A directory that cannot be read lists as empty: a permission the editor
    /// does not have is not a reason to fail the frame it is drawn in.
    fn read(&mut self, directory: &Path) {
        let Ok(listing) = std::fs::read_dir(directory) else {
            self.children.insert(directory.to_path_buf(), Vec::new());
            return;
        };

        let mut entries = Vec::new();
        for item in listing.flatten() {
            let path = item.path();
            let is_directory = item.file_type().is_ok_and(|kind| kind.is_dir());
            let id = self.next;
            self.next = EntryId(id.0 + 1);
            entries.push(Entry::at(id, path, is_directory));
        }
        entries.sort_by(|left, right| {
            right
                .directory
                .cmp(&left.directory)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });

        self.children.insert(directory.to_path_buf(), entries);
    }
}
