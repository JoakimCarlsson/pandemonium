//! The branches a worktree can move between, and the moves themselves.

use pm_host::Location;

use crate::git::run::{Said, answer, git};

/// A local branch in a repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    /// The branch name, relative to `refs/heads`.
    name: String,
    /// Whether this is the branch the worktree has checked out.
    current: bool,
    /// Whether the name belongs to `refs/remotes` rather than `refs/heads`.
    remote: bool,
    /// Whether the branch followed a remote branch that has since been deleted.
    gone: bool,
    /// The latest commit's author, relative age and subject.
    detail: String,
}

impl Branch {
    /// The branch name, relative to `refs/heads`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this is the branch the worktree has checked out.
    pub fn is_current(&self) -> bool {
        self.current
    }

    /// Whether this branch is a remote-tracking branch.
    pub fn is_remote(&self) -> bool {
        self.remote
    }

    /// Whether the branch followed a remote branch that has since been
    /// deleted, as a branch whose pull request was merged does.
    pub fn is_gone(&self) -> bool {
        self.gone
    }

    /// The latest commit's author, relative age and subject.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Every local and remote-tracking branch of the repository at `root`,
/// current branch first.
///
/// A remote's symbolic `HEAD` is left out, and so is a remote-tracking
/// branch a local branch follows: each names a branch that is listed under
/// its own name already.
pub fn branches(root: impl Into<Location>) -> Vec<Branch> {
    let root = root.into();
    let Some(output) = answer(
        &root,
        [
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(HEAD)%00%(refname)%00%(refname:short)%00%(upstream)%00%(upstream:track)%00%(authorname)%00%(committerdate:relative)%00%(contents:subject)",
            "refs/heads",
            "refs/remotes",
        ],
    ) else {
        return Vec::new();
    };

    let lines = output
        .lines()
        .map(|line| line.split('\0').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let followed = lines
        .iter()
        .filter_map(|fields| fields.get(3).copied())
        .filter(|upstream| !upstream.is_empty())
        .collect::<Vec<_>>();
    let mut branches = lines
        .iter()
        .filter_map(|fields| {
            let mut fields = fields.iter().copied();
            let head = fields.next()?;
            let reference = fields.next()?;
            let name = fields.next()?;
            let upstream = fields.next().unwrap_or_default();
            let track = fields.next().unwrap_or_default();
            let author = fields.next().unwrap_or_default();
            let age = fields.next().unwrap_or_default();
            let subject = fields.next().unwrap_or_default();
            let symbolic = reference.starts_with("refs/remotes/") && reference.ends_with("/HEAD");
            let duplicate = followed.contains(&reference);
            (!name.is_empty() && !symbolic && !duplicate).then(|| Branch {
                name: name.to_owned(),
                current: head == "*",
                remote: reference.starts_with("refs/remotes/"),
                gone: !upstream.is_empty() && track.contains("gone"),
                detail: [author, age, subject]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
            })
        })
        .collect::<Vec<_>>();
    branches.sort_by_key(|branch| (branch.remote, !branch.current));
    branches
}

/// Checks out the local branch called `name` in the worktree at `root`.
pub fn switch_branch(root: impl Into<Location>, name: &str) -> Said {
    let root = root.into();
    if name.trim().is_empty() {
        return Err("a branch needs a name".to_owned());
    }
    git(&root, ["switch", name])
}

/// Creates and checks out a local branch called `name` at the current commit.
pub fn create_branch(root: impl Into<Location>, name: &str) -> Said {
    let root = root.into();
    if name.trim().is_empty() {
        return Err("a branch needs a name".to_owned());
    }
    git(&root, ["switch", "-c", name])
}

/// Pushes the checked-out branch, publishing it to a remote when needed.
pub fn push_branch(root: impl Into<Location>, has_upstream: bool) -> Said {
    let root = root.into();
    if has_upstream {
        return git(&root, ["push"]);
    }

    let remotes = answer(&root, ["remote"]).unwrap_or_default();
    let remote = remotes
        .lines()
        .find(|remote| *remote == "origin")
        .or_else(|| remotes.lines().next())
        .ok_or_else(|| "this repository has no remote to publish to".to_owned())?;
    git(&root, ["push", "--set-upstream", remote, "HEAD"])
}

/// Fetches updates from every configured remote, forgetting the
/// remote-tracking branches whose remote branch is gone.
pub fn fetch(root: impl Into<Location>) -> Said {
    let root = root.into();
    git(&root, ["fetch", "--all", "--prune"])
}

/// Configured remote names of the repository at `root`.
pub fn remotes(root: impl Into<Location>) -> Vec<String> {
    let root = root.into();
    answer(&root, ["remote"])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Fetches updates from one configured `remote`, forgetting the
/// remote-tracking branches whose remote branch is gone.
pub fn fetch_from(root: impl Into<Location>, remote: &str) -> Said {
    let root = root.into();
    git(&root, ["fetch", "--prune", remote])
}

/// Pulls the checked-out branch, rebasing when `rebase` asks for it, and
/// forgets the remote-tracking branches whose remote branch is gone.
pub fn pull(root: impl Into<Location>, rebase: bool) -> Said {
    let root = root.into();
    match rebase {
        true => git(&root, ["pull", "--prune", "--rebase"]),
        false => git(&root, ["pull", "--prune"]),
    }
}

/// Brings the branch level with the one it follows: pulls what it is behind
/// by, then pushes what it is ahead by.
///
/// Pulling first is what lets the push through when both sides have moved;
/// a pull that stops on a conflict leaves the push unattempted.
pub fn sync(root: impl Into<Location>) -> Said {
    let root = root.into();
    let pulled = pull(&root, false)?;
    let pushed = git(&root, ["push"])?;
    Ok(pulled + &pushed)
}

/// Force-pushes the checked-out branch without overwriting unseen remote work.
pub fn force_push(root: impl Into<Location>) -> Said {
    let root = root.into();
    git(&root, ["push", "--force-with-lease"])
}

/// Pushes the checked-out branch to one configured `remote`.
pub fn push_to(root: impl Into<Location>, remote: &str) -> Said {
    let root = root.into();
    git(&root, ["push", remote, "HEAD"])
}
