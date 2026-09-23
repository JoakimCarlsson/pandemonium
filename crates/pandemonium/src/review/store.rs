//! What one project has changed, and everything done about it.
//!
//! This is the one seam between the window and git's index: the sidebar, the
//! review pane and a keybinding all stage, unstage, throw away and commit
//! through here, and each of them is followed by asking git again rather
//! than by guessing what the answer would now be. Git is the state; this is
//! what the window last read of it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pm_core::{Changed, Head, Hunk, Side, Status};

use crate::input::{Input, Submit};

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

/// What one project has changed, as the window last read it.
pub struct Review {
    /// The worktree this is a review of.
    root: PathBuf,
    /// What git makes of that worktree.
    status: Status,
    /// The lines of each file that has changed.
    patches: BTreeMap<PathBuf, Patch>,
    /// The files whose lines are folded away in the review pane.
    collapsed: BTreeSet<PathBuf>,
    /// What the next commit will say, as a buffer like any other.
    ///
    /// The message is edited in the editor the window is made of rather than
    /// in a line of its own: a commit message is several lines, it is written
    /// with the cursor moved about and the text selected, and every one of
    /// those is something the editor already does.
    message: Input,
    /// What git said when it last would not do something.
    trouble: Option<String>,
    /// Commits on the checked out branch.
    history_auto: Vec<pm_core::Commit>,
    /// Commits reachable from every reference.
    history_all: Vec<pm_core::Commit>,
    /// First visible commit in each history filter.
    history_scrolls: [usize; 2],
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
}

impl Review {
    /// The changes of the worktree at `root`, read now.
    pub fn of(root: &Path) -> Self {
        let mut review = Self {
            root: root.to_path_buf(),
            status: Status::default(),
            patches: BTreeMap::new(),
            collapsed: BTreeSet::new(),
            message: Input::many_lines("COMMIT_EDITMSG").submitting(Submit::Chord),
            trouble: None,
            history_auto: Vec::new(),
            history_all: Vec::new(),
            history_scrolls: [0; 2],
            ids: BTreeMap::new(),
            next: 0,
            kept: BTreeSet::new(),
            selected: None,
            marked: BTreeSet::new(),
            gesture: None,
            scrolls: BTreeMap::new(),
        };
        review.reread();
        review
    }

    /// Asks git again what the worktree holds.
    ///
    /// The whole worktree is asked about at once, because that is what git
    /// answers in one go: a file at a time would be a subprocess per row of
    /// a list as long as the change is.
    pub fn reread(&mut self) {
        self.status = Status::of(&self.root);
        self.history_auto = pm_core::history(&self.root, 500, false);
        self.history_all = pm_core::history(&self.root, 500, true);
        self.patches = pm_core::diffs(&self.root, Side::Staged)
            .into_iter()
            .map(|(path, hunks)| {
                let patch = Patch {
                    staged: hunks,
                    unstaged: Vec::new(),
                };
                (path, patch)
            })
            .collect();
        for (path, hunks) in pm_core::diffs(&self.root, Side::Unstaged) {
            self.patches.entry(path).or_default().unstaged = hunks;
        }
        for path in self.untracked() {
            let hunks = pm_core::diff(&self.root, &path, Side::Untracked);
            self.patches.entry(path).or_default().unstaged = hunks;
        }
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
    }

    /// The cached commits selected by the Source Control graph filter.
    pub fn history(&self, all: bool) -> &[pm_core::Commit] {
        match all {
            true => &self.history_all,
            false => &self.history_auto,
        }
    }

    /// The first visible commit under the selected history filter.
    pub fn history_scroll(&self, all: bool, visible: usize) -> usize {
        let total = self.history(all).len();
        self.history_scrolls[usize::from(all)].min(total.saturating_sub(visible.max(1)))
    }

    /// Scrolls the selected history filter within the commits it has read.
    pub fn scroll_history(&mut self, all: bool, rows: isize, visible: usize) {
        let index = usize::from(all);
        let total = self.history(all).len();
        let last = total.saturating_sub(visible.max(1));
        self.history_scrolls[index] = self.history_scrolls[index]
            .saturating_add_signed(rows)
            .min(last);
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

    /// How many files git already knows about have changed.
    ///
    /// These are what a commit takes when nothing has been staged: a commit
    /// can take a tracked file's changes without being told to, and can never
    /// take a file git has not been told about.
    pub fn tracked(&self) -> usize {
        self.changed()
            .iter()
            .filter(|changed| !changed.is_untracked() && !changed.is_conflicted())
            .count()
    }

    /// What the control offering to commit says, and whether it can.
    ///
    /// The words are Zed's: with something staged it commits that, and with
    /// nothing staged it offers to take every tracked change instead. What
    /// stops it saying so — a conflict, no message, nothing to commit — is
    /// what the control says in its place.
    pub fn committable(&self) -> (String, Option<&'static str>) {
        let staged = self.staged();
        let title = match staged {
            0 => "Commit Tracked".to_owned(),
            _ => "Commit".to_owned(),
        };

        let stopped = if self.changed().iter().any(Changed::is_conflicted) {
            Some("Resolve the conflicts before committing")
        } else if staged == 0 && self.tracked() == 0 {
            Some("Nothing to commit")
        } else if self.unsaid() {
            Some("No commit message")
        } else {
            None
        };
        (title, stopped)
    }

    /// How many files have something staged for the next commit.
    pub fn staged(&self) -> usize {
        self.changed()
            .iter()
            .filter(|changed| changed.is_staged())
            .count()
    }

    /// How many files of `group` are staged, and how many there are.
    ///
    /// This is what the box on the group's heading says: none of them, all of
    /// them, or somewhere in between.
    pub fn staged_in(&self, group: Group) -> (usize, usize) {
        let rows = self.grouped(group);
        let staged = rows
            .iter()
            .filter_map(|index| self.change(*index))
            .filter(|changed| changed.is_staged() && !changed.is_unstaged())
            .count();
        (staged, rows.len())
    }

    /// The files of `group`, in the order the sidebar lists them.
    pub fn grouped(&self, group: Group) -> Vec<usize> {
        self.changed()
            .iter()
            .enumerate()
            .filter(|(_, changed)| group.holds(changed))
            .map(|(index, _)| index)
            .collect()
    }

    /// Every row the sidebar draws, in the order it draws them.
    fn listed(&self) -> Vec<ChangeId> {
        Group::ALL
            .into_iter()
            .flat_map(|group| self.grouped(group))
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

    /// What git makes of that worktree.
    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Where the worktree's head stands.
    pub fn head(&self) -> &Head {
        self.status.head()
    }

    /// Every file that has changed, in the order the lists show them.
    pub fn changed(&self) -> &[Changed] {
        self.status.changed()
    }

    /// The `index`-th file that has changed.
    pub fn change(&self, index: usize) -> Option<&Changed> {
        self.changed().get(index)
    }

    /// The lines `path` has changed, on each side of the index.
    pub fn patch(&self, path: &Path) -> Option<&Patch> {
        self.patches.get(path)
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

    /// The box the next commit's message is written in.
    pub fn message(&self) -> &Input {
        &self.message
    }

    /// That box, to write in.
    pub fn message_mut(&mut self) -> &mut Input {
        &mut self.message
    }

    /// What it holds.
    pub fn said(&self) -> String {
        self.message.value()
    }

    /// Whether nothing has been written in it.
    pub fn unsaid(&self) -> bool {
        self.said().trim().is_empty()
    }

    /// What git said when it last would not do something.
    pub fn trouble(&self) -> Option<&str> {
        self.trouble.as_deref()
    }

    /// Records what a Git operation outside the review said and rereads it.
    pub fn report(&mut self, said: pm_core::Said) {
        self.done(said);
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

    /// The files `ids` names that `wanted` accepts.
    ///
    /// An id naming a file that has stopped differing is left out rather
    /// than passed on: the list is read again after every change, and a
    /// command carried out over a stale row is a command over one file less.
    fn named_where(&self, ids: &[ChangeId], wanted: impl Fn(&Changed) -> bool) -> Vec<PathBuf> {
        ids.iter()
            .filter_map(|id| self.change(self.place_of(*id)?))
            .filter(|changed| wanted(changed))
            .map(|changed| changed.path.clone())
            .collect()
    }

    /// Puts the files `ids` names into the index.
    ///
    /// Every file goes in one call, because staging four files is one thing
    /// the reader asked for and should be one thing git is told — and a
    /// half-finished bulk change is the state nobody can reason about.
    pub fn stage(&mut self, ids: &[ChangeId]) {
        let paths = self.named_where(ids, Changed::is_unstaged);
        if paths.is_empty() {
            return;
        }
        self.done(pm_core::stage(&self.root, &paths));
    }

    /// Takes the files `ids` names back out of the index.
    pub fn unstage(&mut self, ids: &[ChangeId]) {
        let paths = self.named_where(ids, Changed::is_staged);
        if paths.is_empty() {
            return;
        }
        self.done(pm_core::unstage(&self.root, &paths));
    }

    /// Puts everything that has changed into the index.
    pub fn stage_all(&mut self) {
        let paths = self.paths(Changed::is_unstaged);
        self.done(pm_core::stage(&self.root, &paths));
    }

    /// Takes everything back out of the index.
    pub fn unstage_all(&mut self) {
        let paths = self.paths(Changed::is_staged);
        self.done(pm_core::unstage(&self.root, &paths));
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

        if !staged.is_empty()
            && let Err(said) = pm_core::unstage(&self.root, &staged)
        {
            return self.done(Err(said));
        }
        if !tracked.is_empty()
            && let Err(said) = pm_core::discard_all(&self.root, &tracked)
        {
            return self.done(Err(said));
        }
        for path in created {
            if let Err(said) = pm_core::discard(&self.root, &path, true) {
                return self.done(Err(said));
            }
        }
        self.done(Ok(String::new()));
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
        let (Some(hunk), Some(held)) = (side.get(hunk), pm_core::baseline(&self.root, &path))
        else {
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
        self.done(pm_core::write_index(&self.root, &path, &written));
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

        self.done(
            std::fs::write(&path, written)
                .map(|()| String::new())
                .map_err(|error| error.to_string()),
        );
    }

    /// Commits what the index holds, saying what the message field holds.
    ///
    /// The message is cleared only when the commit was made: a commit a hook
    /// refused is one to try again, and retyping the message is not part of
    /// trying again.
    pub fn commit(&mut self) {
        let message = self.said();
        let said = pm_core::commit(&self.root, &message, self.staged() == 0);
        if said.is_ok() {
            self.message.clear();
        }
        self.done(said);
    }

    /// Takes in what git said, and reads the worktree again.
    fn done(&mut self, said: pm_core::Said) {
        self.trouble = said.err().filter(|said| !said.is_empty());
        self.reread();
    }

    /// The files `wanted` accepts, as paths.
    fn paths(&self, wanted: impl Fn(&Changed) -> bool) -> Vec<PathBuf> {
        self.changed()
            .iter()
            .filter(|changed| wanted(changed))
            .map(|changed| changed.path.clone())
            .collect()
    }

    /// The files git has never been told about.
    fn untracked(&self) -> Vec<PathBuf> {
        self.paths(Changed::is_untracked)
    }
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
