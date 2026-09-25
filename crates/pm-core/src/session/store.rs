//! The set of sessions the window holds, and the seam that changes it.

use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use crate::git::{self, Summary};
use crate::project::{Project, ProjectId};
use crate::session::bootstrap::{self, Bootstrap};
use crate::session::placement;
use crate::session::{Cut, Session, SessionId};

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

    /// Cuts a session of `project` called `name`, under `under`, of the
    /// repositories `chosen` names.
    ///
    /// Each chosen repository is cut as a worktree from `base`
    /// and left detached: a session is somewhere to work, and the branch it
    /// ends up on is named later, by whoever ends up naming it. An empty
    /// `base` is what each repository has checked out, which is where a
    /// reader who was not asked means. The worktrees are laid out the way the
    /// project lays its repositories out, and what lies outside all of them
    /// is copied in beside them; a repository that was not chosen is left
    /// out of the session altogether.
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
        chosen: &[PathBuf],
        under: &Path,
        wanted: &Bootstrap,
    ) -> Result<Started, StartError> {
        if !project.is_repository() {
            return Err(StartError::NotARepository);
        }
        let cut_from = match base.is_empty() {
            true => "HEAD",
            false => base,
        };
        let planned = project
            .repositories()
            .iter()
            .filter(|repository| chosen.iter().any(|root| root == repository.root()))
            .map(|repository| {
                let origin = repository.root().to_path_buf();
                let base = git::commit_of(&origin, cut_from).ok_or(StartError::NoCommit)?;
                Ok((origin, base))
            })
            .collect::<Result<Vec<_>, StartError>>()?;
        if planned.is_empty() {
            return Err(StartError::NothingChosen);
        }
        let root = placement::place(under, project.name(), name);

        std::fs::create_dir_all(&root).map_err(|error| StartError::Place {
            path: root.clone(),
            trouble: error.to_string(),
        })?;
        let mut cuts = Vec::new();
        for (origin, base) in planned {
            let within = origin.strip_prefix(project.root()).unwrap_or(Path::new(""));
            let cut = Cut {
                root: root.join(within),
                origin,
                base,
            };
            if let Err(said) = git::add_worktree(&cut.origin, &cut.root, &cut.base) {
                tear_down(&root, &cuts);
                return Err(StartError::Git(said));
            }
            git::remember(&cut.root, &cut.base, name);
            cuts.push(cut);
        }

        let mut trouble = Vec::new();
        if !cuts.iter().any(|cut| cut.root == root) {
            let repositories = project
                .repositories()
                .iter()
                .map(|repository| repository.root().to_path_buf())
                .collect::<Vec<_>>();
            trouble.extend(bootstrap::loose(
                project.root(),
                &root,
                &repositories,
                wanted,
            ));
            trouble.extend(bootstrap::apply(project.root(), &root, wanted));
        }
        for cut in &cuts {
            trouble.extend(bootstrap::apply(&cut.origin, &cut.root, wanted));
        }
        let port = bootstrap::free_port(&self.ports());
        if let Some(port) = port {
            for cut in &cuts {
                git::remember_port(&cut.root, port);
            }
        }

        let id = self.next;
        self.next = id.next();
        self.open.push(Session {
            id,
            project: project.id(),
            name: name.to_owned(),
            summary: super::drift(&cuts),
            root,
            cuts,
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
        for cut in session.cuts.iter().rev() {
            git::remove_worktree(&cut.origin, &cut.root).map_err(StartError::Git)?;
        }
        if session.root.exists() {
            std::fs::remove_dir_all(&session.root).map_err(|error| StartError::Place {
                path: session.root.clone(),
                trouble: error.to_string(),
            })?;
        }
        self.open.remove(at);
        Ok(())
    }

    /// Takes up the sessions `project` already has under `under`.
    ///
    /// A session outlives the window that started it, because its worktrees
    /// do: what a launch has to do is recognise the ones that are still
    /// there, which is git's list of each repository's worktrees narrowed to
    /// the ones in our own home, gathered by the session folder they sit in.
    /// A session already taken up is left alone, so this is safe to run
    /// again.
    pub fn adopt(&mut self, project: &Project, under: &Path) {
        let ours = under.join(placement::slug(project.name()));
        let mut found: Vec<(PathBuf, Vec<Cut>)> = Vec::new();

        for repository in project.repositories() {
            let origin = repository.root().to_path_buf();
            for root in git::worktrees(&origin) {
                let Some(folder) = session_folder(&ours, &root) else {
                    continue;
                };
                if self.open.iter().any(|open| open.root == folder) {
                    continue;
                }
                let base = git::remembered_base(&root)
                    .or_else(|| git::commit_of(&root, "HEAD"))
                    .unwrap_or_default();
                let cut = Cut {
                    origin: origin.clone(),
                    root,
                    base,
                };
                match found.iter_mut().find(|(held, _)| *held == folder) {
                    Some((_, cuts)) => cuts.push(cut),
                    None => found.push((folder, vec![cut])),
                }
            }
        }

        for (root, cuts) in found {
            let first = &cuts[0].root;
            let name = git::remembered_name(first).unwrap_or_else(|| named(&root));
            let port = git::remembered_port(first);

            let id = self.next;
            self.next = id.next();
            self.open.push(Session {
                id,
                project: project.id(),
                name,
                summary: super::drift(&cuts),
                root,
                cuts,
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

    /// What asks git again how far every session has drifted from its base,
    /// on whichever thread it is called on.
    ///
    /// Git is a subprocess per worktree, so the asking is handed out rather
    /// than done here, and what it answers comes back through
    /// [`Sessions::drifted`].
    pub fn read_drift(&self) -> impl FnOnce() -> Vec<(SessionId, Summary)> + Send + 'static {
        let cuts = self
            .open
            .iter()
            .map(|session| (session.id, session.cuts.clone()))
            .collect::<Vec<_>>();
        move || {
            cuts.into_iter()
                .map(|(id, cuts)| (id, super::drift(&cuts)))
                .collect()
        }
    }

    /// Takes in how far the sessions have drifted, as [`Sessions::read_drift`]
    /// read it; a session finished since is passed over.
    pub fn drifted(&mut self, drifts: Vec<(SessionId, Summary)>) {
        for (id, summary) in drifts {
            if let Some(session) = self.open.iter_mut().find(|session| session.id == id) {
                session.summary = summary;
            }
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

/// The session folder a worktree at `root` belongs to, when it sits in
/// `ours` at all: the directory one level below it.
fn session_folder(ours: &Path, root: &Path) -> Option<PathBuf> {
    let first = root.strip_prefix(ours).ok()?.components().next()?;
    Some(ours.join(first))
}

/// Takes away what a session that could not be cut in full had cut so far.
fn tear_down(root: &Path, cuts: &[Cut]) {
    for cut in cuts.iter().rev() {
        let _ = git::remove_worktree(&cut.origin, &cut.root);
    }
    let _ = std::fs::remove_dir_all(root);
}

/// What a session directory is called when nothing wrote a name down.
fn named(root: &Path) -> String {
    root.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// Why a session could not be started or finished.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartError {
    /// The project is a plain folder, with no repository to cut a worktree of.
    NotARepository,
    /// None of the project's repositories was chosen to be cut.
    NothingChosen,
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
            Self::NotARepository => write!(formatter, "the project is not a git repository"),
            Self::NothingChosen => write!(formatter, "no repository was chosen"),
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
