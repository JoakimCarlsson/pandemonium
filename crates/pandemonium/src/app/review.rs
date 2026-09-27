//! What the window does about what a project has changed.
//!
//! Every git command the window carries out goes through the project's own
//! [`Review`]: the sidebar, the review pane, the palette and a keybinding all
//! ask the same model, which asks git and then reads the answer back. Nothing
//! here decides what a change is — that is git's — and nothing else in the
//! window writes to the index.

use std::collections::BTreeSet;
use std::path::PathBuf;

use pm_core::{FileStatus, Scope};
use pm_text::{Position, Request};

use crate::app::App;
use crate::desktop;
use crate::editor::FileId;
use crate::panes::Item;
use crate::prompt::{Answer, Prompt};
use crate::review::{ChangeId, Group, RepositoryAction, Review, Work};

impl App {
    /// The review of the project the window is pointed at.
    pub(super) fn review(&self) -> Option<&Review> {
        self.reviews.get(&self.scope()?)
    }

    /// Where the active repository of `scope`'s review sits: the one a branch
    /// is switched in, and a fetch, a pull or a push is made from.
    pub(super) fn repository_root(&self, scope: Scope) -> Option<PathBuf> {
        self.reviews
            .get(&scope)?
            .active_root()
            .map(std::path::Path::to_path_buf)
    }

    /// The worktree of `project` a branch or remote command is carried out in:
    /// the one the window is pointed at, when it is one of `project`'s.
    pub(super) fn git_scope(&self, project: pm_core::ProjectId) -> Scope {
        self.scope()
            .filter(|scope| scope.project() == project)
            .unwrap_or_else(|| Scope::checkout(project))
    }

    /// That review, to stage, throw away or commit through.
    pub(super) fn review_mut(&mut self) -> Option<&mut Review> {
        let scope = self.scope()?;
        self.reviews.get_mut(&scope)
    }

    /// Scrolls the Source Control list of changes by `delta` logical pixels
    /// when the pointer is over it, answering whether it was.
    pub(super) fn scroll_changes(&mut self, delta: f32) -> bool {
        let over = self.secondary_sidebar_open
            && self.secondary_sidebar_view == crate::workspace::SidebarView::Changes
            && self
                .pointer
                .is_some_and(|pointer| self.changes_area.get().contains(pointer));
        match self.review().filter(|_| over) {
            Some(review) => {
                review.scroll_list(delta);
                true
            }
            None => false,
        }
    }

    /// The worktrees a pane is holding the review, or one file's diff, of.
    fn reviewed_scopes(&self) -> BTreeSet<Scope> {
        self.panes
            .held()
            .into_iter()
            .filter_map(|item| match item {
                Item::Review(scope) | Item::Change(scope, _) => Some(scope),
                _ => None,
            })
            .collect()
    }

    /// The changed files of `scope`'s review that are on disk to be opened.
    fn reviewed_paths(&self, scope: Scope) -> Vec<PathBuf> {
        self.reviews.get(&scope).map_or_else(Vec::new, |review| {
            review
                .changed()
                .iter()
                .map(|changed| changed.path.clone())
                .filter(|path| path.is_file())
                .collect()
        })
    }

    /// The documents behind the files the reviews in the panes are showing.
    ///
    /// These stay open while a review is, though no tab names them, because
    /// they are what a language server colours a change's lines through.
    pub(super) fn reviewed_files(&self) -> BTreeSet<FileId> {
        self.open_files_of(self.reviewed_scopes())
    }

    /// The documents open over the changed files of the reviews of `scopes`.
    fn open_files_of(&self, scopes: impl IntoIterator<Item = Scope>) -> BTreeSet<FileId> {
        scopes
            .into_iter()
            .flat_map(|scope| {
                self.reviewed_paths(scope)
                    .into_iter()
                    .filter_map(move |path| self.editor.opened(scope, &path))
            })
            .collect()
    }

    /// Opens every file the reviews in the panes are showing, and asks their
    /// servers what the names in them are.
    ///
    /// A change is read in the colours its file has in an editor pane, and
    /// those come from the file's own document, so a review opens the
    /// documents of what it shows the way a tab would. This is asked for
    /// when the panes change and when a review's list of changes does,
    /// never while a frame is drawn.
    pub(super) fn open_reviewed_files(&mut self) {
        for scope in self.reviewed_scopes() {
            self.open_files_reviewed_in(scope);
        }
    }

    /// Opens the files `scope`'s review is showing, when a pane holds it.
    pub(super) fn open_reviewed_files_of(&mut self, scope: Scope) {
        if self.reviewed_scopes().contains(&scope) {
            self.open_files_reviewed_in(scope);
        }
    }

    /// Opens every changed file of `scope`'s review, and asks their servers
    /// what the names in them are.
    fn open_files_reviewed_in(&mut self, scope: Scope) {
        let Some(root) = self
            .reviews
            .get(&scope)
            .map(|review| review.root().to_path_buf())
        else {
            return;
        };
        for path in self.reviewed_paths(scope) {
            let Some(file) = self.editor.open(scope, &root, &path, true) else {
                continue;
            };
            let wanted = self
                .editor
                .get(file)
                .is_some_and(|document| document.borrow_mut().wants_semantics());
            if wanted {
                self.ask_about(file, Position::default(), Request::Semantics);
            }
        }
    }

    /// Colours every review's worktree lines again from the documents open
    /// over them.
    pub(super) fn repaint_reviews(&mut self) {
        for file in self.open_files_of(self.reviews.keys().copied()) {
            self.repaint_review(file);
        }
    }

    /// Colours the lines `file`'s worktree review shows of it again from its
    /// document, what a language server has said about it included.
    pub(super) fn repaint_review(&mut self, file: FileId) {
        let (Some(scope), Some(document)) = (self.editor.scope_of(file), self.editor.get(file))
        else {
            return;
        };
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.repaint_worktree(document.borrow_mut().buffer_mut());
        }
    }

    /// Brings the active project's review forward, opening it if it is closed.
    ///
    /// There is one review per project and it holds every change, so asking
    /// for it twice does not open it twice: a pane already showing it brings
    /// it to the front, and only a window with none opens one.
    pub(super) fn open_review(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let item = Item::Review(scope);
        let holder = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.items().any(|held| held == item))
        });

        match holder {
            Some(pane) => self.activate_tab(pane, item),
            None => self.show_item(self.panes.focus(), scope, item, false),
        }
    }

    /// Makes the `repository`-th repository of the review the active one,
    /// then carries out what its control asked for there.
    ///
    /// A menu asked for from a repository that was not already active opens
    /// where the pointer is: the one it would anchor to was measured beside
    /// the repository that was.
    pub(super) fn in_repository(&mut self, repository: usize, action: RepositoryAction) {
        let Some(review) = self.review_mut() else {
            return;
        };
        let moved = review.active() != repository;
        review.activate(repository);
        if moved && action == RepositoryAction::ShowCommitMenu {
            return self.open_menu(crate::app::MenuTarget::Commit);
        }
        if let Some(message) = action.message() {
            self.apply(message);
        }
    }

    /// Moves the review to the hunk before or after the one it is showing.
    pub(super) fn step_hunk(&mut self, forward: bool) {
        let Some(row) = self.review().and_then(|review| {
            crate::review::hunk_row(review, forward, self.preferences.split_diff)
        }) else {
            return;
        };
        if let Some(review) = self.review_mut() {
            review.scroll_to(None, row);
        }
    }

    /// Puts the list's selection on the `index`-th change.
    ///
    /// This is the one place a click on a row is read: marking with the
    /// secondary modifier and marking a range with shift. A plain click
    /// selects nothing and leaves the list unfocused; it forgets the marks and
    /// takes the reader to the file in the review, because that is what
    /// clicking a change is for.
    pub(super) fn select_change(&mut self, index: usize, marking: bool, ranging: bool) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(id) = self
            .reviews
            .get(&scope)
            .and_then(|review| review.id_of(index))
        else {
            return;
        };
        let Some(review) = self.reviews.get_mut(&scope) else {
            return;
        };

        match (ranging, marking) {
            (true, _) => review.mark_to(id),
            (_, true) => review.toggle_mark(id),
            _ => {
                review.clear_marks();
                return self.open_change(index);
            }
        }
        self.changes_focused = true;
    }

    /// Opens the diff of the row the list's keyboard is on.
    pub(super) fn open_selected_change(&mut self) {
        let Some(index) = self
            .review()
            .and_then(|review| review.place_of(review.selected()?))
        else {
            return;
        };
        self.open_change(index);
    }

    /// Stages what the list is acting on, or unstages it if it is all staged.
    ///
    /// One key does both, because what the reader means by it is plain from
    /// what they are looking at: a row that is in the index comes out of it,
    /// and everything else goes in.
    pub(super) fn toggle_selection_staged(&mut self) {
        let Some(review) = self.review() else {
            return;
        };
        let acting = review.acting_on();
        let staged = acting
            .iter()
            .filter_map(|id| review.change(review.place_of(*id)?))
            .all(pm_core::Changed::is_staged);

        match staged {
            true => self.change_selection(Review::unstage),
            false => self.change_selection(Review::stage),
        }
    }

    /// Moves the list's selection `steps` down it, marking on the way if asked.
    pub(super) fn step_changes(&mut self, steps: isize, marking: bool) {
        if let Some(review) = self.review_mut() {
            review.step(steps, marking);
        }
    }

    /// Forgets every mark the list is holding, saying whether it held any.
    pub(super) fn clear_change_marks(&mut self) -> bool {
        let Some(review) = self.review_mut() else {
            return false;
        };
        let held = review.has_marks();
        review.clear_marks();
        held
    }

    /// Points the list at the `index`-th change, for a menu opened on it.
    ///
    /// A row that is not part of what is marked takes the selection over, the
    /// way it does in a file manager: the menu is about what was pointed at
    /// unless what was pointed at is already part of a larger answer.
    pub(super) fn aim_at_change(&mut self, index: usize) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(id) = self
            .reviews
            .get(&scope)
            .and_then(|review| review.id_of(index))
        else {
            return;
        };
        if let Some(review) = self.reviews.get_mut(&scope) {
            match review.is_marked(id) {
                true => review.selected_is(id),
                false => review.select(id),
            }
            self.changes_focused = true;
        }
    }

    /// Puts the `index`-th change into the index, or takes it back out.
    ///
    /// A file that is not wholly in the index goes in — which is what the
    /// half-filled box means and what clicking it asks for — and a file that
    /// is already in comes back out.
    pub(super) fn toggle_change_staged(&mut self, index: usize) {
        let Some(changed) = self.review().and_then(|review| review.change(index)) else {
            return;
        };
        let staging = !(changed.is_staged() && !changed.is_unstaged());
        if self.modifiers.shift_key() {
            return self.stage_between(index, staging);
        }
        match staging {
            true => self.change_row(index, Review::stage),
            false => self.change_row(index, Review::unstage),
        }
    }

    /// Stages every file from the row the keyboard is on to the `index`-th.
    ///
    /// Shift on a box is a sweep rather than a click, the way it is in the
    /// list itself: what it passes over all goes the same way, decided by the
    /// box that was shift-clicked.
    fn stage_between(&mut self, index: usize, staging: bool) {
        let Some(ids) = self.review().map(|review| review.between(index)) else {
            return;
        };
        let work = self.review().and_then(|review| match staging {
            true => review.stage(&ids),
            false => review.unstage(&ids),
        });
        if let Some(id) = self.review().and_then(|review| review.id_of(index))
            && let Some(review) = self.review_mut()
        {
            review.selected_is(id);
        }
        self.work_here(work);
    }

    /// Puts one hunk into the index, or takes one back out of it.
    pub(super) fn toggle_hunk_staged(&mut self, index: usize, staged: bool, hunk: usize) {
        let Some(id) = self.review().and_then(|review| review.id_of(index)) else {
            return;
        };
        let work = self
            .review()
            .and_then(|review| review.stage_hunk(id, staged, hunk));
        self.work_here(work);
    }

    /// Applies a VS Code style inline action to an open conflict block.
    pub(super) fn conflict_action(
        &mut self,
        file: crate::editor::FileId,
        line: usize,
        action: crate::review::ConflictAction,
    ) {
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let (path, source) = {
            let document = document.borrow();
            (
                document.buffer().path().to_path_buf(),
                document.buffer().contents(),
            )
        };
        let Some(block) = crate::review::conflict::conflicts(&source)
            .into_iter()
            .find(|block| block.start_line == line)
        else {
            return;
        };
        match action {
            crate::review::ConflictAction::Accept(choice) => {
                let resolved = block.resolve(&source, choice);
                self.editor
                    .edit(file, |buffer| buffer.set_contents(&resolved));
            }
            crate::review::ConflictAction::Compare => {
                let Some(scope) = self.editor.scope_of(file) else {
                    return;
                };
                let Some(index) = self.reviews.get(&scope).and_then(|review| {
                    review
                        .changed()
                        .iter()
                        .position(|changed| changed.path == path)
                }) else {
                    return;
                };
                self.open_change_diff(index);
            }
        }
    }

    /// Puts the lines of one hunk back the way they were.
    pub(super) fn restore_hunk(&mut self, index: usize, staged: bool, hunk: usize) {
        let Some(id) = self.review().and_then(|review| review.id_of(index)) else {
            return;
        };
        let work = self
            .review()
            .and_then(|review| review.restore_hunk(id, staged, hunk));
        self.work_here(work);
    }

    /// Puts a whole group of the `repository`-th repository into the index,
    /// or takes the whole of it back out.
    pub(super) fn toggle_group_staged(&mut self, repository: usize, group: Group) {
        let Some(review) = self.review() else {
            return;
        };
        let (staged, count) = review.staged_in(repository, group);
        let ids = review
            .grouped(repository, group)
            .into_iter()
            .filter_map(|index| review.id_of(index))
            .collect::<Vec<_>>();
        let work = match staged < count {
            true => review.stage(&ids),
            false => review.unstage(&ids),
        };
        self.work_here(work);
    }

    /// Carries `change` out over the `index`-th change alone.
    pub(super) fn change_row(
        &mut self,
        index: usize,
        change: impl Fn(&Review, &[ChangeId]) -> Option<Work>,
    ) {
        let work = self
            .review()
            .and_then(|review| change(review, &[review.id_of(index)?]));
        self.work_here(work);
    }

    /// Carries `change` out over everything the list is acting on.
    pub(super) fn change_selection(
        &mut self,
        change: impl Fn(&Review, &[ChangeId]) -> Option<Work>,
    ) {
        let work = self
            .review()
            .and_then(|review| change(review, &review.acting_on()));
        self.work_here(work);
    }

    /// Has git carry `work` out in the worktree the window is pointed at.
    pub(super) fn work_here(&mut self, work: Option<Work>) {
        if let Some(scope) = self.scope() {
            self.work_later(scope, work);
        }
    }

    /// Brings the review forward and moves it to the `index`-th change.
    ///
    /// This is what clicking a row of the list does, and it is why there is
    /// one review rather than a tab per file: the reader goes on down the
    /// same pane, and the row they clicked is the top of it.
    pub(super) fn open_change(&mut self, index: usize) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(id) = self
            .reviews
            .get(&scope)
            .and_then(|review| review.id_of(index))
        else {
            return;
        };

        self.open_review();
        let split = self.preferences.split_diff;
        if let Some(review) = self.reviews.get_mut(&scope) {
            let row = crate::review::row_of(review, id, split).unwrap_or_default();
            review.scroll_to(None, row);
        }
    }

    /// Opens the `index`-th change's diff on its own, in the focused pane.
    ///
    /// The diff opens as a preview, the way a file clicked in the tree does:
    /// clicking down a list of changes leaves one tab behind rather than
    /// twenty, and the one you meant stays when you ask for it twice.
    pub(super) fn open_change_diff(&mut self, index: usize) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(change) = self
            .reviews
            .get(&scope)
            .and_then(|review| review.id_of(index))
        else {
            return;
        };
        self.show_item(self.panes.focus(), scope, Item::Change(scope, change), true);
    }

    /// Keeps the diff of `change` open, so the next one takes its own tab.
    pub(super) fn keep_change(&mut self, scope: Scope, change: ChangeId) {
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.keep(change);
        }
    }

    /// Whether the diff of `change` is only being looked at, not kept open.
    pub(super) fn is_change_preview(&self, scope: Scope, change: ChangeId) -> bool {
        self.reviews
            .get(&scope)
            .is_some_and(|review| review.is_preview(change))
    }

    /// Where the `index`-th change is, for the commands that name a file.
    pub(super) fn changed_path(&self, index: usize) -> Option<std::path::PathBuf> {
        let review = self.review()?;
        Some(review.change(index)?.path.clone())
    }

    /// Puts the `index`-th change's path on the clipboard.
    pub(super) fn copy_changed_path(&mut self, index: usize, relative: bool) {
        let Some(path) = self.changed_path(index) else {
            return;
        };
        let root = self.review().map(|review| review.root().to_path_buf());
        let written = match root.filter(|_| relative) {
            Some(root) => path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string(),
            None => path.display().to_string(),
        };
        desktop::copy(written);
    }

    /// Shows the `index`-th change's file in the desktop's file manager.
    pub(super) fn reveal_change(&mut self, index: usize) {
        if let Some(path) = self.changed_path(index) {
            desktop::reveal(&path);
        }
    }

    /// Opens the file the `index`-th change is to, where it first differs.
    ///
    /// The cursor lands on the first line the change touches rather than at
    /// the top of the file: a row of a list of changes is a place, and the
    /// place is where the file stopped being what it was.
    pub(super) fn open_change_file(&mut self, index: usize) {
        let Some((scope, root, path, line)) = self.changed_at(index) else {
            return;
        };
        let Some(file) = self.editor.open(scope, &root, &path, false) else {
            return;
        };
        self.show_file(self.panes.focus(), file, false);
        self.place_cursor(Position::new(line, 0), false);
    }

    /// Where the `index`-th change is, and the line it first differs at.
    fn changed_at(
        &self,
        index: usize,
    ) -> Option<(Scope, std::path::PathBuf, std::path::PathBuf, usize)> {
        let scope = self.scope()?;
        let review = self.reviews.get(&scope)?;
        let changed = review.change(index)?;
        let first = review
            .conflicts(&changed.path)
            .first()
            .map(|block| block.start_line + 1)
            .or_else(|| {
                review.patch(&changed.path).and_then(|patch| {
                    patch
                        .unstaged
                        .first()
                        .or_else(|| patch.staged.first())
                        .map(|hunk| hunk.start)
                })
            })
            .unwrap_or(1);
        Some((
            scope,
            review.root().to_path_buf(),
            changed.path.clone(),
            first.saturating_sub(1),
        ))
    }

    /// Asks whether the `index`-th change should be thrown away.
    ///
    /// Throwing a change away is the one thing here that cannot be undone by
    /// asking git again, so it is the one thing that is asked about first.
    pub(super) fn ask_to_discard(&mut self) {
        let Some(review) = self.review() else {
            return;
        };
        let acting = review.acting_on();
        let changes = acting
            .iter()
            .filter_map(|id| review.change(review.place_of(*id)?))
            .collect::<Vec<_>>();
        let Some(first) = changes.first() else {
            return;
        };

        let created = changes.iter().all(|changed| changed.is_created());
        let deleted = changes
            .iter()
            .all(|changed| changed.mark() == FileStatus::Deleted);
        let names = changes
            .iter()
            .map(|changed| changed.name())
            .collect::<Vec<_>>();
        let (asked, confirm) = match (changes.len(), created, deleted) {
            (1, true, _) => (
                format!("Are you sure you want to delete {}?", first.name()),
                "Delete File",
            ),
            (1, _, true) => (
                format!("Are you sure you want to restore {}?", first.name()),
                "Restore File",
            ),
            (1, ..) => (
                format!(
                    "Are you sure you want to discard changes to {}?",
                    first.name()
                ),
                "Discard Changes",
            ),
            (_, true, _) => ("Delete these files?".to_owned(), "Delete"),
            (..) => (
                "Discard changes to these files?".to_owned(),
                "Discard Changes",
            ),
        };

        self.discarding = acting;
        self.ask_first(Prompt::asking(
            asked,
            listed(&names),
            vec![
                Answer::new(confirm, crate::message::Message::ConfirmDiscard),
                Answer::cancel(),
            ],
        ));
    }

    /// Throws away the changes the reader was asked about.
    pub(super) fn discard_change(&mut self) {
        let discarding = std::mem::take(&mut self.discarding);
        if discarding.is_empty() {
            return;
        }
        let work = self.review().and_then(|review| review.discard(&discarding));
        self.work_here(work);
    }

    /// Takes the keyboard away from the commit message.
    pub(super) fn release_commit_focus(&mut self) -> bool {
        let focused = self.writing == Some(crate::app::Writing::Commit);
        if focused {
            self.writing = None;
        }
        focused
    }

    /// Scrolls the review under the pointer by `rows`, saying whether one was.
    ///
    /// The pointer answers before the keyboard does, the way it does over a
    /// pane of text: a wheel turned over a review scrolls the review it is
    /// over, whichever pane happens to have the keyboard. A pane showing one
    /// file's diff scrolls apart from the review it came from, because they
    /// are two panes and the reader is somewhere different in each.
    pub(super) fn scroll_review(&mut self, rows: isize) -> bool {
        let Some((scope, shown)) = self.review_under() else {
            return false;
        };
        let split = self.preferences.split_diff;
        let Some(review) = self.reviews.get_mut(&scope) else {
            return false;
        };
        let total = crate::review::row_count(review, shown, split);
        review.scroll_by(shown, rows, total);
        true
    }

    /// The review the pointer is over, and which of its files it is showing.
    fn review_under(&self) -> Option<(Scope, Option<ChangeId>)> {
        let scope = self.scope()?;
        let pane = self
            .pointer
            .and_then(|at| self.geometry.pane_at(at))
            .unwrap_or_else(|| self.panes.focus());
        let item = self.panes.pane(pane)?.active(scope)?;
        match item {
            Item::Review(scope) => Some((scope, None)),
            Item::Change(scope, change) => Some((scope, Some(change))),
            Item::File(_)
            | Item::Image(_)
            | Item::Rendered(_)
            | Item::Excerpts(_)
            | Item::Agent(..)
            | Item::Settings => None,
        }
    }
}

/// The names a question lists, with the tail of a long list left unsaid.
///
/// A question about forty files is not answered any better by naming all
/// forty: the first few say which files these are, and the count says how
/// many of them there are.
fn listed(names: &[String]) -> Vec<String> {
    const SHOWN: usize = 5;

    let mut lines = names.iter().take(SHOWN).cloned().collect::<Vec<_>>();
    match names.len().saturating_sub(SHOWN) {
        0 => {}
        more => lines.push(format!("and {more} more…")),
    }
    lines
}
