//! Merges, rebases and cherry-picks that stopped and wait for their next step.

use std::fs;
use std::path::{Path, PathBuf};

use crate::multirepo::{self, Layout};
use crate::{layout, run};

/// An operation waiting for its next step.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Operation {
    /// A merge awaiting conflict resolution or a commit.
    Merge {
        /// The incoming commit's object name.
        incoming: String,
        /// The message git prepared for the merge commit.
        message: String,
    },
    /// A rebase replaying commits.
    Rebase {
        /// The target commit or branch.
        onto: String,
        /// The branch being rebased.
        branch: String,
        /// The current replay step.
        step: usize,
        /// The total replay steps.
        total: usize,
        /// The message of the commit currently being replayed.
        message: String,
    },
    /// A cherry-pick awaiting conflict resolution.
    CherryPick {
        /// The commit being picked.
        commit: String,
        /// The message git prepared for the picked commit.
        message: String,
    },
}

impl Operation {
    /// A short label for the operation in progress.
    pub fn label(&self) -> String {
        match self {
            Self::Merge { message, .. } => {
                let summary = message.lines().next().unwrap_or_default();
                summary.strip_prefix("Merge ").map_or_else(
                    || "Merging".to_owned(),
                    |incoming| format!("Merging {incoming}"),
                )
            }
            Self::Rebase {
                onto, step, total, ..
            } => format!("Rebasing {step}/{total} onto {onto}"),
            Self::CherryPick { commit, .. } => {
                format!("Cherry-picking {}", &commit[..commit.len().min(7)])
            }
        }
    }

    /// The verb naming this operation in menu actions.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Merge { .. } => "Merge",
            Self::Rebase { .. } => "Rebase",
            Self::CherryPick { .. } => "Cherry-Pick",
        }
    }
}

/// Resolves a git control file or directory of the repository `directory` is in, linked worktrees included.
fn git_path(directory: &Path, name: &str) -> Option<PathBuf> {
    let path = run(directory, &["rev-parse", "--git-path", name]).ok()?;
    let path = Path::new(path.trim());
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    })
}

/// Reads `name` in the git folder of the repository `directory` is in.
fn git_file(directory: &Path, name: &str) -> Option<String> {
    fs::read_to_string(git_path(directory, name)?)
        .ok()
        .map(|value| value.trim().to_owned())
}

/// Reads the message git prepared, without its instructional comment lines.
fn prepared_message(directory: &Path) -> String {
    git_file(directory, "MERGE_MSG")
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

/// The operation waiting in the repository `directory` is in, if there is one. Of a folder of repositories, the one waiting in the first that has one.
pub fn operation(directory: &Path) -> Option<Operation> {
    if let Layout::Repositories(children) = layout(directory) {
        return children
            .iter()
            .find_map(|child| operation(&directory.join(child)));
    }
    for folder in ["rebase-merge", "rebase-apply"] {
        if git_path(directory, folder)?.is_dir() {
            let read = |name: &str| git_file(directory, &format!("{folder}/{name}"));
            let onto = read("onto").unwrap_or_default();
            let onto = run(directory, &["name-rev", "--name-only", &onto])
                .ok()
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty() && name != "undefined")
                .unwrap_or(onto);
            return Some(Operation::Rebase {
                onto,
                branch: read("head-name")
                    .unwrap_or_default()
                    .trim_start_matches("refs/heads/")
                    .to_owned(),
                step: read("msgnum")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
                total: read("end")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0),
                message: read("message")
                    .or_else(|| {
                        run(directory, &["log", "-1", "--pretty=%B", "REBASE_HEAD"])
                            .ok()
                            .map(|value| value.trim().to_owned())
                    })
                    .unwrap_or_default(),
            });
        }
    }
    if let Some(commit) = git_file(directory, "CHERRY_PICK_HEAD") {
        return Some(Operation::CherryPick {
            commit,
            message: prepared_message(directory),
        });
    }
    let incoming = git_file(directory, "MERGE_HEAD")?;
    Some(Operation::Merge {
        incoming,
        message: prepared_message(directory),
    })
}

/// Takes the next step of the operation waiting in `directory`: `continue` it, finishing a merge or a cherry-pick under `message` when there is one, `skip` the commit it stopped on, or `abort` it.
pub fn step(directory: &Path, action: &str, message: Option<&str>) -> Result<(), String> {
    if let Layout::Repositories(children) = layout(directory) {
        let waiting: Vec<String> = children
            .into_iter()
            .filter(|child| operation(&directory.join(child)).is_some())
            .collect();
        if waiting.is_empty() {
            return Err("Nothing is waiting".to_owned());
        }
        return multirepo::each(directory, &waiting, |child| step(child, action, message));
    }
    let waiting = operation(directory).ok_or("Nothing is waiting")?;
    match (action, &waiting) {
        ("abort", Operation::Merge { .. }) => run(directory, &["merge", "--abort"]),
        ("abort", Operation::Rebase { .. }) => run(directory, &["rebase", "--abort"]),
        ("abort", Operation::CherryPick { .. }) => run(directory, &["cherry-pick", "--abort"]),
        ("skip", Operation::Rebase { .. }) => run(directory, &["rebase", "--skip"]),
        ("skip", Operation::CherryPick { .. }) => run(directory, &["cherry-pick", "--skip"]),
        ("skip", Operation::Merge { .. }) => Err("A merge cannot skip a commit".to_owned()),
        (
            "continue",
            Operation::Merge {
                message: prepared, ..
            },
        ) => run(directory, &["commit", "-m", message.unwrap_or(prepared)]),
        ("continue", Operation::Rebase { .. }) => run(
            directory,
            &["-c", "core.editor=true", "rebase", "--continue"],
        ),
        (
            "continue",
            Operation::CherryPick {
                message: prepared, ..
            },
        ) => {
            let path = git_path(directory, "MERGE_MSG").ok_or("Cannot find MERGE_MSG")?;
            fs::write(path, message.unwrap_or(prepared)).map_err(|error| error.to_string())?;
            run(
                directory,
                &["-c", "core.editor=true", "cherry-pick", "--continue"],
            )
        }
        (other, _) => Err(format!("{other} is not a step of an operation")),
    }
    .map(|_| ())
}
