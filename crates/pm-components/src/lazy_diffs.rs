//! The diffs of changed files, which the server lists without them and sends for the files in view.

use std::collections::{HashMap, HashSet};

use crate::sample::Change;
use crate::state::{DIFF_WIDTH, Outgoing, Tab, WorkspaceState};

/// How far down from the top of the view, in logical pixels, the files whose diffs are wanted reach; the view and some of what is beyond it.
const REACH: f32 = 2400.0;

/// How many files one request asks the diffs of at most.
const BATCH: usize = 40;

impl WorkspaceState {
    /// Asks for the diffs of the files in or near the view of the diff, those the server listed without and those shown from before the folder was looked at again, each once while the answer is on its way.
    pub fn diffs_wanted(&mut self) {
        if self.tab != Tab::Diff {
            return;
        }
        let Some(index) = self.review_index() else {
            return;
        };
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        let from = self.diff_scroll - REACH / 4.0;
        let to = self.diff_scroll + REACH;
        let mut top = 0.0;
        let mut paths = Vec::new();
        for (at, change) in session.changes.iter().enumerate() {
            if top > to || paths.len() >= BATCH {
                break;
            }
            let height = crate::diff::section_height(self, session, at, DIFF_WIDTH);
            if top + height >= from
                && (!change.loaded || change.stale)
                && !self.asking_diffs.contains(&(index, change.path.clone()))
            {
                paths.push(change.path.clone());
            }
            top += height;
        }
        if paths.is_empty() {
            return;
        }
        for path in &paths {
            self.asking_diffs.insert((index, path.clone()));
        }
        self.outbox.push(Outgoing::Diffs {
            session: index,
            paths,
        });
    }

    /// The state after the server sent the diffs `files` of the session at `session`, in answer to the request for `asked`; a file it sent none of is shown with none.
    pub fn with_diffs(
        mut self,
        session: usize,
        asked: &[String],
        files: Vec<pm_git::FileDiff>,
    ) -> Self {
        for path in asked {
            self.asking_diffs.remove(&(session, path.clone()));
        }
        let wanted: HashSet<&str> = asked.iter().map(String::as_str).collect();
        let mut sent: HashMap<String, pm_git::FileDiff> = files
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();
        if let Some(held) = self.sessions.get_mut(session) {
            for change in held
                .changes
                .iter_mut()
                .filter(|change| wanted.contains(change.path.as_str()))
            {
                let file = sent.remove(&change.path);
                let read = Change::new(
                    &change.path,
                    file.as_ref().map_or("", |file| file.diff.as_str()),
                );
                change.diff = read.diff;
                change.words = file.map(|file| file.words).unwrap_or_default();
                change.loaded = true;
                change.stale = false;
            }
        }
        self
    }

    /// The state after the server said what the folder of the session at `session` is and where it stands, which it does before it has listed the files.
    pub fn with_standing(mut self, session: usize, standing: Standing) -> Self {
        if let Some(held) = self.sessions.get_mut(session) {
            held.unknown = false;
        }
        self.checked_out(
            session,
            standing.branch.as_deref(),
            standing.head.as_deref(),
        )
        .tracked(
            session,
            standing.branch.as_deref(),
            standing.tracking,
            standing.operation,
        )
        .with_standings(session, standing.repositories)
    }
}

/// Where a folder stands, as the server tells it.
pub struct Standing {
    /// The branch the folder has checked out, when it is on one.
    pub branch: Option<String>,
    /// The commit the folder is at, abbreviated, when it has one.
    pub head: Option<String>,
    /// What the checked out branch follows and how far apart they are.
    pub tracking: pm_git::Tracking,
    /// The merge, rebase or cherry-pick the folder is stopped in, if it is in one.
    pub operation: Option<pm_git::Operation>,
    /// Where each repository stands when the folder holds several side by side.
    pub repositories: Vec<pm_git::Standing>,
}
