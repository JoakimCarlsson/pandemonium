//! The worktrees of a repository, as `git worktree list --porcelain` lists them, leaving out the ones the agent harness makes for its isolated subagents.
//!
//! The harness runs a subagent that works apart from its agent in a git worktree of its own, made from the session's worktree on a detached head in the harness's cache folder, and removes it when the subagent ends. Those worktrees belong to the session that made them, not to a session of their own, so nothing that lists worktrees counts them.

use std::path::Path;

use crate::run;

/// The reason the agent harness locks the worktrees of its isolated subagents with, which is how they are told apart; the harness's `omc_agent::LOCK_REASON`.
pub const HARNESS_LOCK_REASON: &str = "oh-my-crab task worktree";

/// The part of a path every worktree of the harness's subagents has, inside the harness's cache folder, for one whose lock was taken away.
const HARNESS_FOLDER: &str = "oh-my-crab/worktrees/";

/// One worktree of a repository, as git lists it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Listed {
    /// Its folder.
    pub(crate) path: String,
    /// The branch it has checked out, as a full reference such as `refs/heads/main`, or nothing for a detached head.
    pub(crate) branch: Option<String>,
    /// Why it is locked, when it is; empty for a lock without a reason.
    pub(crate) locked: Option<String>,
}

impl Listed {
    /// Whether the agent harness made it for an isolated subagent: locked with [`HARNESS_LOCK_REASON`], or in the harness's folder of them.
    pub(crate) fn is_harness(&self) -> bool {
        self.locked.as_deref() == Some(HARNESS_LOCK_REASON)
            || self.path.replace('\\', "/").contains(HARNESS_FOLDER)
    }
}

/// The worktrees `listing`, the output of `git worktree list --porcelain`, holds, in its order.
pub(crate) fn parse(listing: &str) -> Vec<Listed> {
    listing
        .split("\n\n")
        .filter_map(|block| {
            let mut found = Listed::default();
            for line in block.lines() {
                if let Some(path) = line.strip_prefix("worktree ") {
                    found.path = path.to_owned();
                } else if let Some(branch) = line.strip_prefix("branch ") {
                    found.branch = Some(branch.to_owned());
                } else if line == "locked" {
                    found.locked = Some(String::new());
                } else if let Some(reason) = line.strip_prefix("locked ") {
                    found.locked = Some(reason.to_owned());
                }
            }
            (!found.path.is_empty()).then_some(found)
        })
        .collect()
}

/// The worktrees of the repository `directory` is in, other than those the agent harness made for isolated subagents; nothing when git cannot list them.
pub(crate) fn worktrees(directory: &Path) -> Vec<Listed> {
    run(directory, &["worktree", "list", "--porcelain"])
        .map(|listing| parse(&listing))
        .unwrap_or_default()
        .into_iter()
        .filter(|worktree| !worktree.is_harness())
        .collect()
}

#[cfg(test)]
/// Tests of listing worktrees without the harness's.
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;
    use crate::branch_info;

    /// Runs `git` with `arguments` in `directory` and expects it to succeed.
    fn git(directory: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(arguments)
            .current_dir(directory)
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// The listing is read block by block, with branches, detached heads and locks with and without a reason, and the harness's worktrees are told apart by their lock or their folder.
    #[test]
    fn listings_are_parsed_and_harness_worktrees_recognised() {
        let listing = "worktree /r\nHEAD 1\nbranch refs/heads/main\n\nworktree /r/.pandemonium/worktrees/s\nHEAD 2\ndetached\nlocked\n\nworktree /home/u/.cache/oh-my-crab/worktrees/c/1/r\nHEAD 3\ndetached\nlocked oh-my-crab task worktree\n\nworktree /home/u/.cache/oh-my-crab/worktrees/c/2/r\nHEAD 4\ndetached\n";
        let listed = parse(listing);
        assert_eq!(listed.len(), 4);
        assert_eq!(listed[0].branch.as_deref(), Some("refs/heads/main"));
        assert_eq!(listed[1].locked.as_deref(), Some(""));
        assert_eq!(
            listed.iter().map(Listed::is_harness).collect::<Vec<_>>(),
            vec![false, false, true, true]
        );
    }

    /// A branch checked out only in a worktree the harness made for a subagent is not taken to be checked out elsewhere, while one in a session's worktree is.
    #[test]
    fn harness_worktrees_do_not_hold_branches() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build")
            .join(format!("harness-worktrees-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let main = root.join("main");
        fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        git(&main, &["commit", "-q", "--allow-empty", "-m", "start"]);
        git(&main, &["branch", "held"]);
        git(&main, &["branch", "session"]);
        let harness = root.join("cache/oh-my-crab/worktrees/c/1/main");
        let session = root.join("session");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "--lock",
                "--reason",
                HARNESS_LOCK_REASON,
                &harness.to_string_lossy(),
                "held",
            ],
        );
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                &session.to_string_lossy(),
                "session",
            ],
        );
        assert_eq!(branch_info(&main, Some("held")).elsewhere, None);
        assert!(branch_info(&main, Some("session")).elsewhere.is_some());
        assert_eq!(worktrees(&main).len(), 2);
        git(
            &main,
            &[
                "worktree",
                "remove",
                "--force",
                "--force",
                &harness.to_string_lossy(),
            ],
        );
        let _ = fs::remove_dir_all(&root);
    }
}
