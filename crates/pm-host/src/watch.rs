//! Following a worktree on disk, so the window hears about what others write.
//!
//! An agent, a formatter run from a shell, a `git checkout`: each of them
//! writes into a worktree without going through the editor. The watcher is
//! how the editor finds out. The platform's own notifications are gathered
//! on a thread of their own and settled for a moment before the window is
//! woken, so a tool that writes a file in ten pieces is one change to the
//! window rather than ten. What a path became is read from the disk after
//! that pause: a backend may report flags rather than the order things
//! happened in, and the disk is what the window follows.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use notify::event::{ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::ignore::Ignore;

/// How long the disk has to be quiet before what it did is handed on.
const SETTLE: Duration = Duration::from_millis(50);

/// The longest a stream of changes is held back before it is handed on anyway.
///
/// A build writing into the worktree for a minute is not quiet for fifty
/// milliseconds at a time, and the reader still wants to see the files it
/// has finished with.
const LONGEST: Duration = Duration::from_millis(500);

/// How often a thread with nothing to say checks whether it is still wanted.
const IDLE: Duration = Duration::from_millis(250);

/// The names inside a git directory whose change means git did something.
///
/// Objects and logs change on every commit too, but never alone: whatever
/// git does that the window cares about moves a ref, the index or `HEAD`.
const GIT_STATE: [&str; 4] = ["HEAD", "index", "refs", "packed-refs"];

/// What happened to one path.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Touch {
    /// It was not there, and now is.
    Created,
    /// It was there, and what it holds changed.
    Changed,
    /// It was there, and now is not.
    Removed,
}

impl Touch {
    /// What two touches of the same path, `self` and then `later`, add up to.
    ///
    /// A file made and then written is still a file that was made; one taken
    /// away and put back is a file that changed.
    fn then(self, later: Self) -> Self {
        match (self, later) {
            (Self::Created, Self::Changed) => Self::Created,
            (Self::Removed, Self::Created) => Self::Changed,
            (_, later) => later,
        }
    }
}

/// One path the disk changed, and what happened to it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Touched {
    /// Where the change was.
    pub path: PathBuf,
    /// What happened there.
    pub touch: Touch,
    /// Whether the worktree says the path is not worth following.
    ///
    /// The window still sees an ignored path, because the tree lists one,
    /// but a language server is not told about a build writing its output.
    pub ignored: bool,
}

/// Everything the disk did under one worktree since the window last asked.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Disk {
    /// The paths that changed, each once, in the order of their names.
    pub touched: Vec<Touched>,
    /// Whether git moved a ref, the index or `HEAD`.
    pub repository: bool,
}

impl Disk {
    /// Whether nothing happened at all.
    pub fn is_empty(&self) -> bool {
        self.touched.is_empty() && !self.repository
    }
}

/// What the watching thread and the window share.
#[derive(Default)]
struct Shared {
    /// What has settled and not yet been taken.
    settled: Mutex<Settled>,
    /// Whether the window has let the watcher go.
    stopped: AtomicBool,
}

/// What has settled, gathered path by path so a path is reported once.
#[derive(Default)]
struct Settled {
    /// The paths touched, with what touching them added up to.
    touched: BTreeMap<PathBuf, (Touch, bool)>,
    /// Whether git did something.
    repository: bool,
}

/// A worktree being followed on disk, for as long as this is held.
pub struct Watcher {
    /// What the watching thread has settled on.
    shared: Arc<Shared>,
}

impl Watcher {
    /// Follows everything under `root`, waking the window through `wake`
    /// whenever the disk has changed and settled.
    ///
    /// Watching a large worktree means registering every directory in it,
    /// which takes a while, so that happens on the watching thread too: the
    /// frame that opened the worktree does not wait on it.
    pub fn start(root: &Path, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let shared = Arc::new(Shared::default());
        let watching = Watching {
            root: root.to_path_buf(),
            shared: shared.clone(),
            wake,
        };
        std::thread::spawn(move || watching.run());
        Self { shared }
    }

    /// Creates a watcher with no active backend.
    pub(crate) fn empty() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
        }
    }

    /// Receives settled remote notifications until dropped or disconnected.
    pub(crate) fn remote(
        events: Receiver<crate::wire::Frame>,
        connection: Arc<crate::remote::Connection>,
        channel: u32,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let watcher = Self::empty();
        let shared = watcher.shared.clone();
        std::thread::spawn(move || {
            while !shared.stopped.load(Ordering::Relaxed) {
                match events.recv_timeout(IDLE) {
                    Ok(frame) => {
                        let _ = connection.credit(channel);
                        if let Ok(disk) = serde_json::from_slice::<Disk>(&frame.payload) {
                            let mut settled = shared.settled.lock().unwrap();
                            settled.repository |= disk.repository;
                            for touched in disk.touched {
                                settled
                                    .touched
                                    .entry(touched.path)
                                    .and_modify(|(was, _)| *was = was.then(touched.touch))
                                    .or_insert((touched.touch, touched.ignored));
                            }
                            drop(settled);
                            wake();
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => {
                        wake();
                        break;
                    }
                }
            }
            let _ = connection.request("unwatch", serde_json::json!({"channel":channel}));
        });
        watcher
    }

    /// Takes everything that has settled since this was last asked.
    pub fn take(&self) -> Disk {
        let Ok(mut settled) = self.shared.settled.lock() else {
            return Disk::default();
        };
        let settled = std::mem::take(&mut *settled);
        Disk {
            touched: settled
                .touched
                .into_iter()
                .map(|(path, (touch, ignored))| Touched {
                    path,
                    touch,
                    ignored,
                })
                .collect(),
            repository: settled.repository,
        }
    }
}

impl Drop for Watcher {
    /// Tells the watching thread to stop, which lets the platform's watch go.
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Relaxed);
    }
}

/// The watching thread: the worktree, the rules it keeps and who to wake.
struct Watching {
    /// The worktree being followed.
    root: PathBuf,
    /// What is shared with the window.
    shared: Arc<Shared>,
    /// How the window is woken once something has settled.
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Watching {
    /// Watches the worktree until the window lets it go.
    ///
    /// A worktree the platform will not watch is not followed, and says
    /// nothing about it: the editor works the way it did before it watched
    /// anything, which is by reading the disk when it is asked to.
    fn run(self) {
        let (sender, events) = mpsc::channel();
        let Ok(mut watcher) = RecommendedWatcher::new(sender, notify::Config::default()) else {
            return;
        };
        if watcher.watch(&self.root, RecursiveMode::Recursive).is_err() {
            return;
        }
        let mut ignore = Ignore::read(&self.root);

        while !self.shared.stopped.load(Ordering::Relaxed) {
            let batch = settle(&events);
            if batch.is_empty() {
                continue;
            }
            if batch
                .iter()
                .any(|(path, _)| path.file_name() == Some(OsStr::new(".gitignore")))
            {
                ignore = Ignore::read(&self.root);
            }
            self.publish(batch, &ignore);
        }
    }

    /// Adds `batch` to what has settled, and wakes the window if it matters.
    fn publish(&self, batch: Vec<(PathBuf, Touch)>, ignore: &Ignore) {
        let Ok(mut settled) = self.shared.settled.lock() else {
            return;
        };
        let mut news = false;
        for (path, touch) in batch {
            if let Some(git) = self.git_part(&path) {
                if is_git_state(git) {
                    settled.repository = true;
                    news = true;
                }
                continue;
            }
            let ignored = ignore.covers(&path);
            settled
                .touched
                .entry(path)
                .and_modify(|(was, _)| *was = was.then(touch))
                .or_insert((touch, ignored));
            news = true;
        }
        drop(settled);
        if news {
            (self.wake)();
        }
    }

    /// Where `path` is inside a git directory of the worktree, if it is inside
    /// one: the worktree's own, or that of a repository inside it.
    fn git_part<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        let git = path.ancestors().find(|ancestor| {
            ancestor.file_name() == Some(OsStr::new(".git")) && ancestor.starts_with(&self.root)
        })?;
        path.strip_prefix(git).ok()
    }
}

/// Whether a change at `inside`, within a git directory, means git did something.
///
/// A session's worktree keeps its own `HEAD` and index under
/// `worktrees/<name>` of the repository's git directory, which is watched
/// along with the checkout it belongs to.
fn is_git_state(inside: &Path) -> bool {
    let mut steps = inside.iter().filter_map(|step| step.to_str());
    let first = steps.next();
    let first = match first {
        Some("worktrees") => steps.nth(1),
        first => first,
    };
    first.is_some_and(|name| GIT_STATE.contains(&name))
}

/// Waits for the disk to change and then to go quiet, and says what it did.
///
/// Nothing having happened by the time the thread should check whether it
/// is still wanted comes back as nothing. Each path comes back once, as
/// what the disk holds now rather than as the order the notices arrived.
fn settle(events: &Receiver<notify::Result<Event>>) -> Vec<(PathBuf, Touch)> {
    let mut batch = Vec::new();
    let first = match events.recv_timeout(IDLE) {
        Ok(event) => event,
        Err(RecvTimeoutError::Timeout) => return batch,
        Err(RecvTimeoutError::Disconnected) => {
            std::thread::sleep(IDLE);
            return batch;
        }
    };
    let began = Instant::now();
    gather(first, &mut batch);
    while began.elapsed() < LONGEST {
        match events.recv_timeout(SETTLE) {
            Ok(event) => gather(event, &mut batch),
            Err(_) => break,
        }
    }
    on_disk(batch)
}

/// One touch per path in `noticed`, taken from the disk.
///
/// The notices are folded first. A path that is not there was removed,
/// whatever they said. One that is there and that the fold says was removed
/// was taken away and put back, so it changed. One the fold says was created
/// stays created, which is what shows it in the tree.
fn on_disk(noticed: Vec<(PathBuf, Touch)>) -> Vec<(PathBuf, Touch)> {
    let mut folded: BTreeMap<PathBuf, Touch> = BTreeMap::new();
    for (path, touch) in noticed {
        folded
            .entry(path)
            .and_modify(|was| *was = was.then(touch))
            .or_insert(touch);
    }
    folded
        .into_iter()
        .map(|(path, folded)| {
            let touch = read_touch(&path, folded);
            (path, touch)
        })
        .collect()
}

/// What `folded` means for `path` once it has been looked up.
fn read_touch(path: &Path, folded: Touch) -> Touch {
    if !present(path) {
        return Touch::Removed;
    }
    match folded {
        Touch::Removed => Touch::Changed,
        touch => touch,
    }
}

/// Whether `path` is on disk, counting a dangling symlink as present.
fn present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Adds what `event` says happened to `batch`.
fn gather(event: notify::Result<Event>, batch: &mut Vec<(PathBuf, Touch)>) {
    let Ok(event) = event else {
        return;
    };
    match event.kind {
        EventKind::Access(_) => {}
        EventKind::Create(_) => batch.extend(event.paths.into_iter().map(created)),
        EventKind::Remove(_) => batch.extend(event.paths.into_iter().map(removed)),
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            batch.extend(event.paths.into_iter().map(removed));
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            batch.extend(event.paths.into_iter().map(created));
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            let mut paths = event.paths.into_iter();
            batch.extend(paths.next().map(removed));
            batch.extend(paths.next().map(created));
        }
        EventKind::Modify(ModifyKind::Name(_)) => {
            batch.extend(event.paths.into_iter().map(|path| match present(&path) {
                true => created(path),
                false => removed(path),
            }));
        }
        _ => batch.extend(event.paths.into_iter().map(|path| (path, Touch::Changed))),
    }
}

/// `path`, as a path that was made.
fn created(path: PathBuf) -> (PathBuf, Touch) {
    (path, Touch::Created)
}

/// `path`, as a path that was taken away.
fn removed(path: PathBuf) -> (PathBuf, Touch) {
    (path, Touch::Removed)
}
