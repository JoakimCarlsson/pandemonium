//! Watching the folders clients look at, so each is told as soon as the files of one, or what git keeps about them, change.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use pm_protocol::ServerMessage;

use crate::core::{Connection, Core};

/// How long a folder has to stay still after a change before clients are told of it.
const QUIET: Duration = Duration::from_millis(300);

/// How long a folder that keeps changing waits at most before clients are told of it.
const LONGEST: Duration = Duration::from_millis(1500);

/// How often the watcher looks for folders whose changes are due to be told.
const TICK: Duration = Duration::from_millis(100);

/// The folders, from the root of a watched one, whose changes never matter to what git shows.
const IGNORED: [&str; 5] = [
    ".pandemonium",
    "node_modules",
    "target",
    ".git/objects",
    ".git/logs",
];

/// The names of the folders a watched folder's walk does not go into, wherever they are.
const UNWALKED: [&str; 3] = [".pandemonium", "node_modules", "target"];

/// The most folders one watched folder has watched, so a huge tree does not use up what the system allows.
const MOST_FOLDERS: usize = 20_000;

/// The folders, inside a folder git keeps its state in, whose changes never matter to what git shows.
const IGNORED_IN_GIT: [&str; 2] = ["objects", "logs"];

/// A folder a client watches, and where its changes are seen.
struct Folder {
    /// Where the files are.
    root: PathBuf,
    /// The folders inside it that are watched, itself first, each watched without what is inside it.
    walked: Vec<PathBuf>,
    /// The folders git keeps the state of its worktree in, when they are outside it, each with how deep it is watched.
    git: Vec<(PathBuf, RecursiveMode)>,
}

impl Folder {
    /// The folder `directory`, as a client names it, found on this machine.
    fn find(directory: &str) -> Self {
        let root = crate::handler::place(directory);
        let mut git = Vec::new();
        let repositories = match pm_git::child_repositories(&root).as_slice() {
            [] => vec![root.clone()],
            children => children.iter().map(|child| root.join(child)).collect(),
        };
        for repository in repositories {
            let Some((own, common)) = pm_git::git_folders(&repository) else {
                continue;
            };
            if !own.starts_with(&root) {
                git.push((own, RecursiveMode::NonRecursive));
            }
            if !common.starts_with(&root) {
                git.push((common.join("refs"), RecursiveMode::Recursive));
                git.push((common, RecursiveMode::NonRecursive));
            }
        }
        let mut walked = Vec::new();
        walk(&root, &mut walked);
        Self { root, walked, git }
    }

    /// Watches the folder `created`, and the folders inside it, when it was made inside this one where it matters.
    fn grown(&mut self, created: &Path) -> bool {
        if !created.is_dir() || !created.starts_with(&self.root) || !self.concerns(created) {
            return false;
        }
        if self.walked.iter().any(|held| held == created) {
            return false;
        }
        walk(created, &mut self.walked);
        true
    }

    /// The paths to watch, each with how deep.
    fn watched(&self) -> Vec<(PathBuf, RecursiveMode)> {
        let mut paths: Vec<(PathBuf, RecursiveMode)> = self
            .walked
            .iter()
            .map(|folder| (folder.clone(), RecursiveMode::NonRecursive))
            .collect();
        paths.extend(self.git.iter().cloned());
        paths
    }

    /// Whether a change of `path` can change what git shows of the folder.
    fn concerns(&self, path: &Path) -> bool {
        if path
            .extension()
            .is_some_and(|extension| extension == "lock")
        {
            return false;
        }
        if let Ok(inside) = path.strip_prefix(&self.root) {
            return !IGNORED.iter().any(|ignored| inside.starts_with(ignored));
        }
        self.git.iter().any(|(folder, _)| {
            path.strip_prefix(folder).is_ok_and(|inside| {
                !IGNORED_IN_GIT
                    .iter()
                    .any(|ignored| inside.starts_with(ignored))
            })
        })
    }
}

/// Adds `folder` and the folders inside it to `found`, leaving out those named in [`UNWALKED`] and, of the folder git keeps its state in, all but itself and its references, until there are [`MOST_FOLDERS`].
fn walk(folder: &Path, found: &mut Vec<PathBuf>) {
    if found.len() >= MOST_FOLDERS {
        return;
    }
    found.push(folder.to_path_buf());
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !is_folder || UNWALKED.contains(&name.as_ref()) {
            continue;
        }
        if name == ".git" {
            found.push(entry.path());
            let references = entry.path().join("refs");
            if references.is_dir() {
                walk(&references, found);
            }
            continue;
        }
        walk(&entry.path(), found);
    }
}

/// When a folder first and last changed since its clients were last told.
#[derive(Clone, Copy)]
struct Changed {
    /// The first change.
    first: Instant,
    /// The last one.
    last: Instant,
}

/// What is watched and what has changed.
struct Watching {
    /// What reports the changes, or nothing when this machine could not give one.
    watcher: Option<RecommendedWatcher>,
    /// The folders each connected client watches, as it names them.
    wanted: Vec<(Arc<Connection>, HashSet<String>)>,
    /// Every folder some client watches, by the name clients give it.
    folders: HashMap<String, Folder>,
    /// The paths watched now, each with how deep.
    paths: HashMap<PathBuf, RecursiveMode>,
    /// The folders that changed and are yet to be told of.
    changed: HashMap<String, Changed>,
}

/// The folders clients watch.
pub(crate) struct Folders {
    /// What is watched and what has changed.
    watching: Mutex<Watching>,
    /// What the watcher reports, until the thread that reads it takes it.
    reports: Mutex<Option<Receiver<notify::Result<notify::Event>>>>,
}

impl Folders {
    /// Nothing watched yet.
    pub(crate) fn new() -> Self {
        let (sender, reports) = channel();
        Self {
            watching: Mutex::new(Watching {
                watcher: notify::recommended_watcher(sender).ok(),
                wanted: Vec::new(),
                folders: HashMap::new(),
                paths: HashMap::new(),
                changed: HashMap::new(),
            }),
            reports: Mutex::new(Some(reports)),
        }
    }

    /// Has `connection` watch the folders `directories` and no others.
    pub(crate) fn watch(&self, connection: &Arc<Connection>, directories: Vec<String>) {
        let Ok(mut watching) = self.watching.lock() else {
            return;
        };
        watching
            .wanted
            .retain(|(held, _)| !Arc::ptr_eq(held, connection));
        if !directories.is_empty() {
            watching
                .wanted
                .push((connection.clone(), directories.into_iter().collect()));
        }
        watching.settle();
    }

    /// Stops watching for `connection`, as it went away.
    pub(crate) fn forget(&self, connection: &Arc<Connection>) {
        self.watch(connection, Vec::new());
    }
}

impl Watching {
    /// Watches the paths of every folder some client wants, and only those.
    fn settle(&mut self) {
        let wanted: HashSet<String> = self
            .wanted
            .iter()
            .flat_map(|(_, directories)| directories.iter().cloned())
            .collect();
        self.folders
            .retain(|directory, _| wanted.contains(directory));
        self.changed
            .retain(|directory, _| wanted.contains(directory));
        for directory in wanted {
            self.folders
                .entry(directory.clone())
                .or_insert_with(|| Folder::find(&directory));
        }
        let paths: HashMap<PathBuf, RecursiveMode> =
            self.folders.values().flat_map(Folder::watched).collect();
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        for (path, mode) in &self.paths {
            if paths.get(path) != Some(mode) {
                let _ = watcher.unwatch(path);
            }
        }
        for (path, mode) in &paths {
            if self.paths.get(path) != Some(mode) {
                let _ = watcher.watch(path, *mode);
            }
        }
        self.paths = paths;
    }

    /// Stops counting the folders among `paths` that are gone, and those inside them, as watched.
    fn shrunk(&mut self, paths: &[PathBuf]) {
        let mut shrank = false;
        for folder in self.folders.values_mut() {
            let before = folder.walked.len();
            folder.walked.retain(|held| {
                held == &folder.root
                    || !paths
                        .iter()
                        .any(|path| held.starts_with(path) && !path.is_dir())
            });
            shrank |= folder.walked.len() != before;
        }
        if shrank {
            self.settle();
        }
    }

    /// Watches the folders among `paths` that were just made inside a watched folder.
    fn grown(&mut self, paths: &[PathBuf]) {
        let mut grew = false;
        for folder in self.folders.values_mut() {
            for path in paths {
                grew |= folder.grown(path);
            }
        }
        if grew {
            self.settle();
        }
    }

    /// Notes the folders a change of `paths` concerns as changed at `now`.
    fn noted(&mut self, paths: &[PathBuf], now: Instant) {
        for (directory, folder) in &self.folders {
            if paths.iter().any(|path| folder.concerns(path)) {
                self.changed
                    .entry(directory.clone())
                    .and_modify(|changed| changed.last = now)
                    .or_insert(Changed {
                        first: now,
                        last: now,
                    });
            }
        }
    }

    /// The folders whose changes are due to be told at `now`, each with the clients watching it, taken off what has changed.
    fn due(&mut self, now: Instant) -> Vec<(String, Vec<Arc<Connection>>)> {
        let ready: Vec<String> = self
            .changed
            .iter()
            .filter(|(_, changed)| now - changed.last >= QUIET || now - changed.first >= LONGEST)
            .map(|(directory, _)| directory.clone())
            .collect();
        ready
            .into_iter()
            .map(|directory| {
                self.changed.remove(&directory);
                let watchers = self
                    .wanted
                    .iter()
                    .filter(|(_, directories)| directories.contains(&directory))
                    .map(|(connection, _)| connection.clone())
                    .collect();
                (directory, watchers)
            })
            .collect()
    }
}

/// Starts the thread that reads what the watcher reports and tells each client when a folder it watches changed.
pub(crate) fn start(core: Arc<Core>) {
    let Some(reports) = core
        .folders
        .reports
        .lock()
        .ok()
        .and_then(|mut held| held.take())
    else {
        return;
    };
    std::thread::spawn(move || {
        loop {
            match reports.recv_timeout(TICK) {
                Ok(Ok(event)) if !matches!(event.kind, EventKind::Access(_)) => {
                    if let Ok(mut watching) = core.folders.watching.lock() {
                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)) => {
                                watching.shrunk(&event.paths);
                                watching.grown(&event.paths);
                            }
                            EventKind::Remove(_) => watching.shrunk(&event.paths),
                            _ => {}
                        }
                        watching.noted(&event.paths, Instant::now());
                    }
                }
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let due = match core.folders.watching.lock() {
                Ok(mut watching) => watching.due(Instant::now()),
                Err(_) => return,
            };
            for (directory, watchers) in due {
                for connection in watchers {
                    let _ = connection.outbox.send(ServerMessage::FolderChanged {
                        directory: directory.clone(),
                    });
                }
            }
        }
    });
}
