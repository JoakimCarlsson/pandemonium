//! The Git panel's side of the link: asks the server where the open
//! session's folder stands, its changes, branches and graph; watches the
//! folder so the panel follows changes made anywhere; and carries out what
//! the panel asks, stage, commit, discard, ignore, branch and remote work,
//! as server requests, which work the same for remote projects.

use pm_protocol::{Request, Response};

use super::{Ask, Link, same_folder};
use crate::git::{
    BranchMenu, BranchTask, ChangedFile, GitAction, GitPrompt, GitSection, GitSummary, GraphCommit,
    GraphEdge, Primary, tally,
};
use crate::open_files::SessionTab;

/// How many commits the graph lists.
const GRAPH_COMMITS: usize = 60;

/// What a request the Git panel made was for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GitAsk {
    /// Where the folder stands and what it holds that differs from the last commit.
    Inspect(String),
    /// The graph of the folder's commits.
    Log(String),
    /// The folder's local branches.
    Branches(String),
    /// The folder's remotes and their branches.
    BranchInfo(String),
    /// The diffs of files listed without them: the folder and the paths asked for.
    Diffs(String, Vec<String>),
    /// The folder's stashes.
    Stashes(String),
    /// The checked out submodules of the session's folder.
    Submodules(String),
    /// The files a commit or a branch changed, for the folder.
    Revision(String),
    /// The versions of a conflicted file of the folder, at the path.
    Conflict(String, String),
    /// Something that changes the folder, then `then` once it is done.
    Act {
        /// The folder acted on.
        folder: String,
        /// What to send once it is done, such as a push after a commit.
        then: Option<Box<Request>>,
    },
}

impl Link {
    /// Points the Git panel at `folder`, the open session's, made from the
    /// branch `base`, starting with `branch` shown until the server answers.
    pub(super) fn git_open(&mut self, folder: String, base: Option<String>, branch: String) {
        if self.workspace.git.root == folder && !folder.is_empty() {
            self.git_refresh();
            return;
        }
        self.workspace.git = GitSummary {
            folder: folder.clone(),
            root: folder.clone(),
            base,
            branch,
            changes_open: true,
            graph_open: true,
            ..GitSummary::default()
        };
        let directories = if folder.is_empty() {
            Vec::new()
        } else {
            vec![folder]
        };
        self.ask(Request::WatchFolders { directories }, Ask::Nothing);
        self.git_refresh();
    }

    /// Asks the server again for everything the Git panel shows.
    pub(super) fn git_refresh(&mut self) {
        let folder = self.workspace.git.folder.clone();
        if folder.is_empty() {
            return;
        }
        let git = &self.workspace.git;
        let inspect = Request::Inspect {
            directory: folder.clone(),
            base: git.base.clone().filter(|_| git.submodule.is_none()),
        };
        let root = git.root.clone();
        let log = Request::Log {
            directory: folder.clone(),
            limit: GRAPH_COMMITS,
            all: git.graph_all,
        };
        let branch = (!git.branch.is_empty()).then(|| git.branch.clone());
        self.ask(inspect, Ask::Git(GitAsk::Inspect(folder.clone())));
        self.ask(log, Ask::Git(GitAsk::Log(folder.clone())));
        self.ask(
            Request::Branches {
                directory: folder.clone(),
            },
            Ask::Git(GitAsk::Branches(folder.clone())),
        );
        self.ask(
            Request::BranchInfo {
                directory: folder.clone(),
                branch,
            },
            Ask::Git(GitAsk::BranchInfo(folder.clone())),
        );
        self.ask(
            Request::Stashes {
                directory: folder.clone(),
            },
            Ask::Git(GitAsk::Stashes(folder)),
        );
        self.ask(
            Request::SubmoduleHeads {
                directory: root.clone(),
            },
            Ask::Git(GitAsk::Submodules(root)),
        );
    }

    /// Refreshes the Git panel when the server says the files of `directory`, or what git keeps about them, changed.
    pub(super) fn git_folder_changed(&mut self, directory: &str) {
        let git = &self.workspace.git;
        if !git.folder.is_empty()
            && (same_folder(&git.folder, directory) || same_folder(&git.root, directory))
        {
            self.git_refresh();
        }
    }

    /// Takes in the `response` to a request the Git panel made for `ask`.
    pub(super) fn git_answer(&mut self, ask: GitAsk, response: Response) {
        let git = &mut self.workspace.git;
        match (ask, response) {
            (GitAsk::Act { folder, then }, response) => {
                if folder != git.folder {
                    return;
                }
                match response {
                    Response::Failed { message } => {
                        git.busy = None;
                        git.notice = Some(message);
                    }
                    _ => match then {
                        Some(next) => {
                            git.busy = Some(busy_word(&next).to_owned());
                            self.ask(*next, Ask::Git(GitAsk::Act { folder, then: None }));
                        }
                        None => git.busy = None,
                    },
                }
                self.git_refresh();
            }
            (GitAsk::Inspect(folder), _)
            | (GitAsk::Log(folder), _)
            | (GitAsk::Branches(folder), _)
            | (GitAsk::BranchInfo(folder), _)
            | (GitAsk::Diffs(folder, _), _)
            | (GitAsk::Stashes(folder), _)
                if folder != git.folder => {}
            (GitAsk::Submodules(root), Response::SubmoduleHeads { heads }) => {
                if root == git.root {
                    git.submodules = heads;
                }
            }
            (
                GitAsk::Inspect(_),
                Response::Inspected {
                    changes,
                    branch,
                    head,
                    tracking,
                    operation,
                    ..
                },
            ) => {
                let kept: Vec<ChangedFile> = std::mem::take(&mut git.changes);
                git.changes = changes
                    .into_iter()
                    .map(|change| changed_file(change, &kept))
                    .collect();
                git.branch = branch.unwrap_or_default();
                git.head = head;
                git.upstream = tracking.upstream;
                git.ahead = tracking.ahead;
                git.behind = tracking.behind;
                git.skippable = matches!(operation, Some(pm_git::Operation::Rebase { .. }));
                git.operation = operation.map(|operation| operation.label());
                git.repository = true;
                git.loaded = true;
                git.forget_gone();
                self.git_load_diff();
            }
            (GitAsk::Inspect(_), Response::Failed { .. }) => {
                git.repository = false;
                git.loaded = true;
                git.changes.clear();
            }
            (GitAsk::Log(_), Response::Log { commits }) => {
                git.commits = commits.into_iter().map(graph_commit).collect();
                git.graph_trouble = None;
            }
            (GitAsk::Log(_), Response::Failed { message }) => {
                git.commits.clear();
                git.graph_trouble = Some(message);
            }
            (GitAsk::Branches(_), Response::Branches { names }) => git.branches = names,
            (
                GitAsk::BranchInfo(_),
                Response::BranchInfo {
                    remotes,
                    remote_branches,
                    ..
                },
            ) => {
                git.remotes = remotes;
                git.remote_branches = remote_branches;
            }
            (GitAsk::Diffs(_, asked), response) => {
                let files = match response {
                    Response::Diffs { files } => files,
                    _ => Vec::new(),
                };
                git.wanted.retain(|path| !asked.contains(path));
                for change in &mut git.changes {
                    if !asked.contains(&change.path) {
                        continue;
                    }
                    change.fetched = true;
                    if let Some(file) = files.iter().find(|file| file.path == change.path) {
                        change.diff = file.diff.clone();
                    }
                }
            }
            (GitAsk::Stashes(_), Response::Stashes { stashes }) => {
                git.stashes = stashes
                    .into_iter()
                    .map(|stash| (stash.index, stash.message))
                    .collect();
            }
            (ask @ (GitAsk::Revision(_) | GitAsk::Conflict(..)), response) => {
                self.review_answer(ask, response)
            }
            _ => {}
        }
    }

    /// Asks for the diff of the file the Diff tab shows when the server listed it without one.
    pub(super) fn git_load_diff(&mut self) {
        let git = &self.workspace.git;
        let Some(path) = &git.diff else {
            return;
        };
        let missing = git
            .changes
            .iter()
            .any(|change| &change.path == path && !change.fetched && !change.untracked);
        if missing && !git.wanted.contains(path) {
            let folder = git.folder.clone();
            let paths = vec![path.clone()];
            self.workspace.git.wanted.extend(paths.iter().cloned());
            self.ask(
                Request::Diffs {
                    directory: folder.clone(),
                    paths: paths.clone(),
                },
                Ask::Git(GitAsk::Diffs(folder, paths)),
            );
        }
    }

    /// Sends `request`, which changes the panel's folder, saying `busy` meanwhile, and `then` once it is done.
    fn git_act(&mut self, request: Request, then: Option<Request>) {
        let folder = self.workspace.git.folder.clone();
        if folder.is_empty() {
            return;
        }
        let git = &mut self.workspace.git;
        git.busy = Some(busy_word(&request).to_owned());
        git.notice = None;
        git.menu = None;
        self.ask(
            request,
            Ask::Git(GitAsk::Act {
                folder,
                then: then.map(Box::new),
            }),
        );
    }

    /// Does what the Git panel asks.
    pub fn git(&mut self, action: GitAction) {
        let directory = self.workspace.git.folder.clone();
        let git = &mut self.workspace.git;
        match action {
            GitAction::Refresh => self.git_refresh(),
            GitAction::Message(text) => git.message = text,
            GitAction::Commit { push } => self.git_commit(false, push),
            GitAction::Primary => self.git_primary(),
            GitAction::Stage(paths, stage) => self.git_act(
                Request::Stage {
                    directory,
                    paths,
                    stage,
                },
                None,
            ),
            GitAction::StageAll(stage) => {
                let paths = git
                    .changes
                    .iter()
                    .filter(|change| change.staged != stage)
                    .map(|change| change.path.clone())
                    .collect::<Vec<_>>();
                if !paths.is_empty() {
                    self.git_act(
                        Request::Stage {
                            directory,
                            paths,
                            stage,
                        },
                        None,
                    );
                }
            }
            GitAction::Pick(path, pick) => git.pick(&path, pick),
            GitAction::Discard(paths) => {
                git.menu = None;
                git.prompt = Some(GitPrompt::Discard(paths));
            }
            GitAction::Ignore(paths) => self.git_act(Request::Ignore { directory, paths }, None),
            GitAction::Open(path) => {
                git.menu = None;
                git.diff = Some(path);
                git.revision = None;
                self.workspace.open_files.tab = SessionTab::Diff;
                self.git_load_diff();
                self.git_prepare_file();
            }
            GitAction::CloseDiff => {
                git.diff = None;
                self.workspace.open_files.tab = SessionTab::Chat;
            }
            GitAction::Menu(menu) => {
                git.branch_menu = None;
                git.menu = menu;
            }
            GitAction::Ask(prompt) => {
                git.menu = None;
                git.branch_menu = None;
                git.prompt = Some(prompt);
            }
            GitAction::PromptText(text) => match &mut git.prompt {
                Some(GitPrompt::NewBranch(name)) | Some(GitPrompt::Branch { name, .. }) => {
                    *name = text
                }
                _ => {}
            },
            GitAction::Confirm => self.git_confirm(),
            GitAction::Cancel => git.prompt = None,
            GitAction::BranchMenu(open) => {
                git.menu = None;
                git.branch_menu = open.then(BranchMenu::default);
            }
            GitAction::BranchFilter(filter) => {
                if let Some(menu) = &mut git.branch_menu {
                    menu.filter = filter;
                }
            }
            GitAction::Checkout(branch) => {
                git.branch_menu = None;
                self.git_act(Request::Checkout { directory, branch }, None);
            }
            GitAction::CreateBranch(name) => {
                git.branch_menu = None;
                let name = name.trim().to_owned();
                if !name.is_empty() {
                    self.git_act(Request::CreateBranch { directory, name }, None);
                }
            }
            GitAction::Sync(operation) if operation == "force_push" => {
                git.menu = None;
                git.prompt = Some(GitPrompt::ForcePush);
            }
            GitAction::Sync(operation) => {
                self.git_act(
                    Request::Sync {
                        directory,
                        operation,
                    },
                    None,
                );
            }
            GitAction::UnsetUpstream => {
                let branch = git.branch.clone();
                self.git_act(
                    Request::BranchOperation {
                        directory,
                        action: "unset_upstream".to_owned(),
                        branch,
                        target: None,
                        remote: None,
                        with_remote: false,
                    },
                    None,
                )
            }
            GitAction::Fold(section) => match section {
                GitSection::Changes => git.changes_open = !git.changes_open,
                GitSection::Graph => git.graph_open = !git.graph_open,
            },
            GitAction::GraphAll(all) => {
                git.menu = None;
                git.graph_all = all;
                self.git_refresh();
            }
            GitAction::Dismiss => git.notice = None,
            action @ (GitAction::Note(_)
            | GitAction::RemoveNote(_)
            | GitAction::SendNotes
            | GitAction::ShowRevision { .. }
            | GitAction::CloseRevision
            | GitAction::LoadDiffs(_)
            | GitAction::TakeConflict(..)
            | GitAction::TakeAllConflicts(_)
            | GitAction::Resolve) => self.git_review(action),
            GitAction::Step(step) => {
                let message = (step == "continue")
                    .then(|| git.message.trim().to_owned())
                    .filter(|message| !message.is_empty());
                if message.is_some() {
                    git.message.clear();
                }
                self.git_act(
                    Request::Step {
                        directory,
                        action: step,
                        message,
                    },
                    None,
                );
            }
            GitAction::Repository(submodule) => {
                let root = git.root.clone();
                let folder = match &submodule {
                    Some(path) => format!("{}/{path}", root.trim_end_matches('/')),
                    None => root.clone(),
                };
                let kept = std::mem::take(git);
                *git = GitSummary {
                    folder,
                    root,
                    base: kept.base,
                    submodules: kept.submodules,
                    submodule,
                    loaded: true,
                    repository: true,
                    changes_open: kept.changes_open,
                    graph_open: kept.graph_open,
                    graph_all: kept.graph_all,
                    notes: kept.notes,
                    ..GitSummary::default()
                };
                self.git_refresh();
            }
            GitAction::Stash(stash, index) => {
                let message = (stash == "push")
                    .then(|| git.message.trim().to_owned())
                    .filter(|message| !message.is_empty());
                self.git_act(
                    Request::Stash {
                        directory,
                        action: stash,
                        index,
                        message,
                    },
                    None,
                );
            }
        }
    }

    /// Presses the main button: commits, syncs or publishes, as it reads now.
    fn git_primary(&mut self) {
        match self.workspace.git.primary() {
            Primary::Commit { ready: true } => self.git_commit(false, false),
            Primary::Sync { .. } => self.git(GitAction::Sync("sync".to_owned())),
            Primary::Publish => {
                let branch = self.workspace.git.branch.clone();
                self.git(GitAction::Ask(GitPrompt::Branch {
                    task: BranchTask::Publish,
                    name: branch.clone(),
                    branch,
                }));
            }
            Primary::Commit { ready: false } | Primary::Busy(_) => {}
        }
    }

    /// Commits the message, rewriting the last commit when `amend`, then pushes when `push`.
    fn git_commit(&mut self, amend: bool, push: bool) {
        if self.workspace.git.operation.is_some() && !amend {
            self.git(GitAction::Step("continue".to_owned()));
            return;
        }
        let git = &mut self.workspace.git;
        let message = git.message.trim().to_owned();
        if message.is_empty() && !amend {
            git.notice = Some("Write a commit message first.".to_owned());
            return;
        }
        let tracked = !git.changes.iter().any(|change| change.staged);
        let directory = git.folder.clone();
        git.message.clear();
        let then = push.then(|| Request::Sync {
            directory: directory.clone(),
            operation: "push".to_owned(),
        });
        self.git_act(
            Request::Commit {
                directory,
                message,
                tracked,
                amend,
            },
            then,
        );
    }

    /// Does what the open question asks about, and closes it.
    fn git_confirm(&mut self) {
        let Some(prompt) = self.workspace.git.prompt.take() else {
            return;
        };
        let directory = self.workspace.git.folder.clone();
        match prompt {
            GitPrompt::Discard(paths) => self.git_act(Request::Discard { directory, paths }, None),
            GitPrompt::Amend => self.git_commit(true, false),
            GitPrompt::ForcePush => self.git_act(
                Request::Sync {
                    directory,
                    operation: "force_push".to_owned(),
                },
                None,
            ),
            GitPrompt::NewBranch(name) => self.git(GitAction::CreateBranch(name)),
            GitPrompt::Branch { task, branch, name } => {
                let name = name.trim().to_owned();
                let target = match task {
                    BranchTask::Rename | BranchTask::Publish => Some(name.clone()),
                    BranchTask::Delete | BranchTask::Merge | BranchTask::Rebase => None,
                };
                if target.as_ref().is_some_and(String::is_empty) {
                    self.workspace.git.prompt = Some(GitPrompt::Branch { task, branch, name });
                    return;
                }
                let remote = (task == BranchTask::Publish)
                    .then(|| self.workspace.git.remotes.first().cloned())
                    .flatten();
                self.git_act(
                    Request::BranchOperation {
                        directory,
                        action: task.action().to_owned(),
                        branch,
                        target,
                        remote,
                        with_remote: false,
                    },
                    None,
                );
            }
        }
    }
}

/// The word the panel shows while `request` runs.
fn busy_word(request: &Request) -> &'static str {
    match request {
        Request::Commit { amend: true, .. } => "Amending",
        Request::Commit { .. } => "Committing",
        Request::Sync { operation, .. } => match operation.as_str() {
            "fetch" => "Fetching",
            "pull" | "pull_rebase" => "Pulling",
            "push" | "force_push" => "Pushing",
            _ => "Syncing",
        },
        Request::Checkout { .. } | Request::CreateBranch { .. } => "Switching",
        Request::Discard { .. } => "Discarding",
        Request::Stage { .. } => "Staging",
        Request::Ignore { .. } => "Ignoring",
        Request::Step { action, .. } => match action.as_str() {
            "abort" => "Aborting",
            "skip" => "Skipping",
            _ => "Continuing",
        },
        Request::Stash { action, .. } => match action.as_str() {
            "push" => "Stashing",
            "drop" => "Dropping",
            _ => "Applying",
        },
        Request::BranchOperation { action, .. } => match action.as_str() {
            "publish" => "Publishing",
            "merge" => "Merging",
            "rebase" => "Rebasing",
            "delete" => "Deleting",
            "rename" => "Renaming",
            _ => "Updating",
        },
        _ => "Working",
    }
}

/// The panel's view of `change`, keeping the diff loaded earlier for the same file from `kept` when the server sent none.
pub(super) fn changed_file(change: pm_git::Change, kept: &[ChangedFile]) -> ChangedFile {
    let status = if change.conflicted {
        'U'
    } else if change.untracked {
        '?'
    } else if change.staged {
        change.index.unwrap_or('M')
    } else {
        change.worktree.or(change.index).unwrap_or('M')
    };
    let (counted_added, counted_removed) = tally(&change.diff);
    let fetched = !change.diff.is_empty();
    let diff = if fetched {
        change.diff
    } else {
        kept.iter()
            .find(|old| old.path == change.path && old.staged == change.staged)
            .map(|old| old.diff.clone())
            .unwrap_or_default()
    };
    let partly = !change.untracked
        && !change.conflicted
        && change.index.is_some()
        && change.worktree.is_some();
    ChangedFile {
        path: change.path,
        status,
        added: change.added.max(counted_added),
        removed: change.removed.max(counted_removed),
        staged: change.staged,
        untracked: change.untracked,
        conflicted: change.conflicted,
        partly,
        fetched,
        from: change.from,
        diff,
    }
}

/// The graph's view of `commit`.
fn graph_commit(commit: pm_git::Commit) -> GraphCommit {
    GraphCommit {
        id: commit.id,
        summary: commit.summary,
        refs: commit.refs,
        lane: commit.lanes.lane,
        color: commit.lanes.color,
        width: commit.lanes.width,
        edges: commit
            .lanes
            .edges
            .into_iter()
            .map(|edge| GraphEdge {
                upper: edge.half == pm_git::Half::Upper,
                from: edge.from,
                to: edge.to,
                color: edge.color,
            })
            .collect(),
    }
}
