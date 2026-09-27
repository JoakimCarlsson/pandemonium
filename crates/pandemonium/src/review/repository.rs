//! One repository of a review: what git says about it, and what is being
//! done to it.
//!
//! A worktree can hold several repositories — a folder of services, each its
//! own repository — and each has its own head, its own index, its own
//! history and its own commit to write. What belongs to one repository lives
//! here; what the review lists across all of them lives in the review.

use std::path::{Path, PathBuf};
use std::time::Instant;

use pm_core::{Changed, Head, Status};

use crate::input::{Input, Submit};
use crate::review::reading::RepositoryReading;

/// How many commits of a repository's history are read at a time.
const HISTORY: usize = 500;

/// The commits leading up to a repository, under each of the graph's filters.
#[derive(Default)]
pub struct History {
    /// Commits on the checked out branch.
    auto: Vec<pm_core::Commit>,
    /// Commits reachable from every reference.
    all: Vec<pm_core::Commit>,
}

impl History {
    /// Reads the history of the repository at `root`.
    pub(super) fn of(root: &Path) -> Self {
        Self {
            auto: pm_core::history(root, HISTORY, false),
            all: pm_core::history(root, HISTORY, true),
        }
    }
}

/// One repository of a review, as the window last read it.
pub struct Repository {
    /// The repository's working-copy root.
    root: PathBuf,
    /// What the repository is called where the review lists it.
    name: String,
    /// What git makes of the repository.
    status: Status,
    /// What the repository's next commit will say, as a buffer like any other.
    ///
    /// The message is edited in the editor the window is made of rather than
    /// in a line of its own: a commit message is several lines, it is written
    /// with the cursor moved about and the text selected, and every one of
    /// those is something the editor already does.
    message: Input,
    /// What git said when it last would not do something here.
    trouble: Option<String>,
    /// What is being done with a remote right now, and since when.
    busy: Option<(&'static str, Instant)>,
    /// What git is being had do here right now, and since when.
    working: Option<(&'static str, Instant)>,
    /// The commits leading up to it.
    history: History,
    /// First visible commit in each history filter.
    history_scrolls: [usize; 2],
}

impl Repository {
    /// The repository at `root`, named from `within` — the worktree the
    /// review is of — and not read yet.
    pub(super) fn at(within: &Path, root: &Path) -> Self {
        let name = match root.strip_prefix(within) {
            Ok(relative) if !relative.as_os_str().is_empty() => relative.display().to_string(),
            _ => root.file_name().map_or_else(
                || root.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
        };

        Self {
            root: root.to_path_buf(),
            name,
            status: Status::default(),
            message: Input::many_lines("COMMIT_EDITMSG").submitting(Submit::Chord),
            trouble: None,
            busy: None,
            working: None,
            history: History::default(),
            history_scrolls: [0; 2],
        }
    }

    /// Takes in what git said the repository held and what led up to it.
    pub(super) fn take(&mut self, reading: RepositoryReading) {
        if self.unsaid()
            && let Some(pm_core::Operation::Merge(merge)) = &reading.status.head().operation
        {
            self.message.set(&merge.message);
        }
        self.status = reading.status;
        self.history = reading.history;
    }

    /// The repository's working-copy root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What the repository is called where the review lists it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What git makes of the repository.
    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Where the repository's head stands.
    pub fn head(&self) -> &Head {
        self.status.head()
    }

    /// Every file git has something to say about here, in path order.
    pub(super) fn changed(&self) -> &[Changed] {
        self.status.changed()
    }

    /// The box the next commit's message is written in.
    pub fn message(&self) -> &Input {
        &self.message
    }

    /// That box, to write in.
    pub(super) fn message_mut(&mut self) -> &mut Input {
        &mut self.message
    }

    /// What it holds.
    pub(super) fn said(&self) -> String {
        self.message.value()
    }

    /// Whether nothing has been written in it.
    pub(super) fn unsaid(&self) -> bool {
        self.said().trim().is_empty()
    }

    /// What git said when it last would not do something here.
    pub fn trouble(&self) -> Option<&str> {
        self.trouble.as_deref()
    }

    /// Takes in what git said, keeping what it complained about.
    pub(super) fn heard(&mut self, said: pm_core::Said) {
        self.trouble = said.err().filter(|said| !said.is_empty());
    }

    /// What is being done with a remote right now, and since when.
    pub(super) fn busy(&self) -> Option<(&'static str, Instant)> {
        self.busy
    }

    /// Marks a remote as being talked to, worded as `doing`, or as no
    /// longer being talked to.
    pub(super) fn set_busy(&mut self, doing: Option<&'static str>) {
        self.busy = doing.map(|doing| (doing, Instant::now()));
    }

    /// What git is being had do here right now, and since when.
    pub(super) fn working(&self) -> Option<(&'static str, Instant)> {
        self.working
    }

    /// Marks git as doing something here, worded as `doing`, or as done.
    pub(super) fn set_working(&mut self, doing: Option<&'static str>) {
        self.working = doing.map(|doing| (doing, Instant::now()));
    }

    /// The cached commits selected by the Source Control graph filter.
    pub(super) fn history(&self, all: bool) -> &[pm_core::Commit] {
        match all {
            true => &self.history.all,
            false => &self.history.auto,
        }
    }

    /// The first visible commit under the selected history filter.
    pub(super) fn history_scroll(&self, all: bool, visible: usize) -> usize {
        let total = self.history(all).len();
        self.history_scrolls[usize::from(all)].min(total.saturating_sub(visible.max(1)))
    }

    /// Scrolls the selected history filter within the commits it has read.
    pub(super) fn scroll_history(&mut self, all: bool, rows: isize, visible: usize) {
        let index = usize::from(all);
        let total = self.history(all).len();
        let last = total.saturating_sub(visible.max(1));
        self.history_scrolls[index] = self.history_scrolls[index]
            .saturating_add_signed(rows)
            .min(last);
    }
}
