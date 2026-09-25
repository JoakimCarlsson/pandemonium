//! What one worktree has changed, and everything done about it.
//!
//! This is the one seam between the window and git's index: the sidebar, the
//! review pane and a keybinding all stage, unstage, throw away and commit
//! through here, and each of them is followed by asking git again rather
//! than by guessing what the answer would now be. Git is the state; this is
//! what the window last read of it.
//!
//! A worktree may hold several repositories. The changes of all of them are
//! one list, in the order the repositories are found, and every command over
//! a set of files is carried out in each repository over the files that are
//! its own. What only one repository can do — commit, sync, switch branch —
//! is done in the active one, which the sidebar's sections choose.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use pm_core::{Changed, FileStatus, Head, Hunk, Line};
use pm_text::{Buffer, Highlight};

use crate::input::Input;
use crate::review::reading::{self, Reading, RepositoryReading};
use crate::review::repository::Repository;
use crate::review::shade::{Shading, Version};

/// What the commit button says while there is nothing to commit.
const NOTHING_TO_COMMIT: &str = "Nothing to commit";

/// How long the refresh control takes to turn once round after a press.
const REFRESH_TURN: std::time::Duration = std::time::Duration::from_millis(700);

/// What the button under the commit message does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Primary {
    /// Commits, under `title`, unless `stopped` says why it cannot.
    Commit {
        /// What the button calls the commit it would make.
        title: String,
        /// Why it cannot commit, when it cannot.
        stopped: Option<&'static str>,
    },
    /// Pulls what the branch is behind by and pushes what it is ahead by.
    Sync {
        /// Commits this branch has that the one it follows has not.
        ahead: usize,
        /// Commits the branch it follows has that this one has not.
        behind: usize,
    },
    /// Pushes a branch that follows nothing, and makes it follow what it
    /// was pushed to.
    Publish,
    /// Waits on a remote, saying what it is `doing`.
    Busy {
        /// What is being done, as the button words it: "Syncing…".
        doing: &'static str,
        /// When it began, which the spinner turns from.
        since: Instant,
    },
}

/// One changed file's identity for as long as the window is open.
///
/// A pane showing one file's diff names it with this rather than with its
/// path: a path is not something a tab can carry, and the id survives the
/// file being staged, unstaged and typed into again under it.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChangeId(u64);

/// One range of rows being marked out by a single gesture.
///
/// A gesture holds what was marked before it began, so that every extension
/// of it is the same shape: the marks it started from, plus everything
/// between where it was anchored and where it has reached. Without that, a
/// range dragged back on itself would leave the rows it had passed marked.
struct Gesture {
    /// The row the range is measured from.
    anchor: ChangeId,
    /// What was marked when the gesture began.
    base: BTreeSet<ChangeId>,
}

/// Which of the sidebar's lists a row sits in.
///
/// A file is in exactly one of them, and which one is what it is rather than
/// what has been done with it: whether it is staged is the box on its row,
/// not the list it is in. That is Zed's arrangement, and it is what makes
/// staging a file leave it where the reader last saw it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    /// Changed on both sides of a merge.
    Conflicted,
    /// Changed, and git knows the file.
    Tracked,
    /// New, and git has never been told about it.
    Untracked,
}

impl Group {
    /// Every group, in the order the sidebar lists them.
    pub const ALL: [Self; 3] = [Self::Conflicted, Self::Tracked, Self::Untracked];

    /// What the heading above the group says.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Conflicted => "CONFLICTS",
            Self::Tracked => "TRACKED",
            Self::Untracked => "UNTRACKED",
        }
    }

    /// Whether `changed` belongs in this group.
    fn holds(self, changed: &Changed) -> bool {
        match self {
            Self::Conflicted => changed.is_conflicted(),
            Self::Tracked => !changed.is_conflicted() && !changed.is_untracked(),
            Self::Untracked => !changed.is_conflicted() && changed.is_untracked(),
        }
    }
}

/// The lines of one file's change, on each side of the index.
#[derive(Default)]
pub struct Patch {
    /// What the index holds that the last commit does not.
    pub staged: Vec<Hunk>,
    /// What the worktree holds that the index does not.
    pub unstaged: Vec<Hunk>,
}

/// What one worktree has changed, as the window last read it.
pub struct Review {
    /// The worktree this is a review of.
    root: PathBuf,
    /// The repositories the worktree holds, the root's own first.
    repositories: Vec<Repository>,
    /// The repository a commit, a sync or a branch is made in.
    active: usize,
    /// Every file that has changed, repository by repository.
    changed: Vec<Changed>,
    /// The repository each of those files is in, by its place in the list.
    owners: Vec<usize>,
    /// The lines of each file that has changed.
    patches: BTreeMap<PathBuf, Patch>,
    /// The colour of every character each file's lines are drawn in.
    shades: BTreeMap<PathBuf, Shading>,
    /// The files whose lines are folded away in the review pane.
    collapsed: BTreeSet<PathBuf>,
    /// The id each file that has ever changed here was given.
    ///
    /// A path keeps its id for as long as the window is open, whether or not
    /// it still differs from the last commit: a tab showing the diff of a
    /// file that has just been committed still knows which file it was of.
    ids: BTreeMap<PathBuf, ChangeId>,
    /// The id the next file to change will be given.
    next: u64,
    /// The diffs that are being kept open rather than previewed.
    kept: BTreeSet<ChangeId>,
    /// The row the keyboard is on, which is what a command acts on alone.
    selected: Option<ChangeId>,
    /// The rows marked alongside it, which a command acts on instead.
    marked: BTreeSet<ChangeId>,
    /// The range being drawn out by a gesture, while one is being drawn.
    gesture: Option<Gesture>,
    /// The first row each pane of this review is drawn from.
    ///
    /// The whole review scrolls apart from each file's own diff, so what is
    /// scrolled is named by what the pane is showing: nothing for the review
    /// itself, the file for one of its diffs.
    scrolls: BTreeMap<Option<ChangeId>, usize>,
    /// When the reader last asked for the worktree to be read again, while
    /// the refresh control is still turning for it.
    refreshed: Option<Instant>,
    /// How many readings have been taken in, which names the one a read
    /// started now will answer.
    reads: u64,
}

impl Review {
    /// The changes of the worktree at `root`, read now.
    pub fn of(root: &Path) -> Self {
        let mut review = Self {
            root: root.to_path_buf(),
            repositories: Vec::new(),
            active: 0,
            changed: Vec::new(),
            owners: Vec::new(),
            patches: BTreeMap::new(),
            shades: BTreeMap::new(),
            collapsed: BTreeSet::new(),
            ids: BTreeMap::new(),
            next: 0,
            kept: BTreeSet::new(),
            selected: None,
            marked: BTreeSet::new(),
            gesture: None,
            scrolls: BTreeMap::new(),
            refreshed: None,
            reads: 0,
        };
        review.reread();
        review
    }

    /// Starts the refresh control turning, as the reader has just asked for
    /// the worktree to be read again.
    pub fn start_refresh(&mut self) {
        self.refreshed = Some(Instant::now());
    }

    /// How far round the refresh control has turned, in radians, while it is
    /// turning.
    ///
    /// Git answers a refresh within a frame, so the control turns once in
    /// full however quick the answer was: a press that shows nothing reads
    /// as a press that did nothing.
    pub fn refresh_turn(&self) -> Option<f32> {
        let elapsed = self.refreshed?.elapsed();
        (elapsed < REFRESH_TURN)
            .then(|| elapsed.as_secs_f32() / REFRESH_TURN.as_secs_f32() * std::f32::consts::TAU)
    }

    /// Asks git again what the worktree holds, and waits for the answer.
    ///
    /// This is for what the reader has just done here, whose result the
    /// next frame has to show; what the disk did on its own is read through
    /// [`Review::read_later`] instead.
    pub fn reread(&mut self) {
        let reading = Reading::of(&self.root, self.reads);
        self.take(reading);
    }

    /// What reads the worktree again, on whichever thread it is called on.
    ///
    /// The repositories are looked for again first, so one cloned into the
    /// folder is reviewed without reopening it. Each repository is asked
    /// about whole, because that is what git answers in one go: a file at a
    /// time would be a subprocess per row of a list as long as the change is.
    pub fn read_later(&self) -> impl FnOnce() -> Reading + Send + 'static {
        let root = self.root.clone();
        let reads = self.reads;
        move || Reading::of(&root, reads)
    }

    /// Takes in what git said the worktree held, unless the review has been
    /// read again since that was asked; answers whether it was taken.
    ///
    /// A repository that was already here keeps its message.
    pub fn take(&mut self, reading: Reading) -> bool {
        if reading.reads != self.reads {
            return false;
        }
        self.reads += 1;
        self.find_repositories(reading.repositories);
        self.gather_changes();

        self.patches = reading.patches;
        self.shades = reading.shades;
        self.collapsed
            .retain(|path| self.patches.contains_key(path));
        for path in self.paths(|_| true) {
            self.name(&path);
        }
        let listed = self.listed();
        self.marked.retain(|id| listed.contains(id));
        self.selected = self
            .selected
            .filter(|id| listed.contains(id))
            .or_else(|| listed.first().copied());
        self.gesture = None;
        true
    }

    /// Puts the repositories `read` found in place, keeping what the ones
    /// still there were holding and the active one where it was.
    fn find_repositories(&mut self, read: Vec<RepositoryReading>) {
        let active = self.active_root().map(Path::to_path_buf);
        let mut held = std::mem::take(&mut self.repositories);
        self.repositories = read
            .into_iter()
            .map(|reading| {
                let mut repository = match held.iter().position(|kept| kept.root() == reading.root)
                {
                    Some(at) => held.swap_remove(at),
                    None => Repository::at(&self.root, &reading.root),
                };
                repository.take(reading);
                repository
            })
            .collect();
        self.active = active
            .and_then(|active| {
                self.repositories
                    .iter()
                    .position(|repository| repository.root() == active)
            })
            .unwrap_or_default();
    }

    /// Lists every repository's changes as one.
    fn gather_changes(&mut self) {
        (self.owners, self.changed) = reading::gather(
            self.repositories
                .iter()
                .map(|repository| (repository.root(), repository.changed())),
        )
        .into_iter()
        .unzip();
    }

    /// The repositories the worktree holds, the root's own first.
    pub fn repositories(&self) -> &[Repository] {
        &self.repositories
    }

    /// The `index`-th repository.
    pub fn repository(&self, index: usize) -> Option<&Repository> {
        self.repositories.get(index)
    }

    /// Which repository a commit, a sync or a branch is made in.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Makes the `index`-th repository the one a commit is made in.
    pub fn activate(&mut self, index: usize) {
        if index < self.repositories.len() {
            self.active = index;
        }
    }

    /// The repository a commit is made in, while the worktree holds one.
    fn active_repository(&self) -> Option<&Repository> {
        self.repositories.get(self.active)
    }

    /// That repository, to change.
    fn active_repository_mut(&mut self) -> Option<&mut Repository> {
        self.repositories.get_mut(self.active)
    }

    /// Where the repository a commit is made in sits on disk.
    pub fn active_root(&self) -> Option<&Path> {
        self.active_repository().map(Repository::root)
    }

    /// Which repository `path` is in.
    fn owner_of(&self, path: &Path) -> Option<usize> {
        self.changed
            .iter()
            .position(|changed| changed.path == path)
            .map(|at| self.owners[at])
    }

    /// What git makes of `path`, in whichever repository it is in.
    pub fn mark(&self, path: &Path) -> Option<FileStatus> {
        self.repositories
            .iter()
            .rev()
            .find_map(|repository| repository.status().mark(path))
    }

    /// The cached commits of the active repository selected by the Source
    /// Control graph filter.
    pub fn history(&self, all: bool) -> &[pm_core::Commit] {
        self.active_repository()
            .map_or(&[], |repository| repository.history(all))
    }

    /// The first visible commit under the selected history filter.
    pub fn history_scroll(&self, all: bool, visible: usize) -> usize {
        self.active_repository()
            .map_or(0, |repository| repository.history_scroll(all, visible))
    }

    /// Scrolls the selected history filter within the commits it has read.
    pub fn scroll_history(&mut self, all: bool, rows: isize, visible: usize) {
        if let Some(repository) = self.active_repository_mut() {
            repository.scroll_history(all, rows, visible);
        }
    }

    /// The id `path` goes by, giving it one if it has never had one.
    pub fn name(&mut self, path: &Path) -> ChangeId {
        if let Some(id) = self.ids.get(path) {
            return *id;
        }
        let id = ChangeId(self.next);
        self.next += 1;
        self.ids.insert(path.to_path_buf(), id);
        id
    }

    /// The id the `index`-th file that has changed goes by.
    pub fn id_of(&self, index: usize) -> Option<ChangeId> {
        self.ids.get(&self.change(index)?.path).copied()
    }

    /// Where the file `id` names lives.
    pub fn path_of(&self, id: ChangeId) -> Option<&Path> {
        self.ids
            .iter()
            .find(|(_, named)| **named == id)
            .map(|(path, _)| path.as_path())
    }

    /// Where the file `id` names sits in the list of changes.
    pub fn place_of(&self, id: ChangeId) -> Option<usize> {
        let path = self.path_of(id)?;
        self.changed()
            .iter()
            .position(|changed| changed.path == path)
    }

    /// Whether the diff of `id` is only being looked at, not kept open.
    pub fn is_preview(&self, id: ChangeId) -> bool {
        !self.kept.contains(&id)
    }

    /// Keeps the diff of `id` open, so the next one opened takes its own tab.
    pub fn keep(&mut self, id: ChangeId) {
        self.kept.insert(id);
    }

    /// How many files of the `repository`-th that git already knows about
    /// have changed.
    ///
    /// These are what a commit takes when nothing has been staged: a commit
    /// can take a tracked file's changes without being told to, and can never
    /// take a file git has not been told about.
    fn tracked(&self, repository: usize) -> usize {
        self.changed_in(repository)
            .filter(|changed| !changed.is_untracked() && !changed.is_conflicted())
            .count()
    }

    /// The files of the `repository`-th that have changed.
    fn changed_in(&self, repository: usize) -> impl Iterator<Item = &Changed> {
        self.changed
            .iter()
            .zip(&self.owners)
            .filter(move |(_, owner)| **owner == repository)
            .map(|(changed, _)| changed)
    }

    /// What the control offering to commit the `repository`-th says, and
    /// whether it can.
    ///
    /// The words are Zed's: with something staged it commits that, and with
    /// nothing staged it offers to take every tracked change instead. What
    /// stops it saying so — a conflict, no message, nothing to commit — is
    /// what the control says in its place.
    pub fn committable(&self, repository: usize) -> (String, Option<&'static str>) {
        let staged = self.staged_of(repository);
        let title = match staged {
            0 => "Commit Tracked".to_owned(),
            _ => "Commit".to_owned(),
        };
        let unsaid = self
            .repositories
            .get(repository)
            .is_none_or(Repository::unsaid);

        let stopped = if self.changed_in(repository).any(Changed::is_conflicted) {
            Some("Resolve the conflicts before committing")
        } else if staged == 0 && self.tracked(repository) == 0 {
            Some(NOTHING_TO_COMMIT)
        } else if unsaid {
            Some("No commit message")
        } else {
            None
        };
        (title, stopped)
    }

    /// What the button under the `repository`-th message does now.
    ///
    /// The words are VS Code's: while there is something to commit it
    /// commits, and once there is nothing it offers to bring the branch level
    /// with the one it follows — to sync what the two have drifted apart by,
    /// or to publish a branch that follows nothing yet. While a remote is
    /// being talked to, it says so and waits.
    pub fn primary(&self, repository: usize) -> Primary {
        let Some(held) = self.repositories.get(repository) else {
            return Primary::Commit {
                title: "Commit".to_owned(),
                stopped: Some(NOTHING_TO_COMMIT),
            };
        };
        if let Some((doing, since)) = held.busy() {
            return Primary::Busy { doing, since };
        }
        let (title, stopped) = self.committable(repository);
        if stopped != Some(NOTHING_TO_COMMIT) {
            return Primary::Commit { title, stopped };
        }

        let head = held.head();
        match (&head.branch, &head.upstream) {
            (Some(_), None) if head.commit.is_some() => Primary::Publish,
            (Some(_), Some(_)) if head.ahead + head.behind > 0 => Primary::Sync {
                ahead: head.ahead,
                behind: head.behind,
            },
            _ => Primary::Commit { title, stopped },
        }
    }

    /// How many files have something staged for the next commit, in every
    /// repository.
    pub fn staged(&self) -> usize {
        self.changed
            .iter()
            .filter(|changed| changed.is_staged())
            .count()
    }

    /// How many files of the `repository`-th have something staged.
    fn staged_of(&self, repository: usize) -> usize {
        self.changed_in(repository)
            .filter(|changed| changed.is_staged())
            .count()
    }

    /// How many files of `group` in the `repository`-th are staged, and how
    /// many there are.
    ///
    /// This is what the box on the group's heading says: none of them, all of
    /// them, or somewhere in between.
    pub fn staged_in(&self, repository: usize, group: Group) -> (usize, usize) {
        let rows = self.grouped(repository, group);
        let staged = rows
            .iter()
            .filter_map(|index| self.change(*index))
            .filter(|changed| changed.is_staged() && !changed.is_unstaged())
            .count();
        (staged, rows.len())
    }

    /// The files of `group` in the `repository`-th, in the order the sidebar
    /// lists them.
    pub fn grouped(&self, repository: usize, group: Group) -> Vec<usize> {
        self.changed
            .iter()
            .zip(&self.owners)
            .enumerate()
            .filter(|(_, (changed, owner))| **owner == repository && group.holds(changed))
            .map(|(index, _)| index)
            .collect()
    }

    /// How many files of the `repository`-th have changed.
    pub fn grouped_count(&self, repository: usize) -> usize {
        self.changed_in(repository).count()
    }

    /// Every row the sidebar draws, in the order it draws them: repository
    /// by repository, and group by group within each.
    fn listed(&self) -> Vec<ChangeId> {
        (0..self.repositories.len())
            .flat_map(|repository| {
                Group::ALL
                    .into_iter()
                    .flat_map(move |group| self.grouped(repository, group))
            })
            .filter_map(|index| self.id_of(index))
            .collect()
    }

    /// The row the keyboard is on.
    pub fn selected(&self) -> Option<ChangeId> {
        self.selected
    }

    /// Whether `id` is marked alongside it.
    pub fn is_marked(&self, id: ChangeId) -> bool {
        self.marked.contains(&id)
    }

    /// Whether anything is marked at all.
    pub fn has_marks(&self) -> bool {
        !self.marked.is_empty()
    }

    /// Forgets every mark, leaving the row the keyboard is on where it is.
    pub fn clear_marks(&mut self) {
        self.marked.clear();
        self.gesture = None;
    }

    /// Puts the keyboard on `id` alone, forgetting every mark.
    pub fn select(&mut self, id: ChangeId) {
        self.clear_marks();
        self.selected = Some(id);
    }

    /// Puts the keyboard on `id`, leaving every mark where it is.
    pub fn selected_is(&mut self, id: ChangeId) {
        self.selected = Some(id);
    }

    /// Marks `id`, or takes the mark off it, and puts the keyboard on it.
    ///
    /// Marking a second row marks the first one too: a reader who marks one
    /// row while another is selected means the two of them, not the one they
    /// just pointed at.
    pub fn toggle_mark(&mut self, id: ChangeId) {
        self.gesture = None;
        if !self.has_marks()
            && let Some(selected) = self.selected.filter(|selected| *selected != id)
        {
            self.marked.insert(selected);
        }
        if !self.marked.remove(&id) {
            self.marked.insert(id);
        }
        self.selected = Some(id);
    }

    /// Marks every row between the one the keyboard is on and `id`.
    ///
    /// The range is drawn from where the gesture was anchored rather than
    /// from wherever the keyboard has reached, so dragging one out and back
    /// again leaves behind only what it still covers.
    pub fn mark_to(&mut self, id: ChangeId) {
        let anchor = match &self.gesture {
            Some(gesture) => gesture.anchor,
            None => {
                let anchor = self.selected.unwrap_or(id);
                self.gesture = Some(Gesture {
                    anchor,
                    base: self.marked.clone(),
                });
                anchor
            }
        };

        let listed = self.listed();
        let (Some(from), Some(to)) = (
            listed.iter().position(|row| *row == anchor),
            listed.iter().position(|row| *row == id),
        ) else {
            return;
        };
        let (first, last) = (from.min(to), from.max(to));

        self.marked = match &self.gesture {
            Some(gesture) => gesture.base.clone(),
            None => BTreeSet::new(),
        };
        self.marked.extend(&listed[first..=last]);
        self.selected = Some(id);
    }

    /// Moves the keyboard `steps` down the list, marking on the way if asked.
    pub fn step(&mut self, steps: isize, marking: bool) {
        let listed = self.listed();
        if listed.is_empty() {
            return;
        }
        let at = self
            .selected
            .and_then(|selected| listed.iter().position(|row| *row == selected))
            .map_or(0, |at| {
                (at as isize + steps).clamp(0, listed.len() as isize - 1) as usize
            });
        let Some(reached) = listed.get(at).copied() else {
            return;
        };

        match marking {
            true => self.mark_to(reached),
            false => {
                self.gesture = None;
                self.selected = Some(reached);
            }
        }
    }

    /// The files from the row the keyboard is on to the `index`-th, and both.
    ///
    /// This is what shift held on a box asks for: a sweep from wherever the
    /// last one was ticked to this one, so a run of files goes in together
    /// without a click each.
    pub fn between(&self, index: usize) -> Vec<ChangeId> {
        let listed = self.listed();
        let Some(id) = self.id_of(index) else {
            return Vec::new();
        };
        let Some(target) = listed.iter().position(|row| *row == id) else {
            return vec![id];
        };
        let anchor = self
            .selected
            .and_then(|selected| listed.iter().position(|row| *row == selected))
            .unwrap_or(target);
        let (first, last) = (anchor.min(target), anchor.max(target));
        listed[first..=last].to_vec()
    }

    /// The files a command acts on: what is marked, or the row it is on.
    ///
    /// A single mark on the row the keyboard is already on is not a
    /// selection of one thing said twice, so it reads as the row itself —
    /// which is what keeps marking a row and then acting on it from meaning
    /// something different than acting on it would have.
    pub fn acting_on(&self) -> Vec<ChangeId> {
        let real = match self.marked.len() {
            0 => false,
            1 => self
                .selected
                .is_none_or(|selected| !self.is_marked(selected)),
            _ => true,
        };
        if !real {
            return self.selected.into_iter().collect();
        }
        self.listed()
            .into_iter()
            .filter(|row| self.marked.contains(row))
            .collect()
    }

    /// The worktree this is a review of.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the active repository's head stands, while there is one.
    pub fn head(&self) -> Option<&Head> {
        self.active_repository().map(Repository::head)
    }

    /// Every file that has changed, in the order the lists show them.
    pub fn changed(&self) -> &[Changed] {
        &self.changed
    }

    /// The `index`-th file that has changed.
    pub fn change(&self, index: usize) -> Option<&Changed> {
        self.changed().get(index)
    }

    /// The lines `path` has changed, on each side of the index.
    pub fn patch(&self, path: &Path) -> Option<&Patch> {
        self.patches.get(path)
    }

    /// The highlights of `line`, shown on the `staged` side of `path`'s diff.
    pub fn shade(&self, path: &Path, staged: bool, line: &Line) -> Option<&[Option<Highlight>]> {
        let (version, number) = Version::of(staged, line)?;
        self.shades.get(path)?.line(version, number)
    }

    /// Colours the worktree's lines of the file `buffer` holds again from
    /// it, what a language server has said about its names included.
    ///
    /// A buffer with edits that are not on disk is not what the diff was
    /// read from, so its lines are left as the disk had them.
    pub fn repaint_worktree(&mut self, buffer: &mut Buffer) {
        if buffer.is_dirty() {
            return;
        }
        let path = buffer.path().to_path_buf();
        let (Some(patch), Some(shading)) = (self.patches.get(&path), self.shades.get_mut(&path))
        else {
            return;
        };
        shading.repaint_worktree(patch, buffer);
    }

    /// Whether the lines of `path` are folded away in the review pane.
    pub fn is_collapsed(&self, path: &Path) -> bool {
        self.collapsed.contains(path)
    }

    /// Folds the lines of the `index`-th file away, or shows them again.
    pub fn toggle(&mut self, index: usize) {
        let Some(path) = self.change(index).map(|changed| changed.path.clone()) else {
            return;
        };
        if !self.collapsed.remove(&path) {
            self.collapsed.insert(path);
        }
    }

    /// The box the active repository's next commit message is written in.
    pub fn message(&self) -> Option<&Input> {
        self.active_repository().map(Repository::message)
    }

    /// That box, to write in.
    pub fn message_mut(&mut self) -> Option<&mut Input> {
        self.active_repository_mut().map(Repository::message_mut)
    }

    /// What git said when it last would not do something in the active
    /// repository.
    pub fn trouble(&self) -> Option<&str> {
        self.active_repository().and_then(Repository::trouble)
    }

    /// Marks a remote as being talked to from the active repository, worded
    /// as `doing`.
    pub fn begin(&mut self, doing: &'static str) {
        if let Some(repository) = self.active_repository_mut() {
            repository.set_busy(Some(doing));
        }
    }

    /// Takes in what a remote said, in the repository that was talking to
    /// it, and reads the worktree again.
    pub fn settle(&mut self, said: pm_core::Said) {
        let talking = self
            .repositories
            .iter()
            .position(|repository| repository.busy().is_some())
            .unwrap_or(self.active);
        for repository in &mut self.repositories {
            repository.set_busy(None);
        }
        self.done(talking, said);
    }

    /// Records what a Git operation outside the review said about the active
    /// repository, and rereads it.
    pub fn report(&mut self, said: pm_core::Said) {
        self.done(self.active, said);
    }

    /// The first row the pane showing `shown` is drawn from.
    pub fn scroll(&self, shown: Option<ChangeId>) -> usize {
        self.scrolls.get(&shown).copied().unwrap_or_default()
    }

    /// Puts the pane showing `shown` at `row`.
    pub fn scroll_to(&mut self, shown: Option<ChangeId>, row: usize) {
        self.scrolls.insert(shown, row);
    }

    /// Scrolls that pane by `rows`, as far as there are rows to show.
    ///
    /// How far it can go is not known here — the pane is as tall as the
    /// window made it — so it is held against the rows there are and the
    /// pane clips whatever is left over.
    pub fn scroll_by(&mut self, shown: Option<ChangeId>, rows: isize, total: usize) {
        let at = self.scroll(shown);
        let reached = at.saturating_add_signed(rows).min(total.saturating_sub(1));
        self.scrolls.insert(shown, reached);
    }

    /// The files `ids` names that `wanted` accepts, with the repository
    /// each is in.
    ///
    /// An id naming a file that has stopped differing is left out rather
    /// than passed on: the list is read again after every change, and a
    /// command carried out over a stale row is a command over one file less.
    fn named_where(
        &self,
        ids: &[ChangeId],
        wanted: impl Fn(&Changed) -> bool,
    ) -> Vec<(usize, PathBuf)> {
        ids.iter()
            .filter_map(|id| self.place_of(*id))
            .filter(|place| self.change(*place).is_some_and(&wanted))
            .map(|place| (self.owners[place], self.changed[place].path.clone()))
            .collect()
    }

    /// Carries `command` out in each repository over the files of `named`
    /// that are its own, and reads the worktree again once.
    ///
    /// Every repository's files go in one call, because staging four files
    /// is one thing the reader asked for and should be one thing git is told
    /// — and a half-finished bulk change is the state nobody can reason
    /// about.
    fn in_each(
        &mut self,
        named: Vec<(usize, PathBuf)>,
        command: impl Fn(&Path, &[PathBuf]) -> pm_core::Said,
    ) {
        if named.is_empty() {
            return;
        }
        for repository in 0..self.repositories.len() {
            let paths = named
                .iter()
                .filter(|(owner, _)| *owner == repository)
                .map(|(_, path)| path.clone())
                .collect::<Vec<_>>();
            if paths.is_empty() {
                continue;
            }
            let said = command(self.repositories[repository].root(), &paths);
            self.repositories[repository].heard(said);
        }
        self.reread();
    }

    /// Puts the files `ids` names into the index.
    pub fn stage(&mut self, ids: &[ChangeId]) {
        let named = self.named_where(ids, Changed::is_unstaged);
        self.in_each(named, pm_core::stage);
    }

    /// Takes the files `ids` names back out of the index.
    pub fn unstage(&mut self, ids: &[ChangeId]) {
        let named = self.named_where(ids, Changed::is_staged);
        self.in_each(named, pm_core::unstage);
    }

    /// Puts everything that has changed into the index.
    pub fn stage_all(&mut self) {
        let named = self.owned(Changed::is_unstaged);
        self.in_each(named, pm_core::stage);
    }

    /// Takes everything back out of the index.
    pub fn unstage_all(&mut self) {
        let named = self.owned(Changed::is_staged);
        self.in_each(named, pm_core::unstage);
    }

    /// Puts the files `ids` names back the way the last commit had them.
    ///
    /// What is staged is taken back out of the index first, because a file
    /// half in the index is still a file with changes in it: throwing a
    /// change away means the whole of it, and what the reader is looking at
    /// is the file, not one side of it. What is left is then two commands
    /// rather than one — the files the last commit had are put back from it,
    /// and the files it never had are taken off the disk, which is what the
    /// worktree looked like before they were made.
    pub fn discard(&mut self, ids: &[ChangeId]) {
        let staged = self.named_where(ids, Changed::is_staged);
        let created = self.named_where(ids, Changed::is_created);
        let tracked = self.named_where(ids, |changed| !changed.is_created());
        let touched = staged
            .iter()
            .chain(&created)
            .chain(&tracked)
            .map(|(owner, _)| *owner)
            .collect::<BTreeSet<_>>();

        for repository in touched {
            let root = self.repositories[repository].root().to_path_buf();
            let own = |named: &[(usize, PathBuf)]| {
                named
                    .iter()
                    .filter(|(owner, _)| *owner == repository)
                    .map(|(_, path)| path.clone())
                    .collect::<Vec<_>>()
            };
            let said = discard_in(&root, &own(&staged), &own(&tracked), &own(&created));
            self.repositories[repository].heard(said);
        }
        self.reread();
    }

    /// Puts one hunk of a file into the index, or takes one back out of it.
    ///
    /// This is Zed's way of it, and it is the only way that does not go
    /// through a patch: the text the index is to hold is worked out here —
    /// what it holds now, with the run of lines this hunk covers written over
    /// by the other side's — and handed to git as the whole of the file. An
    /// unstaged hunk is the worktree's lines going in; a staged one is the
    /// last commit's lines going back over them.
    pub fn stage_hunk(&mut self, id: ChangeId, staged: bool, hunk: usize) {
        let Some(path) = self.path_of(id).map(Path::to_path_buf) else {
            return;
        };
        let Some(patch) = self.patches.get(&path) else {
            return;
        };
        let side = match staged {
            true => &patch.staged,
            false => &patch.unstaged,
        };
        let Some(owner) = self.owner_of(&path) else {
            return;
        };
        let root = self.repositories[owner].root().to_path_buf();
        let (Some(hunk), Some(held)) = (side.get(hunk), pm_core::baseline(&root, &path)) else {
            return;
        };

        // An unstaged hunk is measured against the index, so its old side is
        // the run to write over. A staged one is measured against the last
        // commit, and what the index holds is its new side.
        let (from, count, replacement) = match staged {
            true => (hunk.start, hunk.new_count, hunk.side(false)),
            false => (hunk.old_start, hunk.old_count, hunk.side(true)),
        };
        let Some(written) = rewritten(&held, from, count, &replacement) else {
            return;
        };
        self.done(owner, pm_core::write_index(&root, &path, &written));
    }

    /// Puts the lines of one hunk back the way the other side has them.
    ///
    /// This is the hunk-sized discard: the run of lines the hunk covers in
    /// the worktree is written back over with the side it differs from, and
    /// the rest of the file is left exactly as it is.
    pub fn restore_hunk(&mut self, id: ChangeId, staged: bool, hunk: usize) {
        let Some(path) = self.path_of(id).map(Path::to_path_buf) else {
            return;
        };
        let Some(patch) = self.patches.get(&path) else {
            return;
        };
        let side = match staged {
            true => &patch.staged,
            false => &patch.unstaged,
        };
        let (Some(hunk), Ok(held)) = (side.get(hunk), std::fs::read_to_string(&path)) else {
            return;
        };
        let Some(written) = rewritten(&held, hunk.start, hunk.new_count, &hunk.side(false)) else {
            return;
        };
        let owner = self.owner_of(&path).unwrap_or(self.active);

        self.done(
            owner,
            std::fs::write(&path, written)
                .map(|()| String::new())
                .map_err(|error| error.to_string()),
        );
    }

    /// Commits what the active repository's index holds, saying what its
    /// message field holds.
    ///
    /// The message is cleared only when the commit was made: a commit a hook
    /// refused is one to try again, and retyping the message is not part of
    /// trying again.
    pub fn commit(&mut self) {
        let active = self.active;
        let tracked = self.staged_of(active) == 0;
        let Some(repository) = self.active_repository_mut() else {
            return;
        };
        let said = pm_core::commit(repository.root(), &repository.said(), tracked);
        if said.is_ok() {
            repository.message_mut().clear();
        }
        self.done(active, said);
    }

    /// Takes in what git said in the `repository`-th, and reads the worktree
    /// again.
    fn done(&mut self, repository: usize, said: pm_core::Said) {
        if let Some(held) = self.repositories.get_mut(repository) {
            held.heard(said);
        }
        self.reread();
    }

    /// The files `wanted` accepts, as paths.
    fn paths(&self, wanted: impl Fn(&Changed) -> bool) -> Vec<PathBuf> {
        self.changed
            .iter()
            .filter(|changed| wanted(changed))
            .map(|changed| changed.path.clone())
            .collect()
    }

    /// The files `wanted` accepts, with the repository each is in.
    fn owned(&self, wanted: impl Fn(&Changed) -> bool) -> Vec<(usize, PathBuf)> {
        self.changed
            .iter()
            .zip(&self.owners)
            .filter(|(changed, _)| wanted(changed))
            .map(|(changed, owner)| (*owner, changed.path.clone()))
            .collect()
    }
}

/// Throws away the changes to one repository's files at `root`: `staged`
/// taken out of the index, `tracked` put back from it and `created` taken
/// off the disk, stopping at the first that git refuses.
fn discard_in(
    root: &Path,
    staged: &[PathBuf],
    tracked: &[PathBuf],
    created: &[PathBuf],
) -> pm_core::Said {
    if !staged.is_empty() {
        pm_core::unstage(root, staged)?;
    }
    if !tracked.is_empty() {
        pm_core::discard_all(root, tracked)?;
    }
    for path in created {
        pm_core::discard(root, path, true)?;
    }
    Ok(String::new())
}

/// `text` with `count` lines from `from` written over by `replacement`.
///
/// Lines are counted from one, the way a diff counts them. A file that does
/// not end in a newline keeps not ending in one, because whether the last
/// line is finished is part of what the file holds.
fn rewritten(text: &str, from: usize, count: usize, replacement: &str) -> Option<String> {
    let lines = text.split_inclusive('\n').collect::<Vec<_>>();
    let first = from.checked_sub(1)?;
    if first > lines.len() || first + count > lines.len() {
        return None;
    }

    let mut written = lines[..first].concat();
    written.push_str(replacement);
    written.push_str(&lines[first + count..].concat());

    let unfinished = !text.ends_with('\n') && !text.is_empty();
    if unfinished && written.ends_with('\n') {
        written.pop();
    }
    Some(written)
}
