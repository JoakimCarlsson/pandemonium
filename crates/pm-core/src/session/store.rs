//! The set of sessions the window holds, and the seam that changes it.

use pm_host::Location;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use crate::git::{self, Summary};
use crate::project::{Project, ProjectId};
use crate::session::bootstrap::{self, Bootstrap};
use crate::session::placement;
use crate::session::{Cut, Provision, Session, SessionId};
use std::sync::Arc;

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

    /// What cuts a session of `project` called `name`, under `under`, of the
    /// repositories `chosen` names, on whichever thread it is called on.
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
    /// session is given a port no session held now has. Neither can fail the
    /// session: a worktree that was cut exists, and what could not be brought
    /// into it is reported instead. Cutting is a subprocess or more per
    /// repository and a copy of whatever `wanted` names, so it is handed out
    /// rather than done here, and what it cut is taken up through
    /// [`Sessions::took`].
    #[allow(clippy::too_many_arguments)]
    pub fn cut_later(
        &self,
        project: &Project,
        name: &str,
        base: &str,
        chosen: &[PathBuf],
        under: &Path,
        wanted: &Bootstrap,
        provision: Option<Arc<dyn Provision>>,
    ) -> impl FnOnce() -> Result<Cutting, StartError> + Send + 'static + use<> {
        let project = project.clone();
        let name = name.to_owned();
        let base = base.to_owned();
        let chosen = chosen.to_vec();
        let under = under.to_path_buf();
        let wanted = wanted.clone();
        let ports = self.ports();
        move || {
            cut(
                &project,
                &name,
                &base,
                &chosen,
                &under,
                &wanted,
                &ports,
                provision.as_deref(),
            )
        }
    }

    /// Takes up a session [`Sessions::cut_later`] cut, giving it its id.
    ///
    /// A session that was found on disk while its environment was still being
    /// set up is the same session, and keeps the id it was found under.
    pub fn took(&mut self, cutting: Cutting) -> Started {
        if let Some(held) = self
            .open
            .iter_mut()
            .find(|session| session.source == cutting.source)
        {
            held.project = cutting.project;
            held.name = cutting.name;
            held.summary = cutting.summary;
            held.root = cutting.root;
            held.cuts = cutting.cuts;
            held.port = cutting.port;
            return Started {
                id: held.id,
                trouble: cutting.trouble,
            };
        }
        let id = self.next;
        self.next = id.next();
        self.open.push(Session {
            delegation: None,
            id,
            project: cutting.project,
            name: cutting.name,
            summary: cutting.summary,
            root: cutting.root,
            source: cutting.source,
            cuts: cutting.cuts,
            port: cutting.port,
        });
        Started {
            id,
            trouble: cutting.trouble,
        }
    }

    /// The ports the sessions of every project have already been given.
    fn ports(&self) -> Vec<u16> {
        self.open.iter().filter_map(Session::port).collect()
    }

    /// What takes the session `id` names off disk, on whichever thread it
    /// is called on.
    ///
    /// The session stays in the window until [`Sessions::forget`] is told
    /// it went, so a worktree git would not take away is still one to see.
    pub fn finish_later(
        &self,
        id: SessionId,
    ) -> impl FnOnce() -> Result<(), StartError> + Send + 'static + use<> {
        let held = self.get(id).map(|session| {
            (
                session.root.clone(),
                session.source.clone(),
                session.cuts.clone(),
            )
        });
        move || {
            let Some((execution, root, cuts)) = held else {
                return Ok(());
            };
            if execution != root {
                execution
                    .host
                    .retire(&root.path)
                    .map_err(|error| StartError::Environment(error.to_string()))?;
            }
            for cut in cuts.iter().rev() {
                git::remove_worktree(&cut.origin, &cut.root).map_err(StartError::Git)?;
            }
            if root.host.fs().exists(&root) {
                root.host
                    .fs()
                    .remove_dir_all(&root)
                    .map_err(|error| StartError::Place {
                        path: root.path.clone(),
                        trouble: error.to_string(),
                    })?;
            }
            Ok(())
        }
    }

    /// Takes the session `id` names out of the window, once it is off disk.
    pub fn forget(&mut self, id: SessionId) {
        self.open.retain(|session| session.id != id);
    }

    /// What finds the sessions `projects` already have under `under`, on
    /// whichever thread it is called on.
    ///
    /// A session outlives the window that started it, because its worktrees
    /// do: what a launch has to do is recognise the ones that are still
    /// there, which is git's list of each repository's worktrees narrowed to
    /// the ones in our own home, gathered by the session folder they sit in.
    /// A session already held is passed over, here and again once what was
    /// found is taken up through [`Sessions::adopt`], so this is safe to run
    /// again.
    pub fn find_later(
        &self,
        projects: &[Project],
        under: &Path,
        provision: Option<Arc<dyn Provision>>,
    ) -> impl FnOnce() -> Vec<Found> + Send + 'static + use<> {
        let projects = projects
            .iter()
            .filter(|project| {
                !projects.iter().any(|other| {
                    other
                        .native_root()
                        .is_some_and(|root| &root == project.root())
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        let under = under.to_path_buf();
        let held = self
            .open
            .iter()
            .map(|session| session.source.clone())
            .collect::<Vec<_>>();
        move || {
            projects
                .iter()
                .flat_map(|project| find(project, &under, &held, provision.as_deref()))
                .collect()
        }
    }

    /// Takes up the sessions [`Sessions::find_later`] found, passing over
    /// one taken up since it was asked.
    pub fn adopt(&mut self, found: Vec<Found>) {
        for found in found {
            if self.open.iter().any(|open| open.source == found.source) {
                continue;
            }
            let id = self.next;
            self.next = id.next();
            self.open.push(Session {
                delegation: found.delegation,
                id,
                project: found.project,
                name: found.name,
                summary: found.summary,
                root: found.root,
                source: found.source,
                cuts: found.cuts,
                port: found.port,
            });
        }
    }

    /// Persists a delegated session's parent before exposing it to its caller.
    pub fn delegated(
        &mut self,
        id: SessionId,
        delegation: super::Delegation,
    ) -> Result<(), String> {
        let session = self
            .open
            .iter_mut()
            .find(|session| session.id == id)
            .ok_or("The delegated session disappeared")?;
        let text = serde_json::to_string(&delegation).map_err(|error| error.to_string())?;
        let first = session
            .cuts
            .first()
            .ok_or("The delegated session has no worktree")?;
        git::remember_delegation(&first.root, &text)?;
        session.delegation = Some(delegation);
        Ok(())
    }

    /// The session `id` names, while the window still holds it.
    pub fn get(&self, id: SessionId) -> Option<&Session> {
        self.open.iter().find(|session| session.id == id)
    }

    /// Every held session in the order it was started.
    pub fn iter(&self) -> impl Iterator<Item = &Session> {
        self.open.iter()
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

/// A session's worktrees, cut and not yet taken up by the window.
#[derive(Debug)]
pub struct Cutting {
    /// The project whose repositories it was cut from.
    project: ProjectId,
    /// What the reader called it.
    name: String,
    /// The folder the agent works in, as tools see it.
    root: Location,
    /// The folder the worktrees were cut in.
    source: Location,
    /// The worktree cut of each repository, the outermost first.
    cuts: Vec<Cut>,
    /// The local port it serves on, where one was free.
    port: Option<u16>,
    /// How far it has drifted from what it was cut from.
    summary: Summary,
    /// What could not be brought across, one line apiece.
    trouble: Vec<String>,
}

/// A session already on disk that the window did not hold when it looked.
#[derive(Debug)]
pub struct Found {
    /// Persisted parent of a delegated worktree.
    delegation: Option<super::Delegation>,
    /// The project whose repositories it was cut from.
    project: ProjectId,
    /// What it was called when it was cut.
    name: String,
    /// The session folder the worktrees sit in, as tools see it.
    root: Location,
    /// The session folder where the worktrees sit.
    source: Location,
    /// The worktree cut of each repository, the outermost first.
    cuts: Vec<Cut>,
    /// The local port it was given, if it was given one.
    port: Option<u16>,
    /// How far it has drifted from what it was cut from.
    summary: Summary,
}

/// Cuts a session of `project`, as [`Sessions::cut_later`] says, giving it
/// a port none of `taken` is.
///
/// A project living in an environment is cut on the desktop, where its files
/// are, and the session is then given an environment of its own, set up before
/// the session is taken up. Dependency trees are not linked into it: a link
/// would point at a desktop path the session's environment cannot see.
#[allow(clippy::too_many_arguments)]
fn cut(
    project: &Project,
    name: &str,
    base: &str,
    chosen: &[PathBuf],
    under: &Path,
    wanted: &Bootstrap,
    taken: &[u16],
    provision: Option<&dyn Provision>,
) -> Result<Cutting, StartError> {
    let native = project.native();
    if native.is_none() && provision.is_none() {
        return cut_on(project, name, base, chosen, under, wanted, taken);
    }
    let host = &project.root().host;
    let chosen = chosen
        .iter()
        .map(|path| {
            host.native(path)
                .map_or_else(|| path.clone(), |native| native.path)
        })
        .collect::<Vec<_>>();
    let wanted = Bootstrap {
        link: Vec::new(),
        ..wanted.clone()
    };
    let working = native.as_ref().unwrap_or(project);
    let mut cutting = cut_on(working, name, base, &chosen, under, &wanted, taken)?;
    let isolated = match (&native, provision) {
        (Some(_), _) => host.isolate(&cutting.source.path),
        (None, Some(provision)) => provision.isolate(project.root(), &cutting.source.path),
        (None, None) => unreachable!(),
    };
    match isolated {
        Ok(execution) => {
            cutting.root = execution;
            Ok(cutting)
        }
        Err(error) => {
            tear_down(&cutting.source, &cutting.cuts);
            Err(StartError::Environment(error.to_string()))
        }
    }
}

/// Cuts a session of `project`, wherever its files are, on its own machine.
fn cut_on(
    project: &Project,
    name: &str,
    base: &str,
    chosen: &[PathBuf],
    under: &Path,
    wanted: &Bootstrap,
    taken: &[u16],
) -> Result<Cutting, StartError> {
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
        .filter(|repository| chosen.iter().any(|root| root == &repository.root().path))
        .map(|repository| {
            let origin = repository.root().clone();
            let base = git::commit_of(&origin, cut_from).ok_or(StartError::NoCommit)?;
            Ok((origin, base))
        })
        .collect::<Result<Vec<_>, StartError>>()?;
    if planned.is_empty() {
        return Err(StartError::NothingChosen);
    }
    let under = worktrees_on(project, under).map_err(|error| StartError::Place {
        path: under.to_path_buf(),
        trouble: error.to_string(),
    })?;
    let root =
        placement::reserve(&under, project.name(), name).map_err(|error| StartError::Place {
            path: under.to_path_buf(),
            trouble: error.to_string(),
        })?;
    let root = project.root().at(root);
    let mut cuts = Vec::new();
    for (origin, base) in planned {
        let within = origin.strip_prefix(project.root()).unwrap_or(Path::new(""));
        let cut = Cut {
            root: root.at(root.join(within)),
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
    let port = bootstrap::free_port(&project.root().host, taken);
    if let Some(port) = port {
        for cut in &cuts {
            git::remember_port(&cut.root, port);
        }
    }

    Ok(Cutting {
        project: project.id(),
        name: name.to_owned(),
        summary: super::drift(&cuts),
        source: root.clone(),
        root,
        cuts,
        port,
        trouble,
    })
}

/// Resolves the editor's worktree storage on the same machine as the project.
fn worktrees_on(project: &Project, local: &Path) -> std::io::Result<Location> {
    let root = project.root();
    if root.host.is_local() {
        Ok(root.at(local))
    } else {
        let home = root.host.home().ok_or_else(|| {
            std::io::Error::other("The remote host did not report a home directory")
        })?;
        Ok(root.at(home.join(".pandemonium/worktrees")))
    }
}

/// The sessions of `project` under `under` whose folders are none of `held`.
fn find(
    original: &Project,
    under: &Path,
    held: &[Location],
    provision: Option<&dyn Provision>,
) -> Vec<Found> {
    let native = original.native();
    let project = native.as_ref().unwrap_or(original);
    let Ok(under) = worktrees_on(project, under) else {
        return Vec::new();
    };
    let ours = under.join(placement::slug(project.name()));
    let mut found: Vec<(Location, Vec<Cut>)> = Vec::new();

    for repository in project.repositories() {
        let origin = repository.root().clone();
        for root in git::worktrees(&origin) {
            let Some(folder) = session_folder(&ours, &root) else {
                continue;
            };
            let folder = origin.at(folder);
            let root = origin.at(root);
            if held.contains(&folder) {
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
            match found
                .iter_mut()
                .find(|(folder_held, _)| *folder_held == folder)
            {
                Some((_, cuts)) => cuts.push(cut),
                None => found.push((folder, vec![cut])),
            }
        }
    }

    found
        .into_iter()
        .filter_map(|(source, cuts)| {
            let first = &cuts[0].root;
            let root = match (&native, provision) {
                (Some(_), _) => original.root().host.locate(&source.path).ok()?,
                (None, Some(provision)) => provision
                    .locate(original.root(), &source.path)
                    .unwrap_or_else(|| source.clone()),
                (None, None) => source.clone(),
            };
            Some(Found {
                delegation: git::remembered_delegation(first)
                    .and_then(|text| serde_json::from_str(&text).ok()),
                project: project.id(),
                name: git::remembered_name(first).unwrap_or_else(|| named(&source)),
                port: git::remembered_port(first),
                summary: super::drift(&cuts),
                root,
                source,
                cuts,
            })
        })
        .collect()
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
fn tear_down(root: &Location, cuts: &[Cut]) {
    for cut in cuts.iter().rev() {
        let _ = git::remove_worktree(&cut.origin, &cut.root);
    }
    let _ = root.host.fs().remove_dir_all(root);
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
    /// The session's environment could not be set up or taken away, and said this.
    Environment(String),
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
            Self::Environment(said) => write!(formatter, "the session's environment: {said}"),
        }
    }
}

impl std::error::Error for StartError {}
