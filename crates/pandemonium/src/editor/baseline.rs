//! What the index holds for each open file, read away from the window.
//!
//! A file's baseline is one `git show` of it, and a review opening the
//! documents of fifty changed files is fifty of them. So a document opens
//! with no baseline, and one thread behind the window reads each baseline
//! asked for in turn and wakes the window with it; the document marks where
//! it differs once it arrives.

use pm_host::Location;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use crate::editor::store::FileId;

/// One baseline asked for: whose it is, which asking it answers, and where.
struct Ask {
    /// The document it is for.
    file: FileId,
    /// Which of that document's askings it answers.
    asked: u64,
    /// The worktree the file is in.
    root: Location,
    /// The file itself.
    path: PathBuf,
}

/// A baseline that has been read and not yet taken in.
pub(super) type Read = (FileId, u64, Option<String>);

/// The thread reading baselines, and what it has read.
#[derive(Default)]
pub(super) struct Baselines {
    /// Where the thread is asked, once it has been started.
    asks: Option<Sender<Ask>>,
    /// What it has read and the window has not taken in yet.
    read: Arc<Mutex<Vec<Read>>>,
}

impl Baselines {
    /// Starts the thread, which wakes the window through `notify` with each
    /// baseline it has read.
    pub(super) fn start(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        let (asks, asked) = mpsc::channel::<Ask>();
        let read = self.read.clone();
        std::thread::spawn(move || {
            for ask in asked {
                let baseline = pm_core::baseline(&ask.root, &ask.path);
                if let Ok(mut read) = read.lock() {
                    read.push((ask.file, ask.asked, baseline));
                }
                notify();
            }
        });
        self.asks = Some(asks);
    }

    /// Asks for the baseline of `file`, at `path` in the worktree at `root`,
    /// as its `asked`-th asking; answers whether the thread took it, which it
    /// does not before it has been started.
    pub(super) fn ask(&self, file: FileId, asked: u64, root: &Location, path: &Path) -> bool {
        self.asks.as_ref().is_some_and(|asks| {
            asks.send(Ask {
                file,
                asked,
                root: root.clone(),
                path: path.to_path_buf(),
            })
            .is_ok()
        })
    }

    /// Takes every baseline that has been read since this was last asked.
    pub(super) fn take(&self) -> Vec<Read> {
        self.read
            .lock()
            .map(|mut read| std::mem::take(&mut *read))
            .unwrap_or_default()
    }
}
