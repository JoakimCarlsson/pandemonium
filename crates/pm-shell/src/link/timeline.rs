//! The timeline of the open file and the changes of a reader's turn: the
//! states the file was saved in and the commits that changed it, listed in
//! the quick input, each opened in the Diff tab against what the file holds
//! now with a button that puts it back; and a turn's changes shown the same
//! way with a button that undoes the turn. Everything is asked of the
//! server, so it works the same for remote projects.

use std::time::{SystemTime, UNIX_EPOCH};

use pm_protocol::{Request, Response};

use super::git::GitAsk;
use super::{Ask, Link, age};
use crate::git::{ChangedFile, tally};
use crate::open_files::SessionTab;
use crate::quick::{Choosing, QuickItem, QuickKind, QuickPick};
use crate::review::{Revision, RevisionRestore};

/// How many lines of context the timeline's diffs keep around each change.
const CONTEXT: usize = 3;

/// Where an earlier version of a file is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// A state the file was kept in when saved, by its number.
    Saved(u64),
    /// A commit that changed the file.
    Commit {
        /// The commit's full object name.
        object: String,
        /// The path the file had in the commit.
        path: String,
    },
}

/// One earlier version of the file the timeline is for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    /// When it was made, in milliseconds since the epoch.
    when: u64,
    /// How it is listed.
    title: String,
    /// What is listed beside it.
    detail: String,
    /// Where it is kept.
    source: Source,
}

/// The timeline of one file, as far as the server answered.
#[derive(Clone, Debug, Default)]
pub(super) struct Timeline {
    /// The folder the file is in, as the server knows it.
    directory: String,
    /// The path of the file, from the root of the folder.
    path: String,
    /// Its earlier versions, newest first.
    entries: Vec<Entry>,
    /// The version being read to be shown, by its place among `entries`.
    reading: Option<usize>,
}

impl Timeline {
    /// The rows the quick input lists for the timeline.
    fn choices(&self) -> Vec<QuickItem> {
        self.entries
            .iter()
            .enumerate()
            .map(|(place, entry)| QuickItem {
                title: entry.title.clone(),
                detail: entry.detail.clone(),
                kind: QuickKind::Choice,
                keys: None,
                matched: Vec::new(),
                pick: QuickPick::Choose(Choosing::History, place),
            })
            .collect()
    }

    /// Adds `entries` and keeps them newest first.
    fn add(&mut self, entries: impl IntoIterator<Item = Entry>) {
        self.entries.extend(entries);
        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.when));
    }
}

impl Link {
    /// Asks the server for the timeline of the file at `path` of `directory`: the states it was saved in and the commits that changed it.
    pub(super) fn ask_timeline(&mut self, directory: String, path: String) {
        self.timeline = Timeline {
            directory: directory.clone(),
            path: path.clone(),
            ..Timeline::default()
        };
        self.ask(
            Request::LocalHistory {
                directory: directory.clone(),
                path: path.clone(),
            },
            Ask::Listing(Choosing::History),
        );
        self.ask(
            Request::FileHistory { directory, path },
            Ask::Listing(Choosing::History),
        );
    }

    /// Takes in a `response` to a request made for the timeline, giving back the rows the quick input lists now, or `None` when it was not for the timeline.
    pub(super) fn answer_timeline(&mut self, response: &Response) -> Option<Vec<QuickItem>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        match response {
            Response::LocalHistory { versions } => {
                self.timeline.add(versions.iter().map(|version| Entry {
                    when: version.id,
                    title: format!("Saved {}", age(now.saturating_sub(version.id))),
                    detail: format!("{} bytes", version.size),
                    source: Source::Saved(version.id),
                }));
            }
            Response::FileHistory { commits } => {
                self.timeline.add(commits.iter().map(|commit| Entry {
                    when: u64::try_from(commit.time).unwrap_or_default() * 1000,
                    title: commit.summary.clone(),
                    detail: format!("{} · {} · {}", commit.id, commit.author, commit.when),
                    source: Source::Commit {
                        object: commit.object.clone(),
                        path: commit.path.clone(),
                    },
                }));
            }
            Response::File {
                text: Some(text), ..
            } => {
                let place = self.timeline.reading.take()?;
                self.show_version(place, text);
                return Some(self.timeline.choices());
            }
            _ => return None,
        }
        Some(self.timeline.choices())
    }

    /// Reads the version at `place` of the timeline, to show it against what the file holds now.
    pub(super) fn open_timeline(&mut self, place: usize) {
        let Some(entry) = self.timeline.entries.get(place) else {
            return;
        };
        let directory = self.timeline.directory.clone();
        let request = match &entry.source {
            Source::Saved(version) => Request::ReadVersion {
                directory,
                path: self.timeline.path.clone(),
                version: *version,
            },
            Source::Commit { object, path } => Request::ReadFile {
                directory,
                path: path.clone(),
                head: false,
                revision: Some(object.clone()),
            },
        };
        self.timeline.reading = Some(place);
        self.ask(request, Ask::Listing(Choosing::History));
    }

    /// Shows in the Diff tab how the version at `place` of the timeline, holding `text`, differs from what the file holds now.
    fn show_version(&mut self, place: usize, text: &str) {
        let Some(entry) = self.timeline.entries.get(place) else {
            return;
        };
        let (directory, path) = (self.timeline.directory.clone(), self.timeline.path.clone());
        let current = self
            .workspace
            .open_files
            .files
            .iter()
            .find(|file| file.directory == directory && file.path == path)
            .map(|file| file.document.0.borrow().field.text().to_owned())
            .unwrap_or_default();
        let diff = hunks_between(text, &current);
        let (added, removed) = tally(&diff);
        let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
        self.workspace.git.revision = Some(Revision {
            title: format!("Timeline: {name} · {}", entry.title),
            to: path.clone(),
            changes: vec![ChangedFile {
                path: path.clone(),
                status: 'M',
                added,
                removed,
                fetched: true,
                diff,
                ..ChangedFile::default()
            }],
            loaded: true,
            restore: Some(RevisionRestore::Version {
                directory,
                path: path.clone(),
                text: text.to_owned(),
            }),
            ..Revision::default()
        });
        self.workspace.open_files.tab = SessionTab::Diff;
        self.ask_grammar_of(&path);
    }

    /// Shows in the Diff tab what the reader's turn `turn` of the open session changed, with a button that undoes it.
    pub(super) fn show_turn(&mut self, turn: u64) {
        let Some(session) = self.watched else {
            return;
        };
        let folder = self.workspace.git.folder.clone();
        self.workspace.git.revision = Some(Revision {
            title: format!("Turn {}", turn + 1),
            restore: Some(RevisionRestore::Turn { session, turn }),
            ..Revision::default()
        });
        self.workspace.open_files.tab = SessionTab::Diff;
        self.ask(
            Request::TurnChanges { session, turn },
            Ask::Git(GitAsk::Revision(folder)),
        );
    }

    /// What a request that changes the session's files is asked for: the Git panel's notice tells when it fails, and the panel is refreshed once it is done.
    pub(super) fn files_changed_ask(&self) -> Ask {
        Ask::Git(GitAsk::Act {
            folder: self.workspace.git.folder.clone(),
            then: None,
        })
    }

    /// Puts back what `restore` offers: takes a turn's changes out of the session's files, or writes an earlier version of a file.
    pub(super) fn restore_revision(&mut self, restore: RevisionRestore) {
        let folder = self.workspace.git.folder.clone();
        match restore {
            RevisionRestore::Turn { session, turn } => {
                self.workspace.git.busy = Some("Undoing".to_owned());
                self.ask(
                    Request::UndoTurn {
                        session,
                        turn,
                        paths: Vec::new(),
                    },
                    Ask::Git(GitAsk::Act { folder, then: None }),
                );
            }
            RevisionRestore::Version {
                directory,
                path,
                text,
            } => {
                let open = self
                    .workspace
                    .open_files
                    .files
                    .iter()
                    .position(|file| file.directory == directory && file.path == path);
                match open {
                    Some(index) => {
                        let file = &mut self.workspace.open_files.files[index];
                        let language = file.document.0.borrow().language.clone();
                        file.take_text(&text, false, language);
                        file.document.0.borrow_mut().mark_unsaved();
                        self.workspace.open_files.tab = SessionTab::File(index);
                        self.save_file(index);
                    }
                    None => self.ask(
                        Request::WriteFile {
                            directory,
                            path,
                            text,
                        },
                        Ask::Git(GitAsk::Act { folder, then: None }),
                    ),
                }
            }
        }
    }
}

/// The hunks of the diff from `old` to `new`, from the first `@@` line on, as the Diff tab reads them.
fn hunks_between(old: &str, new: &str) -> String {
    let old = old.replace("\r\n", "\n");
    let new = new.replace("\r\n", "\n");
    similar::TextDiff::from_lines(&old, &new)
        .unified_diff()
        .context_radius(CONTEXT)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::hunks_between;

    /// The diff starts at its first hunk and marks lines removed and added.
    #[test]
    fn hunks_start_at_the_first_hunk() {
        let diff = hunks_between("a\nb\nc\n", "a\nB\nc\n");
        assert!(diff.starts_with("@@"));
        assert!(diff.contains("\n-b\n"));
        assert!(diff.contains("\n+B\n"));
    }

    /// The same text has no hunks.
    #[test]
    fn same_text_has_no_hunks() {
        assert!(hunks_between("a\n", "a\n").is_empty());
    }
}
