//! The review side of the Git panel's link: notes left on lines and sent to
//! the agent, the diffs of files asked for as they come into view, the files
//! a commit or a branch changed, and the resolver of a conflicted file, all
//! as server requests so they work the same for remote projects.

use pm_editor::Document;
use pm_protocol::{Request, Response};
use pm_treesitter::{Syntax, registry};

use super::code::CodeAsk;
use super::git::{GitAsk, changed_file};
use super::{Ask, Link};
use crate::git::GitAction;
use crate::open_files::{SessionTab, SharedDocument};
use crate::review::{
    ConflictView, Revision, language_by_ending, notes_prompt, settle_all, settle_conflict,
};

impl Link {
    /// Does what the Diff tab asks beyond the panel's own work: notes,
    /// lazily loaded diffs, a commit's or a branch's files and conflicts.
    pub(super) fn git_review(&mut self, action: GitAction) {
        let folder = self.workspace.git.folder.clone();
        let git = &mut self.workspace.git;
        match action {
            GitAction::Note(note) => {
                if !note.text.trim().is_empty() {
                    git.notes.push(note);
                }
            }
            GitAction::RemoveNote(place) => {
                if place < git.notes.len() {
                    git.notes.remove(place);
                }
            }
            GitAction::SendNotes => {
                if git.notes.is_empty() {
                    return;
                }
                let prompt = notes_prompt(&git.notes);
                git.notes.clear();
                self.workspace.open_files.tab = SessionTab::Chat;
                self.send_prompt(prompt);
            }
            GitAction::ShowRevision { title, from, to } => {
                git.menu = None;
                git.revision = Some(Revision {
                    title,
                    from: from.clone(),
                    to: to.clone(),
                    ..Revision::default()
                });
                self.workspace.open_files.tab = SessionTab::Diff;
                if folder.is_empty() {
                    return;
                }
                self.ask(
                    Request::RevisionChanges {
                        directory: folder.clone(),
                        from,
                        to,
                    },
                    Ask::Git(GitAsk::Revision(folder)),
                );
            }
            GitAction::CloseRevision => git.revision = None,
            GitAction::RestoreRevision => {
                if let Some(restore) = git.revision.take().and_then(|revision| revision.restore) {
                    self.restore_revision(restore);
                }
            }
            GitAction::LoadDiffs(paths) => {
                let paths: Vec<String> = paths
                    .into_iter()
                    .filter(|path| !git.wanted.contains(path))
                    .filter(|path| {
                        git.changes
                            .iter()
                            .any(|change| &change.path == path && !change.fetched)
                    })
                    .collect();
                if paths.is_empty() || folder.is_empty() {
                    return;
                }
                git.wanted.extend(paths.iter().cloned());
                self.ask(
                    Request::Diffs {
                        directory: folder.clone(),
                        paths: paths.clone(),
                    },
                    Ask::Git(GitAsk::Diffs(folder, paths)),
                );
            }
            GitAction::TakeConflict(place, side) => {
                if let Some(conflict) = &mut git.conflict {
                    let text = conflict.result.0.borrow().field.text().to_owned();
                    let settled = settle_conflict(&text, place, side);
                    conflict.result = result_document(&settled, conflict.language.as_deref());
                }
            }
            GitAction::TakeAllConflicts(side) => {
                if let Some(conflict) = &mut git.conflict {
                    let text = conflict.result.0.borrow().field.text().to_owned();
                    let settled = settle_all(&text, side);
                    conflict.result = result_document(&settled, conflict.language.as_deref());
                }
            }
            GitAction::Resolve => {
                let Some(conflict) = &git.conflict else {
                    return;
                };
                let path = conflict.path.clone();
                let text = conflict.result.0.borrow().field.text().to_owned();
                git.busy = Some("Resolving".to_owned());
                self.ask(
                    Request::ResolveConflict {
                        directory: folder.clone(),
                        path,
                        text,
                    },
                    Ask::Git(GitAsk::Act { folder, then: None }),
                );
                self.workspace.git.conflict = None;
                self.workspace.git.diff = None;
            }
            _ => {}
        }
    }

    /// Readies the Diff tab for the file it now shows: reads the versions of
    /// a conflicted file, and asks for the grammar its diff is coloured with.
    pub(super) fn git_prepare_file(&mut self) {
        let git = &mut self.workspace.git;
        let Some(path) = git.diff.clone() else {
            return;
        };
        let conflicted = git
            .changes
            .iter()
            .any(|change| change.path == path && change.conflicted);
        let folder = git.folder.clone();
        if conflicted && git.conflict.as_ref().is_none_or(|open| open.path != path) {
            git.conflict = Some(ConflictView {
                path: path.clone(),
                ..ConflictView::default()
            });
            self.ask(
                Request::ReadConflict {
                    directory: folder.clone(),
                    path: path.clone(),
                },
                Ask::Git(GitAsk::Conflict(folder, path.clone())),
            );
        } else if !conflicted {
            git.conflict = None;
        }
        self.ask_grammar_of(&path);
    }

    /// Asks the server for the grammar of the language a file at `path` is written in, when it is not loaded.
    pub(super) fn ask_grammar_of(&mut self, path: &str) {
        if let Some(language) = language_by_ending(path)
            && registry::loaded(language).is_none()
            && self.client.is_some()
        {
            self.ask(
                Request::Grammar {
                    language: language.to_owned(),
                },
                Ask::Code(CodeAsk::Grammar(language.to_owned())),
            );
        }
    }

    /// Handles the `response` to a review request made for `ask`.
    pub(super) fn review_answer(&mut self, ask: GitAsk, response: Response) {
        let git = &mut self.workspace.git;
        match (ask, response) {
            (GitAsk::Revision(folder), _) if folder != git.folder => {}
            (GitAsk::Revision(_), Response::RevisionChanges { changes }) => {
                if let Some(revision) = &mut git.revision {
                    revision.changes = changes
                        .into_iter()
                        .map(|change| changed_file(change, &[]))
                        .collect();
                    revision.loaded = true;
                }
                let paths: Vec<String> = git
                    .revision
                    .iter()
                    .flat_map(|revision| revision.changes.iter().map(|change| change.path.clone()))
                    .collect();
                for path in paths {
                    self.ask_grammar_of(&path);
                }
            }
            (GitAsk::Revision(_), Response::Failed { message }) => {
                if let Some(revision) = &mut git.revision {
                    revision.loaded = true;
                    revision.trouble = Some(message);
                }
            }
            (GitAsk::Conflict(folder, path), response) => {
                let Some(conflict) = git
                    .conflict
                    .as_mut()
                    .filter(|open| open.path == path && folder == git.folder)
                else {
                    return;
                };
                match response {
                    Response::Conflict { files } => {
                        conflict.language = files.language.clone();
                        conflict.result = result_document(&files.merged, files.language.as_deref());
                        conflict.loaded = true;
                    }
                    Response::Failed { message } => {
                        conflict.loaded = true;
                        conflict.trouble = Some(message);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// A document holding `text`, the result of a conflicted file, highlighted
/// with the grammar of `language` when it is loaded.
fn result_document(text: &str, language: Option<&str>) -> SharedDocument {
    let mut document = Document::new(text, false, false);
    document.language = language.map(str::to_owned);
    if let Some(syntax) = language
        .and_then(registry::called)
        .and_then(|grammar| Syntax::parse(grammar, text))
    {
        document.with_syntax(syntax);
    }
    SharedDocument::new(document)
}
