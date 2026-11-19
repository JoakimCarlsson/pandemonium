//! The diffs of changed files, which the server lists without them and sends for the files in view.

use std::collections::{HashMap, HashSet};

use crate::sample::{Change, Session};
use crate::state::{Outgoing, Tab, WorkspaceState};

/// How many files one request for the files in view asks the diffs of at most; a small one comes back quickly.
const BATCH_VIEW: usize = 12;

/// How many files one request for the files beyond the view asks the diffs of at most.
const BATCH_AHEAD: usize = 16;

/// How many files above the view count as near it, for when the diff is scrolled back up.
const BEHIND: usize = 4;

/// How many files may have their diffs asked and not yet answered before those beyond the view are left until some have been: scrolling fast must not queue up what it has left behind in front of what it has come to.
const AHEAD_IN_FLIGHT: usize = 24;

impl WorkspaceState {
    /// Asks for the diffs of the files in or near the view of the diff, those the server listed without and those shown from before the folder was looked at again, each once while the answer is on its way: the files in view first and on their own, so that they are read at once, then some of those beyond it while few are waiting.
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
        let (first, last) = self.files_in_view(session);
        let wanted = |at: &usize| {
            let change = &session.changes[*at];
            (!change.loaded || change.stale)
                && !self.asking_diffs.contains(&(index, change.path.clone()))
        };
        let path = |at: usize| session.changes[at].path.clone();
        let count = session.changes.len();
        let mut in_view: Vec<String> = (first..(last + 1).min(count))
            .filter(wanted)
            .map(path)
            .collect();
        let mut beyond: Vec<String> = (last + 1..count)
            .chain(first.saturating_sub(BEHIND)..first)
            .filter(wanted)
            .map(path)
            .collect();
        in_view.truncate(BATCH_VIEW);
        beyond.truncate(BATCH_AHEAD);
        if self.asking_diffs.len() + in_view.len() >= AHEAD_IN_FLIGHT {
            beyond.clear();
        }
        for paths in [in_view, beyond] {
            if paths.is_empty() {
                continue;
            }
            for path in &paths {
                self.asking_diffs.insert((index, path.clone()));
            }
            self.outbox.push(Outgoing::Diffs {
                session: index,
                paths,
            });
        }
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
        self.pin_diff();
        self
    }

    /// The first and the last of the files of `session` with a part in view of the diff's list as it was last laid out; before it was, the files from the top that a request for those in view asks for.
    fn files_in_view(&self, session: &Session) -> (usize, usize) {
        let info = self.diff_list.layout_info();
        let items = crate::diff::item_files(self, session);
        let file = |item: &rustle::LazyListItemInfo| items.get(item.index).copied();
        match (
            info.visible_items_info.first().and_then(file),
            info.visible_items_info.last().and_then(file),
        ) {
            (Some(first), Some(last)) => (first, last),
            _ => (0, BATCH_VIEW.saturating_sub(1)),
        }
    }

    /// Asks the diff's list to bring the header of the file it is anchored to to its top, now that the parts of the files above it may have changed; an anchor whose file is no longer in the diff being reviewed is let go. The list keeps the header there as the files above it are measured, until the reader scrolls.
    pub(crate) fn pin_diff(&mut self) {
        let Some((session, path)) = &self.diff_anchor else {
            return;
        };
        let item = self
            .sessions
            .get(*session)
            .filter(|_| self.review_index() == Some(*session))
            .and_then(|held| {
                held.changes
                    .iter()
                    .position(|change| &change.path == path)
                    .map(|index| crate::diff::header_item(self, held, index))
            });
        match item {
            Some(item) => self
                .diff_list
                .scroll_to_item(item, rustle::ItemAlign::Start),
            None => self.diff_anchor = None,
        }
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

#[cfg(test)]
mod tests;
