//! A folder the window holds open, and the set of them it has open.
//!
//! [`Projects`] is the one seam a project is opened and closed through: a
//! keybinding, the title-bar menu and a restored window all go through it, and
//! nothing else hands out a [`ProjectId`]. [`repository`] is what tells a path
//! inside a repository from a plain folder; [`store`] is the set itself.

mod repository;
mod store;

pub use repository::repositories;
pub use store::{OpenError, Projects};

use pm_host::Location;

/// A project's identity for as long as the window holds it open.
///
/// The id is handed out by [`Projects`] and means nothing outside the window
/// that issued it. Everything scoped by a project — a session, a buffer, a
/// task, a language server — carries one of these rather than a path, so a
/// project that moves on disk stays the same project.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectId(u64);

impl ProjectId {
    /// The project identity within this running editor window.
    pub fn number(self) -> u64 {
        self.0
    }

    /// The id following this one, for the store handing them out.
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A folder the window holds open, and the repositories it holds.
///
/// A project is a repository's working copy, a folder of several of them, or
/// a plain folder with none — its files, its terminals and its language
/// servers work the same whichever it is. Only git needs to know which: a
/// plain folder has no branch to state and no history to cut a session from,
/// and a folder of several has one of each per repository.
#[derive(Clone, Debug)]
pub struct Project {
    /// What this project is called for as long as it is open.
    id: ProjectId,
    /// The folder the project is rooted at.
    root: Location,
    /// The last component of the root, shown wherever the project is named.
    name: String,
    /// The repositories at or below the root, the root's own first.
    repositories: Vec<Repository>,
}

impl Project {
    /// The project rooted at `root`, reading its repositories off disk.
    fn at(id: ProjectId, root: Location) -> Self {
        let name = root.file_name().map_or_else(
            || root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let name = root
            .host
            .name()
            .map_or(name.clone(), |host| format!("{name} ({host})"));
        let repositories = Repository::under(&root);

        Self {
            id,
            root,
            name,
            repositories,
        }
    }

    /// What this project is called for as long as it is open.
    pub fn id(&self) -> ProjectId {
        self.id
    }

    /// The folder the project is rooted at.
    pub fn root(&self) -> &Location {
        &self.root
    }

    /// The last component of the root, shown wherever the project is named.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The repositories at or below the root, the root's own first.
    pub fn repositories(&self) -> &[Repository] {
        &self.repositories
    }

    /// The branch the project has checked out, while it holds exactly one
    /// repository.
    ///
    /// A plain folder has no branch, and a folder of several repositories has
    /// one per repository rather than one to speak for all of them.
    pub fn branch(&self) -> Option<&str> {
        match self.repositories.as_slice() {
            [only] => Some(only.branch()),
            _ => None,
        }
    }

    /// Whether the project holds a repository at all.
    pub fn is_repository(&self) -> bool {
        !self.repositories.is_empty()
    }

    /// Reads the repositories and the branches they have out again, noticing
    /// one cloned into the folder or taken out of it since.
    fn refresh(&mut self) {
        self.repositories = Repository::under(&self.root);
    }
}

/// One repository a project holds: where it is, and what it has checked out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repository {
    /// The repository's working-copy root.
    root: Location,
    /// The branch the working copy has checked out.
    branch: String,
}

impl Repository {
    /// Every repository at or below `root`, the root's own first.
    fn under(root: &Location) -> Vec<Self> {
        repository::repositories(root)
            .into_iter()
            .map(|root| Self {
                branch: repository::branch(&root),
                root,
            })
            .collect()
    }

    /// The repository's working-copy root.
    pub fn root(&self) -> &Location {
        &self.root
    }

    /// The branch the working copy has checked out.
    pub fn branch(&self) -> &str {
        &self.branch
    }
}
