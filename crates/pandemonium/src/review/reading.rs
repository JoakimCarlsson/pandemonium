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
    /// The colour of every character each file's lines are drawn in.
    pub(super) shades: BTreeMap<PathBuf, Shading>,
}

impl Reading {
    /// Reads the worktree at `root`, as the review's `reads`-th read.
    pub(super) fn of(root: &Path, reads: u64) -> Self {
        let repositories = pm_core::repositories(root)
            .into_iter()
            .map(|root| RepositoryReading {
                history: History::of(&root),
                status: Status::of(&root),
                root,
            })
            .collect::<Vec<_>>();
        let roots = repositories
            .iter()
            .map(|repository| repository.root.as_path())
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
        for (owner, changed) in changed.iter().filter(|(_, changed)| changed.is_untracked()) {
            let hunks = pm_core::diff(roots[*owner], &changed.path, Side::Untracked);
            patches.entry(changed.path.clone()).or_default().unstaged = hunks;
        }
        patches.retain(|path, _| changed.iter().any(|(_, changed)| changed.path == *path));
        let shades = patches
            .iter()
            .map(|(path, patch)| {
                let holding = holding(&roots, path).unwrap_or(root);
                (path.clone(), Shading::of(holding, path, patch))
            })
            .collect();

        Self {
            reads,
            repositories,
            patches,
            shades,
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

/// The root of the innermost of `roots` that `path` is in.
pub(super) fn holding<'a>(roots: &[&'a Path], path: &Path) -> Option<&'a Path> {
    roots
        .iter()
        .rev()
        .find(|root| path.starts_with(root))
        .copied()
}
