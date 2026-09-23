//! The branches a worktree can move between, and the moves themselves.

use std::path::Path;

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

    /// The latest commit's author, relative age and subject.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Every local branch of the repository at `root`, current branch first.
pub fn branches(root: &Path) -> Vec<Branch> {
    let Some(output) = answer(
        root,
        [
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(HEAD)%00%(refname)%00%(refname:short)%00%(authorname)%00%(committerdate:relative)%00%(contents:subject)",
            "refs/heads",
            "refs/remotes",
        ],
    ) else {
        return Vec::new();
    };

    let mut branches = output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let head = fields.next()?;
            let reference = fields.next()?;
            let name = fields.next()?;
            let author = fields.next().unwrap_or_default();
            let age = fields.next().unwrap_or_default();
            let subject = fields.next().unwrap_or_default();
            (!name.is_empty()).then(|| Branch {
                name: name.to_owned(),
                current: head == "*",
                remote: reference.starts_with("refs/remotes/"),
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
pub fn switch_branch(root: &Path, name: &str) -> Said {
    if name.trim().is_empty() {
        return Err("a branch needs a name".to_owned());
    }
    git(root, ["switch", name])
}

/// Creates and checks out a local branch called `name` at the current commit.
pub fn create_branch(root: &Path, name: &str) -> Said {
    if name.trim().is_empty() {
        return Err("a branch needs a name".to_owned());
    }
    git(root, ["switch", "-c", name])
}

/// Pushes the checked-out branch, publishing it to a remote when needed.
pub fn push_branch(root: &Path, has_upstream: bool) -> Said {
    if has_upstream {
        return git(root, ["push"]);
    }

    let remotes = answer(root, ["remote"]).unwrap_or_default();
    let remote = remotes
        .lines()
        .find(|remote| *remote == "origin")
        .or_else(|| remotes.lines().next())
        .ok_or_else(|| "this repository has no remote to publish to".to_owned())?;
    git(root, ["push", "--set-upstream", remote, "HEAD"])
}

/// Fetches updates from every configured remote.
pub fn fetch(root: &Path) -> Said {
    git(root, ["fetch", "--all"])
}

/// Configured remote names of the repository at `root`.
pub fn remotes(root: &Path) -> Vec<String> {
    answer(root, ["remote"])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Fetches updates from one configured `remote`.
pub fn fetch_from(root: &Path, remote: &str) -> Said {
    git(root, ["fetch", remote])
}

/// Pulls the checked-out branch, rebasing when `rebase` asks for it.
pub fn pull(root: &Path, rebase: bool) -> Said {
    match rebase {
        true => git(root, ["pull", "--rebase"]),
        false => git(root, ["pull"]),
    }
}

/// Brings the branch level with the one it follows: pulls what it is behind
/// by, then pushes what it is ahead by.
///
/// Pulling first is what lets the push through when both sides have moved;
/// a pull that stops on a conflict leaves the push unattempted.
pub fn sync(root: &Path) -> Said {
    let pulled = pull(root, false)?;
    let pushed = git(root, ["push"])?;
    Ok(pulled + &pushed)
}

/// Force-pushes the checked-out branch without overwriting unseen remote work.
pub fn force_push(root: &Path) -> Said {
    git(root, ["push", "--force-with-lease"])
}

/// Pushes the checked-out branch to one configured `remote`.
pub fn push_to(root: &Path, remote: &str) -> Said {
    git(root, ["push", remote, "HEAD"])
}
