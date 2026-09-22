//! The set of sessions the window holds, and the seam that changes it.

use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use crate::git;
use crate::project::{Project, ProjectId};
use crate::session::bootstrap::{self, Bootstrap};
use crate::session::placement;
use crate::session::{Session, SessionId};

/// Every session the window holds, in the order they were started.
///
/// Sessions are scoped by project the way everything else is: a session that
/// is not attached to a project id is a bug, and a project leaving the window
/// takes its sessions' rows with it — though not their worktrees, which are
/// git's and survive until they are finished.
#[derive(Debug, Default)]
pub struct Sessions {
    /// The sessions, in the order they were started.
    open: Vec<Session>,
    /// The id the next session started will be given.
    next: SessionId,
}

impl Sessions {
    /// A window holding no sessions.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cuts a worktree for a session of `project` called `name`, under `under`.
    ///
    /// The worktree is cut from `base` and left detached: a session is
    /// somewhere to work, and the branch it ends up on is named later, by
    /// whoever ends up naming it. An empty `base` is what the project has
    /// checked out, which is where a reader who was not asked means.
    ///
    /// What git leaves out of a worktree, `wanted` brings across, and the
    /// session is given a port nothing else in the window has. Neither can
    /// fail the session: a worktree that was cut exists, and what could not
    /// be brought into it is reported instead.
    pub fn start(
        &mut self,
        project: &Project,
        name: &str,
        base: &str,
        under: &Path,
        wanted: &Bootstrap,
    ) -> Result<Started, StartError> {
        let origin = project.root().to_path_buf();
        let cut_from = match base.is_empty() {
            true => "HEAD",
            false => base,
        };
        let base = git::commit_of(&origin, cut_from).ok_or(StartError::NoCommit)?;
        let root = placement::place(under, project.name(), name);

        if let Some(parent) = root.parent() {
            std::fs::create_dir_all(parent).map_err(|error| StartError::Place {
                path: parent.to_path_buf(),
                trouble: error.to_string(),
            })?;
        }
        git::add_worktree(&origin, &root, &base).map_err(StartError::Git)?;
        git::remember(&root, &base, name);

        let trouble = bootstrap::apply(&origin, &root, wanted);
        let port = bootstrap::free_port(&self.ports());
        if let Some(port) = port {
            git::remember_port(&root, port);
        }

        let id = self.next;
        self.next = id.next();
        self.open.push(Session {
            id,
            project: project.id(),
            name: name.to_owned(),
            summary: git::since(&root, &base),
            root,
            origin,
            base,
            port,
        });
        Ok(Started { id, trouble })
    }

    /// The ports the sessions of every project have already been given.
    fn ports(&self) -> Vec<u16> {
        self.open.iter().filter_map(Session::port).collect()
    }

    /// Takes the session `id` names out of the window and off disk.
    pub fn finish(&mut self, id: SessionId) -> Result<(), StartError> {
        let Some(at) = self.open.iter().position(|session| session.id == id) else {
            return Ok(());
        };
        let session = &self.open[at];
        git::remove_worktree(&session.origin, &session.root).map_err(StartError::Git)?;
        self.open.remove(at);
        Ok(())
    }

    /// Takes up the worktrees `project` already has under `under`.
    ///
    /// A session outlives the window that started it, because the worktree
    /// does: what a launch has to do is recognise the ones that are still
    /// there, which is git's list narrowed to the ones in our own home. A
    /// worktree already taken up is left alone, so this is safe to run again.
    pub fn adopt(&mut self, project: &Project, under: &Path) {
        let origin = project.root().to_path_buf();
        let ours = under.join(placement::slug(project.name()));

        for root in git::worktrees(&origin) {
            if !root.starts_with(&ours) || self.open.iter().any(|open| open.root == root) {
                continue;
            }
            let base = git::remembered_base(&root)
                .or_else(|| git::commit_of(&root, "HEAD"))
                .unwrap_or_default();
            let name = git::remembered_name(&root).unwrap_or_else(|| named(&root));
            let port = git::remembered_port(&root);

            let id = self.next;
            self.next = id.next();
            self.open.push(Session {
                id,
                project: project.id(),
                name,
                summary: git::since(&root, &base),
                root,
                origin: origin.clone(),
                base,
                port,
            });
        }
    }

    /// The session `id` names, while the window still holds it.
    pub fn get(&self, id: SessionId) -> Option<&Session> {
        self.open.iter().find(|session| session.id == id)
    }

    /// The sessions of `project`, in the order they were started.
    pub fn of(&self, project: ProjectId) -> impl Iterator<Item = &Session> {
        self.open
            .iter()
            .filter(move |session| session.project == project)
    }

    /// How many sessions `project` has.
    pub fn count(&self, project: ProjectId) -> usize {
        self.of(project).count()
    }

    /// Forgets the sessions of `project`, for a project leaving the window.
    ///
    /// The worktrees stay where they are: closing a project is putting it
    /// down, and the next window to open it finds its sessions again.
    pub fn close_project(&mut self, project: ProjectId) {
        self.open.retain(|session| session.project != project);
    }

    /// Asks git again how far every session has drifted from its base.
    pub fn reread(&mut self) {
        for session in &mut self.open {
            session.refresh();
        }
    }
}

/// A session that was cut, and what could not be brought into its worktree.
///
/// The two travel together because the session is started either way: a
/// dependency tree that could not be linked is worth saying out loud, but it
/// is not a reason to take a worktree away again.
#[derive(Clone, Debug)]
pub struct Started {
    /// The session that was cut.
    pub id: SessionId,
    /// What could not be brought across, one line apiece.
    pub trouble: Vec<String>,
}

/// What a session directory is called when nothing wrote a name down.
fn named(root: &Path) -> String {
    root.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// Why a session could not be started or finished.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartError {
    /// The project has no commit to cut a worktree from.
    NoCommit,
    /// The directory the worktree would go in could not be made.
    Place {
        /// The directory that could not be made.
        path: PathBuf,
        /// What the platform said about it.
        trouble: String,
    },
    /// Git would not do it, and said this.
    Git(String),
}

impl Display for StartError {
    /// Says why the session could not be started or finished.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommit => write!(formatter, "the project has no commits yet"),
            Self::Place { path, trouble } => {
                write!(
                    formatter,
                    "`{}` could not be made: {trouble}",
                    path.display()
                )
            }
            Self::Git(said) => write!(formatter, "{said}"),
        }
    }
}

impl std::error::Error for StartError {}
