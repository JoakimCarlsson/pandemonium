//! A folder the window holds open, and the set of them it has open.
//!
//! [`Projects`] is the one seam a project is opened and closed through: a
//! keybinding, the title-bar menu and a restored window all go through it, and
//! nothing else hands out a [`ProjectId`]. [`repository`] is what tells a path
//! inside a repository from a plain folder; [`store`] is the set itself.

mod repository;
mod store;

pub use store::{OpenError, Projects};

use std::path::{Path, PathBuf};

/// A project's identity for as long as the window holds it open.
///
/// The id is handed out by [`Projects`] and means nothing outside the window
/// that issued it. Everything scoped by a project — a session, a buffer, a
/// task, a language server — carries one of these rather than a path, so a
/// project that moves on disk stays the same project.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectId(u64);

impl ProjectId {
    /// The id following this one, for the store handing them out.
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A folder the window holds open, a repository's working copy or not.
///
/// A plain folder is a project like any other — its files, its terminals and
/// its language servers work the same — it only has no branch to state and
/// no history to cut a session from.
#[derive(Clone, Debug)]
pub struct Project {
    /// What this project is called for as long as it is open.
    id: ProjectId,
    /// The folder the project is rooted at: a working-copy root, or the plain
    /// folder that was opened.
    root: PathBuf,
    /// The last component of the root, shown wherever the project is named.
    name: String,
    /// The branch the working copy has checked out, or `None` for a folder
    /// no repository contains.
    branch: Option<String>,
}

impl Project {
    /// The project rooted at `root`, reading its name and branch off disk.
    fn at(id: ProjectId, root: PathBuf) -> Self {
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let branch = repository::is_root(&root).then(|| repository::branch(&root));

        Self {
            id,
            root,
            name,
            branch,
        }
    }

    /// What this project is called for as long as it is open.
    pub fn id(&self) -> ProjectId {
        self.id
    }

    /// The folder the project is rooted at.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The last component of the root, shown wherever the project is named.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The branch the working copy has checked out, or `None` for a folder
    /// no repository contains.
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// Whether the project is a repository's working copy.
    pub fn is_repository(&self) -> bool {
        self.branch.is_some()
    }

    /// Reads the branch checked out in this working copy again, noticing a
    /// folder that has since become a repository or stopped being one.
    fn refresh(&mut self) {
        self.branch = repository::is_root(&self.root).then(|| repository::branch(&self.root));
    }
}
