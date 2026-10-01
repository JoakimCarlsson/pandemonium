//! What the window does about what a project has changed.
//!
//! Every git command the window carries out goes through the project's own
//! [`Review`]: the sidebar, the review pane, the palette and a keybinding all
//! ask the same model, which asks git and then reads the answer back. Nothing
//! here decides what a change is — that is git's — and nothing else in the
//! window writes to the index.

use pm_host::Location;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pm_core::{FileStatus, Scope};
use pm_text::{Position, Request};
use pm_ui::{ResizeEvent, ResizePhase};

use crate::agent::TalkId;
use crate::app::language::Purpose;
use crate::app::{App, Writing};
use crate::desktop;
use crate::editor::FileId;
use crate::panes::Item;
use crate::prompt::{Answer, Prompt};
use crate::review::comment::{Anchor, CommentId, Quote, Side};
use crate::review::{
    ChangeId, Delivery, Group, Remarking, RepositoryAction, Review, Work, hunk_anchor, line_at,
};

impl App {
    /// The review of the project the window is pointed at.
    pub(super) fn review(&self) -> Option<&Review> {
        self.reviews.get(&self.scope()?)
    }

    /// Where the active repository of `scope`'s review sits: the one a branch
    /// is switched in, and a fetch, a pull or a push is made from.
    pub(super) fn repository_root(&self, scope: Scope) -> Option<Location> {
        self.reviews.get(&scope)?.active_root().cloned()
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
        let Some(root) = self.reviews.get(&scope).map(|review| review.root().clone()) else {
            return;
        };
        for path in self.reviewed_paths(scope) {
            let Some(file) = self.editor.open(scope, &root, &path, true) else {
                continue;
            };
            if let Some(document) = self.editor.get(file) {
                let clients = document.borrow().servers();
                let served = document.borrow().buffer().path().to_path_buf();
                for client in clients {
                    if client.offers(&Request::Semantics, &served)
                        && document.borrow_mut().wants_semantics(&client)
                    {
                        self.ask_of(
                            client,
                            file,
                            Position::default(),
                            Request::Semantics,
                            Purpose::Act,
                        );
                    }
                }
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
        let root = self.review().map(|review| review.root().clone());
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
    fn changed_at(&self, index: usize) -> Option<(Scope, Location, std::path::PathBuf, usize)> {
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
            review.root().clone(),
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

    /// The text of the file at `path` in `scope` as a reader sees it: the
    /// open document when there is one, the disk otherwise.
    fn lines_of(&self, scope: Scope, path: &Path) -> Vec<String> {
        match self
            .editor
            .opened(scope, path)
            .and_then(|file| self.editor.get(file))
        {
            Some(document) => {
                let document = document.borrow();
                let buffer = document.buffer();
                (0..buffer.line_count())
                    .map(|line| buffer.line_text(line))
                    .collect()
            }
            None => self
                .root_of(scope)
                .and_then(|root| root.host.fs().read_to_string(path).ok())
                .ok_or(())
                .map(|text| text.lines().map(str::to_owned).collect())
                .unwrap_or_default(),
        }
    }

    /// The quote of lines `first..=last` of `path`, counted on `side`.
    ///
    /// New lines are read from the file; removed lines are read from the
    /// hunks that remove them, which are the only place they still are.
    fn quote_of(&self, scope: Scope, path: &Path, side: Side, first: usize, last: usize) -> Quote {
        match side {
            Side::New => {
                let lines = self.lines_of(scope, path);
                Quote::of(
                    |number| lines.get(number.checked_sub(1)?).cloned(),
                    first,
                    last,
                )
            }
            Side::Old => {
                let removed = self
                    .reviews
                    .get(&scope)
                    .and_then(|review| review.patch(path))
                    .into_iter()
                    .flat_map(|patch| patch.staged.iter().chain(&patch.unstaged))
                    .flat_map(|hunk| &hunk.lines)
                    .filter_map(|line| {
                        Some((line.old?, line.text.trim_end_matches('\n').to_owned()))
                    })
                    .collect::<std::collections::BTreeMap<_, _>>();
                Quote::of(|number| removed.get(&number).cloned(), first, last)
            }
        }
    }

    /// Starts a comment on lines `first..=last` of `path` in `scope`'s
    /// worktree, counted on `side`, and gives the keyboard to it.
    ///
    /// This is the one way a comment is begun, whichever pane the lines were
    /// pressed in. While a comment whose lines are gone is waiting for a
    /// line, the lines take that comment instead.
    pub(super) fn comment_on(
        &mut self,
        scope: Scope,
        path: &Path,
        side: Side,
        first: usize,
        last: usize,
    ) {
        let Some(review) = self.reviews.get(&scope) else {
            return;
        };
        if !path.starts_with(review.root()) || first == 0 || last < first {
            return;
        }
        let anchor = Anchor {
            path: review.relative(path),
            side,
            first,
            last,
        };
        let quote = self.quote_of(scope, path, side, first, last);
        let comments = review.comments().clone();
        match comments.moving() {
            Some(id) => {
                comments.relocate(id, anchor, quote);
                self.comments_changed(scope);
            }
            None => {
                comments.begin(anchor, quote);
                self.write_in(Writing::Comment(scope));
            }
        }
    }

    /// Starts a comment on the `first`-th to `last`-th line of the
    /// `index`-th change, counted on `side`.
    pub(super) fn comment_on_change(
        &mut self,
        index: usize,
        side: Side,
        first: usize,
        last: usize,
    ) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(path) = self
            .reviews
            .get(&scope)
            .and_then(|review| Some(review.change(index)?.path.clone()))
        else {
            return;
        };
        self.comment_on(scope, &path, side, first, last);
    }

    /// Starts a comment on the `hunk`-th hunk of the `index`-th change.
    pub(super) fn comment_on_hunk(&mut self, index: usize, staged: bool, hunk: usize) {
        let Some(review) = self.review() else {
            return;
        };
        let Some((side, first, last)) = review.change(index).and_then(|changed| {
            let patch = review.patch(&changed.path)?;
            let hunks = match staged {
                true => &patch.staged,
                false => &patch.unstaged,
            };
            hunk_anchor(review, index, staged, hunks.get(hunk)?)
        }) else {
            return;
        };
        self.comment_on_change(index, side, first, last);
    }

    /// Follows a gesture down the numbers of a change: the lines it sweeps
    /// over are marked as it goes, and let go of, they are commented on.
    pub(super) fn drag_comment(
        &mut self,
        shown: Option<ChangeId>,
        index: usize,
        side: Side,
        line: usize,
        event: ResizeEvent,
    ) {
        let Some(scope) = self.scope() else {
            return;
        };
        let theme = self.theme();
        let split = self.preferences.split_diff;
        let Some(review) = self.reviews.get(&scope) else {
            return;
        };
        let Some(path) = review.change(index).map(|changed| changed.path.clone()) else {
            return;
        };
        let reached = line_at(
            &theme,
            review,
            (shown, split),
            (index, side),
            event.current.y,
        )
        .unwrap_or(line);
        let (first, last) = (line.min(reached), line.max(reached));
        let swept = Anchor {
            path: review.relative(&path),
            side,
            first,
            last,
        };
        match event.phase {
            ResizePhase::Started | ResizePhase::Moved => review.comments().pick(Some(swept)),
            ResizePhase::Ended => {
                review.comments().pick(None);
                self.comment_on(scope, &path, side, first, last);
            }
        }
    }

    /// Starts a comment from a press on the gutter of `line` of `file`, in a
    /// pane of excerpts: on what is selected when the line is part of it,
    /// on the line alone otherwise.
    pub(super) fn comment_excerpt(&mut self, file: FileId, line: usize) {
        let Some(scope) = self.editor.scope_of(file) else {
            return;
        };
        let Some((path, first, last)) = self.editor.get(file).map(|document| {
            let document = document.borrow();
            let buffer = document.buffer();
            let (first, last) = selected_lines(buffer);
            let (first, last) =
                match (first..=last).contains(&line) && !buffer.selection().is_empty() {
                    true => (first, last),
                    false => (line, line),
                };
            (buffer.path().to_path_buf(), first + 1, last + 1)
        }) else {
            return;
        };
        self.comment_on(scope, &path, Side::New, first, last);
    }

    /// Starts a comment on what the focused pane has selected.
    ///
    /// Nothing selected is the line the cursor is on, the way it is when
    /// a selection is attached to a prompt.
    pub(super) fn add_comment(&mut self) {
        let Some(file) = self.active_file_id() else {
            return;
        };
        let Some(scope) = self.editor.scope_of(file) else {
            return;
        };
        let Some((path, first, last)) = self.editor.get(file).map(|document| {
            let document = document.borrow();
            let buffer = document.buffer();
            let (first, last) = selected_lines(buffer);
            (buffer.path().to_path_buf(), first + 1, last + 1)
        }) else {
            return;
        };
        self.comment_on(scope, &path, Side::New, first, last);
    }

    /// The comment box of the worktree the window is pointed at.
    fn comments_here(&self) -> Option<crate::review::comment::Comments> {
        Some(self.review()?.comments().clone())
    }

    /// Keeps the comment being written.
    pub(super) fn save_comment(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if let Some(comments) = self.comments_here() {
            comments.save();
        }
        self.leave_comment_box();
        self.comments_changed(scope);
    }

    /// Throws away the comment being written.
    pub(super) fn cancel_comment(&mut self) {
        if let Some(comments) = self.comments_here() {
            comments.cancel();
        }
        self.leave_comment_box();
    }

    /// Takes the keyboard away from the box a comment is written in.
    fn leave_comment_box(&mut self) {
        if matches!(self.writing, Some(Writing::Comment(_))) {
            self.writing = None;
        }
    }

    /// Lets go of the comment box on Escape: an empty one is cancelled, one
    /// with something in it only gives up the keyboard, keeping what was
    /// written. Answers whether the keyboard was in a comment box.
    pub(super) fn release_comment_focus(&mut self) -> bool {
        let Some(Writing::Comment(scope)) = self.writing else {
            return false;
        };
        let empty = self
            .reviews
            .get(&scope)
            .and_then(|review| review.comments().write(|input| input.is_empty()))
            .unwrap_or(true);
        if empty && let Some(review) = self.reviews.get(&scope) {
            review.comments().cancel();
        }
        self.writing = None;
        true
    }

    /// Rewrites the comment `id` names.
    pub(super) fn edit_comment(&mut self, id: CommentId) {
        let Some(scope) = self.scope() else {
            return;
        };
        if let Some(comments) = self.comments_here() {
            comments.edit(id);
            self.write_in(Writing::Comment(scope));
        }
    }

    /// Takes the comment `id` names away.
    pub(super) fn delete_comment(&mut self, id: CommentId) {
        let Some(scope) = self.scope() else {
            return;
        };
        if let Some(comments) = self.comments_here() {
            comments.delete(id);
        }
        self.comments_changed(scope);
    }

    /// Has the next line pressed take the comment `id` names, or stops
    /// waiting for one when it already was.
    pub(super) fn move_comment(&mut self, id: CommentId) {
        if let Some(comments) = self.comments_here() {
            let waiting = comments.moving() == Some(id);
            comments.arm_move((!waiting).then_some(id));
        }
    }

    /// Hides the comments already sent, or draws them again.
    pub(super) fn toggle_sent_comments(&mut self) {
        if let Some(comments) = self.comments_here() {
            comments.toggle_sent();
        }
    }

    /// Takes away every comment that has not been sent.
    pub(super) fn discard_review(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if let Some(comments) = self.comments_here() {
            comments.cancel();
            comments.discard();
        }
        self.leave_comment_box();
        self.comments_changed(scope);
    }

    /// The agent a review of `scope` is sent to.
    fn reviewer_of(&self, scope: Scope) -> Option<TalkId> {
        match scope.session() {
            Some(session) => self.agents.of_session(session),
            None => self.agent_in(scope),
        }
    }

    /// Whether the review of `scope` can be sent, and if not why not.
    fn delivery_of(&self, scope: Scope) -> Delivery {
        match self
            .reviewer_of(scope)
            .and_then(|talk| self.agents.get(talk))
        {
            None => Delivery::NoAgent,
            Some(talk) if talk.is_busy() => Delivery::Busy,
            Some(_) => Delivery::Ready,
        }
    }

    /// What the panes drawing `scope`'s review need to know of its comments.
    pub(super) fn remarking(&self, scope: Scope) -> Remarking {
        Remarking {
            focused: self.writing == Some(Writing::Comment(scope)),
            solid: self.caret_solid(),
            delivery: self.delivery_of(scope),
        }
    }

    /// Sends every pending comment to the session's agent as one prompt,
    /// through the conversation the reader's own prompts go through.
    ///
    /// The comments become sent only once the prompt has gone, so a review
    /// that could not be delivered is still all there to send.
    pub(super) fn send_review(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.delivery_of(scope) != Delivery::Ready {
            return;
        }
        let Some(talk) = self.reviewer_of(scope) else {
            return;
        };
        let Some(review) = self.reviews.get(&scope) else {
            return;
        };
        let comments = review.comments().clone();
        let Some((prompt, sent)) = comments.prompt(&review.changed_relative()) else {
            return;
        };
        if let Some(talk) = self.agents.get_mut(talk) {
            self.checks.reset(talk.scope());
            talk.send_text(&prompt);
        }
        comments.mark_sent(&sent);
        self.follow_agents();
        self.comments_changed(scope);
    }

    /// Follows the comments of `scope` to where their lines are in the
    /// documents that are open over them, which may hold edits the disk
    /// does not yet.
    pub(super) fn follow_comments_in_documents(&mut self, scope: Scope) {
        let Some(review) = self.reviews.get(&scope) else {
            return;
        };
        let comments = review.comments().clone();
        let root = review.root().clone();
        for path in comments.paths() {
            let Some(document) = self
                .editor
                .opened(scope, &root.join(&path))
                .and_then(|file| self.editor.get(file))
            else {
                continue;
            };
            let text = document.borrow().buffer().contents();
            comments.reanchor(&path, Some(&text));
        }
        self.comments_changed(scope);
    }

    /// Writes `scope`'s comments down, away from the window, if they have
    /// changed since they last were.
    pub(super) fn comments_changed(&mut self, scope: Scope) {
        self.remember_comments_later(scope);
        self.request_redraw();
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
            | Item::Outline(_)
            | Item::Excerpts(_)
            | Item::Search(_)
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

/// The first and last line, counted from zero, that the selection of
/// `buffer` covers.
///
/// A selection that ends at the start of a line does not cover that line;
/// nothing selected is the line the cursor is on.
fn selected_lines(buffer: &pm_text::Buffer) -> (usize, usize) {
    let selection = buffer.selection();
    let (start, end) = (selection.start(), selection.end());
    let last = match end.column == 0 && end.line > start.line {
        true => end.line - 1,
        false => end.line,
    };
    (start.line, last)
}
