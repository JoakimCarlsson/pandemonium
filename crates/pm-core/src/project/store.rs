//! The set of projects the window holds open, and the seam that changes it.

use pm_host::Location;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use crate::project::repository;
use crate::project::{Project, ProjectId, Repository};

/// Every project the window holds open, in the order the window lists them.
///
/// Projects are peers: the set has no implicit first project and no notion of
/// one window per project. One of them is active — the project a new session
/// or a project-scoped command applies to — and that is the only distinction
/// the set draws between them.
#[derive(Debug, Default)]
pub struct Projects {
    /// The open projects, in the order the window lists them.
    open: Vec<Project>,
    /// The project a project-scoped command applies to.
    active: Option<ProjectId>,
    /// The id the next project opened will be given.
    next: ProjectId,
}

impl Projects {
    /// A window holding nothing open.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens `path` as a project and makes it the active project.
    ///
    /// A path inside a repository opens that repository's working copy, the
    /// way a file dropped on the window opens the project it belongs to. A
    /// folder no repository contains opens as itself. A file outside every
    /// repository opens the folder holding it.
    ///
    /// A path already open, or inside a project already open, resolves to
    /// the project that is already there instead of being held twice, so a
    /// menu entry, a keybinding and a restored window all land on the same
    /// project — and a repository inside a folder of several is the folder's.
    pub fn find_or_open(&mut self, path: impl Into<Location>) -> Result<ProjectId, OpenError> {
        let location = path.into();
        let path = if location.host.connected() {
            location
                .host
                .fs()
                .canonicalize(&location.path)
                .map_err(|_| OpenError::Missing {
                    path: location.path.clone(),
                })?
        } else {
            location.path.clone()
        };
        let path = location.at(path);
        let holding = self
            .open
            .iter()
            .filter(|project| path.host == project.root().host && path.starts_with(project.root()))
            .max_by_key(|project| project.root().components().count());
        if let Some(project) = holding {
            let id = project.id();
            self.active = Some(id);
            return Ok(id);
        }

        let root = repository::root(&path).unwrap_or_else(|| folder_of(path));
        let id = self.next;
        self.next = id.next();
        self.open.push(Project::at(id, root));
        self.active = Some(id);
        Ok(id)
    }

    /// Takes `id` out of the window, passing active to one of its neighbours.
    pub fn remove(&mut self, id: ProjectId) {
        let Some(index) = self.open.iter().position(|project| project.id() == id) else {
            return;
        };
        self.open.remove(index);

        if self.active == Some(id) {
            let neighbour = index.min(self.open.len().saturating_sub(1));
            self.active = self.open.get(neighbour).map(Project::id);
        }
    }

    /// Moves `id` into the gap before the project at `gap`, or to the end.
    ///
    /// A gap counts the projects as they stand before the move, so the gaps
    /// either side of `id` leave the order as it is.
    pub fn move_to(&mut self, id: ProjectId, gap: usize) {
        let Some(from) = self.open.iter().position(|project| project.id() == id) else {
            return;
        };
        let project = self.open.remove(from);
        let at = match gap > from {
            true => gap - 1,
            false => gap,
        };
        self.open.insert(at.min(self.open.len()), project);
    }

    /// Makes `id` the project that project-scoped commands apply to.
    pub fn activate(&mut self, id: ProjectId) {
        if self.open.iter().any(|project| project.id() == id) {
            self.active = Some(id);
        }
    }

    /// Makes the first project active, as a window just restored wants it.
    pub fn activate_first(&mut self) {
        self.active = self.open.first().map(Project::id);
    }

    /// The project a project-scoped command applies to.
    pub fn active(&self) -> Option<&Project> {
        self.open
            .iter()
            .find(|project| Some(project.id()) == self.active)
    }

    /// The project `id` names, while the window still holds it open.
    pub fn get(&self, id: ProjectId) -> Option<&Project> {
        self.open.iter().find(|project| project.id() == id)
    }

    /// Reads the repositories of project `id`, and the branch each has out,
    /// again.
    pub fn refresh(&mut self, id: ProjectId) {
        if let Some(project) = self.open.iter_mut().find(|project| project.id() == id) {
            project.refresh();
        }
    }

    /// What reads the repositories of every open project again, and the
    /// branch each has out, on whichever thread it is called on.
    ///
    /// Finding the repositories is a walk of each folder and a subprocess
    /// per repository, so the reading is handed out rather than done here,
    /// and what it answers comes back through [`Projects::reread`].
    pub fn read_later(
        &self,
    ) -> impl FnOnce() -> Vec<(ProjectId, Vec<Repository>)> + Send + 'static + use<> {
        let roots = self
            .open
            .iter()
            .filter(|project| project.root().host.connected())
            .map(|project| (project.id(), project.root().clone()))
            .collect::<Vec<_>>();
        move || {
            roots
                .into_iter()
                .map(|(id, root)| (id, Repository::under(&root)))
                .collect()
        }
    }

    /// Takes in the repositories [`Projects::read_later`] read; a project
    /// closed since is passed over.
    pub fn reread(&mut self, read: Vec<(ProjectId, Vec<Repository>)>) {
        for (id, repositories) in read {
            if let Some(project) = self.open.iter_mut().find(|project| project.id() == id) {
                project.repositories = repositories;
            }
        }
    }

    /// The open projects, in the order the window lists them.
    pub fn iter(&self) -> impl Iterator<Item = &Project> {
        self.open.iter()
    }

    /// How many projects the window holds open.
    pub fn len(&self) -> usize {
        self.open.len()
    }

    /// Whether the window holds nothing open.
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// The roots to reopen to bring this window back as it stands.
    pub fn roots(&self) -> Vec<PathBuf> {
        self.open
            .iter()
            .map(|project| project.root().stored())
            .collect()
    }
}

/// The folder `path` names, or the one holding it when it names a file.
fn folder_of(path: Location) -> Location {
    if !path.host.connected() {
        return path;
    }
    match path.host.fs().is_dir(&path) {
        true => path,
        false => path.parent().map_or(path.clone(), |parent| path.at(parent)),
    }
}

/// Why a path could not be opened as a project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenError {
    /// Nothing on disk answers to this path.
    Missing {
        /// The path that answered to nothing.
        path: PathBuf,
    },
}

impl Display for OpenError {
    /// Says why the path could not be opened.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(formatter, "`{}` does not exist", path.display()),
        }
    }
}

impl std::error::Error for OpenError {}
