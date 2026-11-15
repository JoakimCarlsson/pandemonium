//! The git state of the open session's worktree as the right panel shows it:
//! the branch and where it stands, the changed files in groups, what is
//! picked among them, the commit message, the graph, the open menu or
//! question, and the file whose diff the Diff tab shows; with the actions
//! the panel asks for.

use crate::review::{ConflictSide, ConflictView, LineNote, Revision};

/// The git state of a worktree, as the right panel shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GitSummary {
    /// The folder the panel works on, as the session names it, or empty when no session is open.
    pub folder: String,
    /// The branch the session's worktree was made from, when it has one.
    pub base: Option<String>,
    /// The checked out branch, or empty on a detached head.
    pub branch: String,
    /// The commit checked out, abbreviated, when there is one.
    pub head: Option<String>,
    /// The remote branch the checked out one follows, when it follows one.
    pub upstream: Option<String>,
    /// How many commits the branch has that the one it follows has not.
    pub ahead: usize,
    /// How many commits the branch it follows has that it has not.
    pub behind: usize,
    /// The remotes the repository has.
    pub remotes: Vec<String>,
    /// The merge, rebase or cherry-pick the folder is stopped in, by name, when it is in one.
    pub operation: Option<String>,
    /// Whether the folder is a repository; false until the server said.
    pub repository: bool,
    /// Whether the server answered at least once for this folder.
    pub loaded: bool,
    /// The files changed and not committed.
    pub changes: Vec<ChangedFile>,
    /// The commit message typed and not yet committed.
    pub message: String,
    /// The paths of the files picked in the list.
    pub selected: Vec<String>,
    /// The path a range pick runs from.
    pub anchor: Option<String>,
    /// Whether the Changes section is unfolded.
    pub changes_open: bool,
    /// Whether the Graph section is unfolded.
    pub graph_open: bool,
    /// Whether the graph shows what every reference reaches, not just the checked out branch.
    pub graph_all: bool,
    /// The latest commits, newest first, with the lanes they are drawn in.
    pub commits: Vec<GraphCommit>,
    /// Why the server could not list the commits, when it could not.
    pub graph_trouble: Option<String>,
    /// The local branches, the current one first.
    pub branches: Vec<String>,
    /// The branches of the remotes, each with its remote's name in front.
    pub remote_branches: Vec<String>,
    /// The branch menu, while it is open.
    pub branch_menu: Option<BranchMenu>,
    /// The menu open in the panel, when one is.
    pub menu: Option<GitMenu>,
    /// The question the panel asks before acting, when it asks one.
    pub prompt: Option<GitPrompt>,
    /// What the panel is waiting for the server to finish, such as `Pushing`.
    pub busy: Option<String>,
    /// The last thing git refused to do, and why.
    pub notice: Option<String>,
    /// The file whose diff the Diff tab shows, when it is open.
    pub diff: Option<String>,
    /// The notes left on lines of the diffs, waiting to be sent to the agent.
    pub notes: Vec<LineNote>,
    /// The files of a commit or a branch the Diff tab shows in place of the uncommitted ones, when it shows them.
    pub revision: Option<Revision>,
    /// The conflicted file being resolved, once the Diff tab opened it.
    pub conflict: Option<ConflictView>,
    /// The files whose diffs were asked for and not yet answered.
    pub wanted: Vec<String>,
}

/// A file changed in a worktree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChangedFile {
    /// Its path from the worktree's root.
    pub path: String,
    /// Its status letter, such as `M`.
    pub status: char,
    /// Lines added.
    pub added: usize,
    /// Lines removed.
    pub removed: usize,
    /// Whether the change is staged for the next commit.
    pub staged: bool,
    /// Whether git does not track the file yet.
    pub untracked: bool,
    /// Whether the file is changed on both sides of a merge.
    pub conflicted: bool,
    /// Where the file was before it was renamed, when it was.
    pub from: Option<String>,
    /// The lines of its diff from the first hunk on, or empty until the server sent them.
    pub diff: String,
}

/// The groups the changed files are listed in, in the order they show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChangeGroup {
    /// Files changed on both sides of a merge.
    Conflicted,
    /// Files staged for the next commit.
    Staged,
    /// Tracked files changed and not staged.
    Tracked,
    /// Files git does not track yet.
    Untracked,
}

impl ChangeGroup {
    /// Every group, in the order the panel lists them.
    pub const ALL: [ChangeGroup; 4] = [
        ChangeGroup::Conflicted,
        ChangeGroup::Staged,
        ChangeGroup::Tracked,
        ChangeGroup::Untracked,
    ];

    /// The heading the group shows under.
    pub fn title(self) -> &'static str {
        match self {
            ChangeGroup::Conflicted => "Merge conflicts",
            ChangeGroup::Staged => "Staged changes",
            ChangeGroup::Tracked => "Changes",
            ChangeGroup::Untracked => "Untracked",
        }
    }
}

impl ChangedFile {
    /// The group the file is listed in.
    pub fn group(&self) -> ChangeGroup {
        if self.conflicted {
            ChangeGroup::Conflicted
        } else if self.staged {
            ChangeGroup::Staged
        } else if self.untracked {
            ChangeGroup::Untracked
        } else {
            ChangeGroup::Tracked
        }
    }

    /// The file's name, after its last slash.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The folder the file is in, before its last slash, or empty at the root.
    pub fn folder(&self) -> &str {
        self.path
            .rsplit_once('/')
            .map(|(folder, _)| folder)
            .unwrap_or("")
    }
}

/// One commit of the graph and how its row is drawn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphCommit {
    /// Its abbreviated name.
    pub id: String,
    /// The first line of its message.
    pub summary: String,
    /// The branches and tags pointing at it.
    pub refs: Vec<String>,
    /// The lane its dot sits in.
    pub lane: usize,
    /// The colour index of its own branch.
    pub color: usize,
    /// Every line crossing its row.
    pub edges: Vec<GraphEdge>,
    /// How many lanes its row is wide.
    pub width: usize,
}

/// One line through half of a commit's row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraphEdge {
    /// Whether it crosses the upper half, from the top edge down to the dot, rather than the lower.
    pub upper: bool,
    /// The lane it starts in, at the top of its half.
    pub from: usize,
    /// The lane it ends in, at the bottom of its half.
    pub to: usize,
    /// The colour index of the branch it belongs to.
    pub color: usize,
}

/// The open list of branches under the branch button.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchMenu {
    /// What the list is narrowed to, which is also the name of a new branch.
    pub filter: String,
}

/// A menu of the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitMenu {
    /// The menu on a changed file, or on the picked files when it is one of them.
    Change(String),
    /// The menu under the commit button: commit and push, amend.
    Commit,
    /// The panel's own menu: pull, push, fetch, stage all and the branch's tasks.
    Source,
    /// The menu on a commit of the graph.
    History(String),
}

/// The tasks on a branch that ask before they act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchTask {
    /// Publish the checked out branch to a remote.
    Publish,
    /// Rename a branch.
    Rename,
    /// Delete a branch.
    Delete,
    /// Merge a branch into the checked out one.
    Merge,
    /// Rebase the checked out branch onto a branch.
    Rebase,
}

impl BranchTask {
    /// The name the server knows the task by.
    pub fn action(self) -> &'static str {
        match self {
            BranchTask::Publish => "publish",
            BranchTask::Rename => "rename",
            BranchTask::Delete => "delete",
            BranchTask::Merge => "merge",
            BranchTask::Rebase => "rebase",
        }
    }
}

/// A question the panel asks before it acts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitPrompt {
    /// Whether to throw away the changes of these files.
    Discard(Vec<String>),
    /// Whether to rewrite the last commit with the message and what is staged.
    Amend,
    /// Whether to overwrite the remote branch with the local one.
    ForcePush,
    /// What to call a new branch made at the checked out commit.
    NewBranch(String),
    /// A task on `branch`, with the name typed for it when it takes one.
    Branch {
        /// The task.
        task: BranchTask,
        /// The branch it acts on.
        branch: String,
        /// The name typed, for renaming and publishing.
        name: String,
    },
}

/// How a click picks a file of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Picks only this file.
    Only,
    /// Adds the file to the picked ones, or takes it out.
    Toggle,
    /// Picks every file from the last one picked to this one.
    Range,
}

/// A section of the panel that folds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitSection {
    /// The changed files.
    Changes,
    /// The commit graph.
    Graph,
}

/// What the person asks the Git panel to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitAction {
    /// Asks the server again for everything the panel shows.
    Refresh,
    /// Sets the commit message to the text.
    Message(String),
    /// Commits what is staged, or every tracked change when nothing is, then pushes when `push`.
    Commit {
        /// Whether to push after committing.
        push: bool,
    },
    /// Presses the main button: commits, syncs or publishes, as [`GitSummary::primary`] says.
    Primary,
    /// Stages the files when true, unstages them when false.
    Stage(Vec<String>, bool),
    /// Stages every change when true, unstages every one when false.
    StageAll(bool),
    /// Picks the file at the path the way given.
    Pick(String, Pick),
    /// Asks whether to throw away the changes of the files.
    Discard(Vec<String>),
    /// Adds the files to the `.gitignore`.
    Ignore(Vec<String>),
    /// Opens the file's diff in the Diff tab.
    Open(String),
    /// Closes the Diff tab.
    CloseDiff,
    /// Opens a menu, or closes the open one when nothing is given.
    Menu(Option<GitMenu>),
    /// Opens a question.
    Ask(GitPrompt),
    /// Sets the name the open question has typed.
    PromptText(String),
    /// Does what the open question asks about.
    Confirm,
    /// Closes the open question without acting.
    Cancel,
    /// Opens the branch menu when true, closes it when false.
    BranchMenu(bool),
    /// Narrows the branch menu to the text.
    BranchFilter(String),
    /// Checks out the branch, local or a remote's.
    Checkout(String),
    /// Makes a branch with the name at the checked out commit and checks it out.
    CreateBranch(String),
    /// Fetches, pulls or pushes: `fetch`, `pull`, `pull_rebase`, `push`, `force_push` or `sync`.
    Sync(String),
    /// Unsets what the checked out branch follows.
    UnsetUpstream,
    /// Folds or unfolds a section.
    Fold(GitSection),
    /// Shows every reference's commits in the graph when true, only the checked out branch's when false.
    GraphAll(bool),
    /// Forgets the last thing git refused.
    Dismiss,
    /// Leaves a note on a line of a file's diff.
    Note(LineNote),
    /// Takes away the note at the place among the notes.
    RemoveNote(usize),
    /// Sends the notes to the session's agent and forgets them.
    SendNotes,
    /// Shows in the Diff tab the files changed from `from`, or the commit's parent, to `to`.
    ShowRevision {
        /// What the changes are named as.
        title: String,
        /// The branch or commit counted from, the commit's parent when nothing.
        from: Option<String>,
        /// The commit or branch counted to.
        to: String,
    },
    /// Goes back from a commit's or a branch's files to the uncommitted ones.
    CloseRevision,
    /// Asks for the diffs of the files listed without them.
    LoadDiffs(Vec<String>),
    /// Settles the conflict at the place among the open file's conflicts with a side.
    TakeConflict(usize, ConflictSide),
    /// Settles every conflict of the open file with a side.
    TakeAllConflicts(ConflictSide),
    /// Writes the open conflicted file as resolved and stages it.
    Resolve,
}

/// What the main button under the commit message does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Primary {
    /// Commits; `ready` once there is something to commit and a message.
    Commit {
        /// Whether pressing it commits.
        ready: bool,
    },
    /// Pulls and pushes the commits the branch and the one it follows differ by.
    Sync {
        /// Commits to push.
        ahead: usize,
        /// Commits to pull.
        behind: usize,
    },
    /// Publishes a branch that follows nothing.
    Publish,
    /// Waits for the server to finish, saying what.
    Busy(String),
}

impl GitSummary {
    /// What the main button does now.
    pub fn primary(&self) -> Primary {
        if let Some(busy) = &self.busy {
            return Primary::Busy(busy.clone());
        }
        if !self.changes.is_empty() {
            return Primary::Commit {
                ready: !self.message.trim().is_empty(),
            };
        }
        if self.upstream.is_some() && (self.ahead > 0 || self.behind > 0) {
            return Primary::Sync {
                ahead: self.ahead,
                behind: self.behind,
            };
        }
        if self.upstream.is_none() && !self.branch.is_empty() && !self.remotes.is_empty() {
            return Primary::Publish;
        }
        Primary::Commit { ready: false }
    }

    /// The changed files in `group`, in the order the server listed them.
    pub fn group(&self, group: ChangeGroup) -> Vec<&ChangedFile> {
        self.changes
            .iter()
            .filter(|change| change.group() == group)
            .collect()
    }

    /// The changes in the order the list shows them.
    pub fn listed(&self) -> Vec<&ChangedFile> {
        ChangeGroup::ALL
            .iter()
            .flat_map(|group| self.group(*group))
            .collect()
    }

    /// The paths an action on the file at `path` acts on: the picked files when it is one of them, else that file alone.
    pub fn acting(&self, path: &str) -> Vec<String> {
        if self.selected.iter().any(|picked| picked == path) {
            self.selected.clone()
        } else {
            vec![path.to_owned()]
        }
    }

    /// The branches the branch menu lists for its filter: local ones, then remote ones not checked out locally.
    pub fn branch_choices(&self) -> Vec<String> {
        let filter = self
            .branch_menu
            .as_ref()
            .map(|menu| menu.filter.trim().to_lowercase())
            .unwrap_or_default();
        self.branches
            .iter()
            .chain(self.remote_branches.iter().filter(|remote| {
                !remote.ends_with("/HEAD")
                    && !self.branches.iter().any(|local| {
                        remote
                            .split_once('/')
                            .is_some_and(|(_, name)| name == local)
                    })
            }))
            .filter(|name| filter.is_empty() || name.to_lowercase().contains(&filter))
            .cloned()
            .collect()
    }

    /// Picks the file at `path` the way `pick` says.
    pub fn pick(&mut self, path: &str, pick: Pick) {
        match pick {
            Pick::Only => {
                self.selected = vec![path.to_owned()];
                self.anchor = Some(path.to_owned());
            }
            Pick::Toggle => {
                if let Some(place) = self.selected.iter().position(|picked| picked == path) {
                    self.selected.remove(place);
                } else {
                    self.selected.push(path.to_owned());
                }
                self.anchor = Some(path.to_owned());
            }
            Pick::Range => {
                let listed: Vec<String> = self
                    .listed()
                    .into_iter()
                    .map(|change| change.path.clone())
                    .collect();
                let to = listed.iter().position(|listed| listed == path);
                let from = self
                    .anchor
                    .as_ref()
                    .and_then(|anchor| listed.iter().position(|listed| listed == anchor))
                    .or(to);
                if let (Some(from), Some(to)) = (from, to) {
                    let (low, high) = (from.min(to), from.max(to));
                    self.selected = listed[low..=high].to_vec();
                }
            }
        }
    }

    /// Drops picks of files no longer changed.
    pub fn forget_gone(&mut self) {
        let changes = &self.changes;
        self.selected
            .retain(|picked| changes.iter().any(|change| &change.path == picked));
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !changes.iter().any(|change| &change.path == anchor))
        {
            self.anchor = None;
        }
    }
}

/// One row of a side-by-side diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SplitRow {
    /// The old side: its line number and text, when this row has one.
    pub left: Option<(usize, String)>,
    /// The new side: its line number and text, when this row has one.
    pub right: Option<(usize, String)>,
    /// The hunk header this row stands for, when it is one.
    pub hunk: Option<String>,
    /// The stretches of the old side's text that changed, as byte ranges.
    pub left_words: Vec<(usize, usize)>,
    /// The stretches of the new side's text that changed, as byte ranges.
    pub right_words: Vec<(usize, usize)>,
}

/// One side of a changed line waiting to be paired: its number, its text and the stretches of it that changed.
type Waiting = (usize, String, Vec<(usize, usize)>);

/// Lays out `diff`, its lines from the first hunk on, side by side: unchanged
/// lines on both sides, removed lines on the left beside the added lines that
/// replace them on the right, with the words that changed inside them.
pub fn side_by_side(diff: &str) -> Vec<SplitRow> {
    let changed: std::collections::HashMap<usize, Vec<(usize, usize)>> = pm_git::words(diff)
        .into_iter()
        .map(|words| (words.line, words.ranges))
        .collect();
    let words_of = |at: usize| changed.get(&at).cloned().unwrap_or_default();
    let mut rows = Vec::new();
    let mut old = 0;
    let mut new = 0;
    let mut removed: Vec<Waiting> = Vec::new();
    let mut added: Vec<Waiting> = Vec::new();
    for (at, line) in diff.lines().enumerate() {
        if let Some(header) = line.strip_prefix("@@") {
            pair_up(&mut rows, &mut removed, &mut added);
            let (from, to) = hunk_starts(header);
            old = from;
            new = to;
            rows.push(SplitRow {
                hunk: Some(line.to_owned()),
                ..SplitRow::default()
            });
        } else if let Some(text) = line.strip_prefix('-') {
            removed.push((old, text.to_owned(), words_of(at)));
            old += 1;
        } else if let Some(text) = line.strip_prefix('+') {
            added.push((new, text.to_owned(), words_of(at)));
            new += 1;
        } else if !line.starts_with('\\') {
            pair_up(&mut rows, &mut removed, &mut added);
            let text = line.strip_prefix(' ').unwrap_or(line).to_owned();
            rows.push(SplitRow {
                left: Some((old, text.clone())),
                right: Some((new, text)),
                ..SplitRow::default()
            });
            old += 1;
            new += 1;
        }
    }
    pair_up(&mut rows, &mut removed, &mut added);
    rows
}

/// Moves the `removed` and `added` lines waiting into `rows`, side by side.
fn pair_up(rows: &mut Vec<SplitRow>, removed: &mut Vec<Waiting>, added: &mut Vec<Waiting>) {
    let count = removed.len().max(added.len());
    let mut left = removed.drain(..);
    let mut right = added.drain(..);
    for _ in 0..count {
        let (left, left_words) = match left.next() {
            Some((number, text, words)) => (Some((number, text)), words),
            None => (None, Vec::new()),
        };
        let (right, right_words) = match right.next() {
            Some((number, text, words)) => (Some((number, text)), words),
            None => (None, Vec::new()),
        };
        rows.push(SplitRow {
            left,
            right,
            hunk: None,
            left_words,
            right_words,
        });
    }
}

/// The first old and new line numbers of a hunk whose header, after its leading `@@`, is `header`.
fn hunk_starts(header: &str) -> (usize, usize) {
    let start = |sign: char| {
        header
            .split_whitespace()
            .find_map(|part| part.strip_prefix(sign))
            .and_then(|range| range.split(',').next())
            .and_then(|number| number.parse().ok())
            .unwrap_or(1)
    };
    (start('-'), start('+'))
}

/// How many lines `diff` adds and removes.
pub fn tally(diff: &str) -> (usize, usize) {
    diff.lines().fold((0, 0), |(added, removed), line| {
        if line.starts_with('+') {
            (added + 1, removed)
        } else if line.starts_with('-') {
            (added, removed + 1)
        } else {
            (added, removed)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Removed and added lines pair up beside each other, numbered from the hunk header.
    #[test]
    fn lays_out_hunks_side_by_side() {
        let rows = side_by_side("@@ -3,3 +3,4 @@\n a\n-b\n+B\n+C\n d");
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[1].left, Some((3, "a".to_owned())));
        assert_eq!(rows[2].left, Some((4, "b".to_owned())));
        assert_eq!(rows[2].right, Some((4, "B".to_owned())));
        assert_eq!(rows[3].left, None);
        assert_eq!(rows[3].right, Some((5, "C".to_owned())));
        assert_eq!(rows[4].right, Some((6, "d".to_owned())));
    }

    /// A range pick takes every listed file between the anchor and the file clicked.
    #[test]
    fn range_picks_follow_the_list() {
        let file = |path: &str, staged| ChangedFile {
            path: path.to_owned(),
            staged,
            ..ChangedFile::default()
        };
        let mut git = GitSummary {
            changes: vec![file("a", false), file("b", true), file("c", false)],
            ..GitSummary::default()
        };
        git.pick("b", Pick::Only);
        git.pick("c", Pick::Range);
        assert_eq!(
            git.selected,
            vec!["b".to_owned(), "a".to_owned(), "c".to_owned()]
        );
        assert_eq!(git.acting("a").len(), 3);
        assert_eq!(git.acting("x"), vec!["x".to_owned()]);
    }

    /// The main button pushes once everything is committed and the branch is ahead.
    #[test]
    fn primary_follows_the_state() {
        let mut git = GitSummary {
            branch: "main".to_owned(),
            upstream: Some("origin/main".to_owned()),
            ahead: 2,
            ..GitSummary::default()
        };
        assert_eq!(
            git.primary(),
            Primary::Sync {
                ahead: 2,
                behind: 0
            }
        );
        git.changes.push(ChangedFile::default());
        assert_eq!(git.primary(), Primary::Commit { ready: false });
        git.busy = Some("Pushing".to_owned());
        assert_eq!(git.primary(), Primary::Busy("Pushing".to_owned()));
    }
}
