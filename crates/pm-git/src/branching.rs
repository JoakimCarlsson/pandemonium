//! Branches and the remotes they follow: publishing, the upstream, renaming, deleting, merging and rebasing.

use std::path::Path;

use crate::multirepo::{self, Layout};
use crate::{layout, operation, push_to, run, tracking};

/// What the controls over branches and remotes need to know about a repository.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BranchInfo {
    /// The remotes the repository has; of a folder of repositories, those every one has.
    pub remotes: Vec<String>,
    /// The branches of those remotes, each written with its remote's name in front, such as `origin/main`.
    pub remote_branches: Vec<String>,
    /// The commits the asked branch has that the checked out one has not.
    pub unmerged: usize,
    /// The commits the asked branch has that the branch it follows has not, or that no remote has when it follows none.
    pub unpushed: usize,
    /// Where the asked branch is checked out, when that is another worktree than this one.
    pub elsewhere: Option<String>,
    /// The remote branch the asked branch follows, such as `origin/main`, when it follows one.
    pub follows: Option<String>,
}

/// The names of the remotes of the repository `directory` is in.
pub(crate) fn remotes(directory: &Path) -> Vec<String> {
    run(directory, &["remote"])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The branches of the remotes of the repository `directory` is in, each with its remote's name in front.
fn remote_branches(directory: &Path) -> Vec<String> {
    run(
        directory,
        &["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
    )
    .unwrap_or_default()
    .lines()
    .filter(|name| name.contains('/') && !name.ends_with("/HEAD"))
    .map(str::to_owned)
    .collect()
}

/// How many commits `range` lists in the repository `directory` is in, none when it names something unknown.
fn count(directory: &Path, range: &[&str]) -> usize {
    let mut arguments = vec!["rev-list", "--count"];
    arguments.extend_from_slice(range);
    run(directory, &arguments)
        .ok()
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or(0)
}

/// The folder of the other worktree of the repository `directory` is in that has `branch` checked out, when there is one.
fn checked_out_elsewhere(directory: &Path, branch: &str) -> Option<String> {
    let here = run(directory, &["rev-parse", "--show-toplevel"]).ok()?;
    let listing = run(directory, &["worktree", "list", "--porcelain"]).ok()?;
    let wanted = format!("branch refs/heads/{branch}");
    listing
        .split("\n\n")
        .filter(|block| block.lines().any(|line| line == wanted))
        .filter_map(|block| {
            block
                .lines()
                .find_map(|line| line.strip_prefix("worktree "))
        })
        .find(|path| *path != here.trim())
        .map(str::to_owned)
}

/// The remote and the branch on it that the local branch `branch` follows, when it follows one.
fn followed(directory: &Path, branch: &str) -> Option<(String, String)> {
    let setting = |key: &str| {
        run(
            directory,
            &["config", "--get", &format!("branch.{branch}.{key}")],
        )
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    };
    let remote = setting("remote").filter(|remote| remote != ".")?;
    let merge = setting("merge")?;
    Some((remote, merge.strip_prefix("refs/heads/")?.to_owned()))
}

/// What the controls need to know about the repository `directory` is in, and about `branch` there when one is asked about. Of a folder of repositories, what every one of them has, with the commits added up.
pub fn branch_info(directory: &Path, branch: Option<&str>) -> BranchInfo {
    if let Layout::Repositories(children) = layout(directory) {
        let each: Vec<BranchInfo> = children
            .iter()
            .map(|child| branch_info(&directory.join(child), branch))
            .collect();
        let common = |pick: fn(&BranchInfo) -> &Vec<String>| -> Vec<String> {
            each.first()
                .map(|first| {
                    pick(first)
                        .iter()
                        .filter(|name| each.iter().all(|held| pick(held).contains(name)))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        return BranchInfo {
            remotes: common(|held| &held.remotes),
            remote_branches: common(|held| &held.remote_branches),
            unmerged: each.iter().map(|held| held.unmerged).sum(),
            unpushed: each.iter().map(|held| held.unpushed).sum(),
            elsewhere: each.iter().find_map(|held| held.elsewhere.clone()),
            follows: each
                .iter()
                .all(|held| held.follows.is_some())
                .then(|| each.first().and_then(|held| held.follows.clone()))
                .flatten(),
        };
    }
    let mut info = BranchInfo {
        remotes: remotes(directory),
        remote_branches: remote_branches(directory),
        ..BranchInfo::default()
    };
    if let Some(branch) = branch {
        let heads = format!("refs/heads/{branch}");
        info.unmerged = count(directory, &[&format!("HEAD..{heads}")]);
        let follows = followed(directory, branch);
        info.follows = follows
            .as_ref()
            .map(|(remote, name)| format!("{remote}/{name}"));
        info.unpushed = if follows.is_some() {
            count(
                directory,
                &[&heads, "--not", &format!("{branch}@{{upstream}}")],
            )
        } else {
            count(directory, &[&heads, "--not", "--remotes"])
        };
        info.elsewhere = checked_out_elsewhere(directory, branch);
    }
    info
}

/// Refuses to change the branch `branch` when it is checked out in another worktree, such as that of another session.
fn refuse_elsewhere(directory: &Path, branch: &str) -> Result<(), String> {
    match checked_out_elsewhere(directory, branch) {
        Some(path) => Err(format!("{branch} is checked out in {path}")),
        None => Ok(()),
    }
}

/// Publishes the checked out branch to `remote` as `name`, and makes it follow that branch.
fn publish(directory: &Path, remote: &str, name: &str) -> Result<(), String> {
    if !remotes(directory).iter().any(|held| held == remote) {
        return Err(format!("there is no remote called {remote}"));
    }
    if name.trim().is_empty() {
        return Err("the branch on the remote needs a name".to_owned());
    }
    push_to(directory, Some((remote, name)))
}

/// Makes the checked out branch follow `upstream`, a branch of a remote, or follow nothing when there is none.
fn set_upstream(directory: &Path, upstream: Option<&str>) -> Result<(), String> {
    match upstream {
        Some(upstream) => run(
            directory,
            &["branch", &format!("--set-upstream-to={upstream}")],
        ),
        None => run(directory, &["branch", "--unset-upstream"]),
    }
    .map(|_| ())
}

/// Renames the local branch `branch` to `name`, and the branch it follows on its remote with it when `with_remote` is set, after which it follows the renamed one.
fn rename(directory: &Path, branch: &str, name: &str, with_remote: bool) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("the branch needs a name".to_owned());
    }
    refuse_elsewhere(directory, branch)?;
    let remote = if with_remote {
        Some(followed(directory, branch).ok_or_else(|| {
            format!("{branch} follows no branch on a remote, so there is none to rename")
        })?)
    } else {
        None
    };
    if let Some((remote, _)) = &remote {
        run(
            directory,
            &[
                "push",
                remote,
                &format!("refs/heads/{branch}:refs/heads/{name}"),
            ],
        )?;
    }
    run(directory, &["branch", "--move", branch, name])?;
    if let Some((remote, old)) = remote {
        run(
            directory,
            &[
                "branch",
                &format!("--set-upstream-to={remote}/{name}"),
                name,
            ],
        )?;
        run(directory, &["push", &remote, "--delete", &old])?;
    }
    Ok(())
}

/// Deletes the local branch `branch`, and the branch it follows on its remote with it when `with_remote` is set. The checked out branch and one checked out in another worktree are kept.
fn delete(directory: &Path, branch: &str, with_remote: bool) -> Result<(), String> {
    if crate::current_branch(directory).as_deref() == Some(branch) {
        return Err(format!("{branch} is checked out, so it cannot be deleted"));
    }
    refuse_elsewhere(directory, branch)?;
    let remote = if with_remote {
        Some(followed(directory, branch).ok_or_else(|| {
            format!("{branch} follows no branch on a remote, so there is none to delete")
        })?)
    } else {
        None
    };
    run(directory, &["branch", "--delete", "--force", branch])?;
    if let Some((remote, name)) = remote {
        run(directory, &["push", &remote, "--delete", &name])?;
    }
    Ok(())
}

/// Runs `arguments` as a merge or rebase, counting a stop on conflicts as done, since the conflicts wait in the in-progress operation for the reader to resolve and conclude or abort.
fn until_conflicts(directory: &Path, arguments: &[&str]) -> Result<(), String> {
    match run(directory, arguments) {
        Err(_) if operation(directory).is_some() => Ok(()),
        other => other.map(|_| ()),
    }
}

/// Does the branch or remote `action` to the repository `directory` is in, or to each repository of a folder that holds several, naming those that fail.
///
/// The actions are `publish` the checked out branch to the remote `remote` as `target`; `set_upstream` of the checked out branch to the remote branch `target`; `unset_upstream`; `rename` the local `branch` to `target`, with its remote branch when `with_remote`; `delete` the local `branch`, with its remote branch when `with_remote`; and `merge` or `rebase` the branch `branch` into the checked out one.
///
/// # Errors
///
/// Fails when the action is not one of these, when a name it needs is missing, or when git refuses.
pub fn branch_operation(
    directory: &Path,
    action: &str,
    branch: &str,
    target: Option<&str>,
    remote: Option<&str>,
    with_remote: bool,
) -> Result<(), String> {
    if let Layout::Repositories(children) = layout(directory) {
        return multirepo::each(directory, &children, |child| {
            branch_operation(child, action, branch, target, remote, with_remote)
        });
    }
    let missing = || format!("{action} needs a name");
    match action {
        "publish" => publish(
            directory,
            remote.ok_or_else(|| "publishing needs a remote".to_owned())?,
            target.ok_or_else(missing)?,
        ),
        "set_upstream" => set_upstream(directory, Some(target.ok_or_else(missing)?)),
        "unset_upstream" => {
            if tracking(directory).upstream.is_none() {
                return Err("this branch follows nothing".to_owned());
            }
            set_upstream(directory, None)
        }
        "rename" => rename(directory, branch, target.ok_or_else(missing)?, with_remote),
        "delete" => delete(directory, branch, with_remote),
        "merge" => until_conflicts(directory, &["merge", "--no-edit", branch]),
        "rebase" => until_conflicts(directory, &["rebase", branch]),
        other => Err(format!("{other} is not something to do with a branch")),
    }
}
