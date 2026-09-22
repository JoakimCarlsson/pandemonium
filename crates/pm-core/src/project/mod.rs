//! A repository the window holds open, and the set of them it has open.
//!
//! [`Projects`] is the one seam a project is opened and closed through: a
//! keybinding, the title-bar menu and a restored window all go through it, and
//! nothing else hands out a [`ProjectId`]. [`repository`] is what turns a path
//! on disk into one of these; [`store`] is the set itself.

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

/// A repository the window holds open.
#[derive(Clone, Debug)]
pub struct Project {
    /// What this project is called for as long as it is open.
    id: ProjectId,
    /// The repository's working-copy root.
    root: PathBuf,
    /// The last component of the root, shown wherever the project is named.
    name: String,
    /// The branch the working copy has checked out.
    branch: String,
}

impl Project {
    /// The project rooted at `root`, reading its name and branch off disk.
    fn at(id: ProjectId, root: PathBuf) -> Self {
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let branch = repository::branch(&root);

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

    /// The repository's working-copy root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The last component of the root, shown wherever the project is named.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The branch the working copy has checked out.
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Reads the branch checked out in this working copy again.
    fn refresh(&mut self) {
        self.branch = repository::branch(&self.root);
    }
}
