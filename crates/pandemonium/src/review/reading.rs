//! What git says about a worktree, read away from the window.
//!
//! Reading a worktree is a handful of subprocesses per repository and two
//! more, plus a parse, per changed file. An agent writes into its worktree
//! several times a second, so that reading is done on a thread of its own
//! and handed to the [`Review`](crate::review::Review) whole once it is done:
//! the frame never waits on git.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pm_core::{Changed, Side, Status};

use crate::review::conflict::{self, Conflict};
use crate::review::repository::History;
use crate::review::shade::Shading;
use crate::review::store::Patch;

/// What one repository of a worktree held when it was read.
pub struct RepositoryReading {
    /// The repository's working-copy root.
    pub root: PathBuf,
    /// What git made of it.
    pub status: Status,
    /// The commits leading up to it.
    pub history: History,
}

/// Everything a review shows of one worktree, as git said it at one moment.
pub struct Reading {
    /// Which of the review's reads this answers, so that one overtaken by a
    /// read made since is dropped rather than put back over it.
    pub(super) reads: u64,
    /// The repositories the worktree holds, the root's own first.
    pub(super) repositories: Vec<RepositoryReading>,
    /// The lines of each file that has changed.
    pub(super) patches: BTreeMap<PathBuf, Patch>,
    /// The unresolved marker blocks of each conflicted file.
    pub(super) conflicts: BTreeMap<PathBuf, Vec<Conflict>>,
    /// The colour of every character each file's lines are drawn in.
    pub(super) shades: BTreeMap<PathBuf, Shading>,
    /// What the worktree wrote down of its review comments, when this is the
    /// first reading and the comments are still to be restored.
    pub(super) remembered: Option<String>,
    /// The text of each file that has comments on it, relative to the
    /// worktree, or nothing where the file is gone.
    pub(super) texts: BTreeMap<PathBuf, Option<String>>,
}

impl Reading {
    /// Reads the worktree at `root`, as the review's `reads`-th read.
    ///
    /// `commented` names the files, relative to `root`, that have review
    /// comments on them, whose text is read for the comments to be followed
    /// to. `restoring` asks for the comments the worktree wrote down.
    pub(super) fn of(root: &Path, reads: u64, commented: Vec<PathBuf>, restoring: bool) -> Self {
        let repositories = pm_core::repositories(root)
            .into_iter()
            .map(|root| RepositoryReading {
                history: History::of(&root),
                status: Status::of(&root),
                root,
            })
            .collect::<Vec<_>>();
        let changed = gather(
            repositories
                .iter()
                .map(|repository| (repository.root.as_path(), repository.status.changed())),
        );

        let mut patches = BTreeMap::<PathBuf, Patch>::new();
        for repository in &repositories {
            for (path, hunks) in pm_core::diffs(&repository.root, Side::Staged) {
                patches.entry(path).or_default().staged = hunks;
            }
            for (path, hunks) in pm_core::diffs(&repository.root, Side::Unstaged) {
                patches.entry(path).or_default().unstaged = hunks;
            }
        }
        let untracked = changed
            .iter()
            .filter(|(_, changed)| changed.is_untracked())
            .map(|(_, changed)| changed.path.as_path())
            .collect::<Vec<_>>();
        for (path, hunks) in untracked.iter().zip(pm_core::untracked(root, &untracked)) {
            patches.entry(path.to_path_buf()).or_default().unstaged = hunks;
        }
        patches.retain(|path, _| changed.iter().any(|(_, changed)| changed.path == *path));
        let conflicts = changed
            .iter()
            .filter(|(_, changed)| changed.is_conflicted())
            .filter_map(|(_, changed)| {
                let source = std::fs::read_to_string(&changed.path).ok()?;
                Some((changed.path.clone(), conflict::conflicts(&source)))
            })
            .collect();
        let shades = Shading::all(root, &patches);
        let remembered = restoring
            .then(|| pm_core::remembered_review(root))
            .flatten();
        let restored = remembered
            .as_deref()
            .map(crate::review::comment::paths_in)
            .unwrap_or_default();
        let texts = commented
            .into_iter()
            .chain(restored)
            .map(|path| {
                let text = std::fs::read_to_string(root.join(&path)).ok();
                (path, text)
            })
            .collect();

        Self {
            reads,
            repositories,
            patches,
            conflicts,
            shades,
            remembered,
            texts,
        }
    }
}

/// Lists every repository's changes as one, each with the place of the
/// repository it is in, leaving out what a repository reports of another
/// inside it.
///
/// A repository inside another is, to the outer one, a directory it has
/// never been told about; its files are the inner repository's to list.
pub(super) fn gather<'a>(
    repositories: impl Iterator<Item = (&'a Path, &'a [Changed])> + Clone,
) -> Vec<(usize, Changed)> {
    let roots = repositories
        .clone()
        .map(|(root, _)| root)
        .collect::<Vec<_>>();
    repositories
        .enumerate()
        .flat_map(|(owner, (root, changes))| {
            let inner = roots
                .iter()
                .filter(|other| **other != root && other.starts_with(root))
                .copied()
                .collect::<Vec<_>>();
            changes
                .iter()
                .filter(move |changed| !inner.iter().any(|other| changed.path.starts_with(other)))
                .map(move |changed| (owner, changed.clone()))
        })
        .collect()
}
