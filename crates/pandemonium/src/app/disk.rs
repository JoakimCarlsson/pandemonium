//! What the window does when a worktree changes under it.
//!
//! Every worktree the window holds is watched, the project's checkout and
//! each session's alike, because an agent writing into its worktree is the
//! ordinary case rather than the exception. What the disk did reaches the
//! three things drawn from it: the file tree, the open documents and the
//! language servers, and git is asked again what it makes of the worktree.

use pm_host::Location;

use std::collections::BTreeSet;
use std::path::PathBuf;

use pm_core::{Disk, Scope, Touch, Watcher};
use pm_text::Watched;

use crate::app::{App, Wake};

impl App {
    /// Watches every worktree the window holds, and stops watching the rest.
    pub(super) fn watch_worktrees(&mut self) {
        let worktrees = self.worktrees();
        self.watchers
            .retain(|scope, _| worktrees.iter().any(|(held, _)| held == scope));
        for (scope, root) in worktrees {
            if !self.watchers.contains_key(&scope) {
                let watcher = Watcher::start(&root, self.waker(Wake::Disk));
                self.watchers.insert(scope, watcher);
            }
        }
    }

    /// Takes in what the disk did under every watched worktree, answering
    /// whether anything did.
    pub(super) fn take_disk(&mut self) -> bool {
        let mut any = false;
        let mut repository = false;
        let mut session_changed = false;
        for (scope, root) in self.worktrees() {
            let Some(disk) = self.watchers.get(&scope).map(Watcher::take) else {
                continue;
            };
            if disk.is_empty() {
                continue;
            }
            any = true;
            repository |= disk.repository;
            session_changed |= scope.session().is_some()
                && (disk.repository || disk.touched.iter().any(|touched| !touched.ignored));
            self.follow_disk(scope, &root, &disk);
        }
        if session_changed {
            self.reread_drift_later();
        }
        if repository {
            self.reread_changes();
        }
        any
    }

    /// Brings the tree, the documents, the servers and the review of `scope`,
    /// whose worktree sits at `root`, up to what `disk` says happened.
    fn follow_disk(&mut self, scope: Scope, root: &Location, disk: &Disk) {
        if let Some(tree) = self.files.get_mut(&scope)
            && disk.touched.iter().any(|touched| {
                !touched.ignored
                    && touched.touch != Touch::Changed
                    && tree.lists_beside(&touched.path)
            })
        {
            tree.reload();
        }

        let written = disk
            .touched
            .iter()
            .filter(|touched| touched.touch != Touch::Removed)
            .map(|touched| touched.path.clone())
            .collect::<BTreeSet<_>>();
        self.editor.reread_paths(scope, root, &written);
        self.images.reread_paths(scope, &written);

        let followed = disk
            .touched
            .iter()
            .map(|touched| (touched.path.clone(), watched(touched.touch)))
            .collect::<Vec<(PathBuf, Watched)>>();
        if followed.is_empty() {
            return;
        }
        self.editor.watched(root, &followed);
        if disk.touched.iter().any(|touched| !touched.ignored) {
            self.reread_review_later(scope);
        }
    }
}

/// What a language server is told `touch` was.
fn watched(touch: Touch) -> Watched {
    match touch {
        Touch::Created => Watched::Created,
        Touch::Changed => Watched::Changed,
        Touch::Removed => Watched::Deleted,
    }
}
