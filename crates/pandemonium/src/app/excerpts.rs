//! What the window does with a worktree's changes held as excerpts.
//!
//! The excerpts are built from the worktree's review — the same list of
//! changed files the sidebar shows — and read again whenever git is, so the
//! pane follows the review rather than keeping a list of its own. Every
//! keystroke in the pane goes to the document of the excerpt the cursor is
//! in, through the same seam any other pane's keystrokes go through; this is
//! only where the cursor is moved from one excerpt to the next.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use pm_core::Scope;
use pm_text::Position;
use pm_ui::ResizePhase;

use crate::app::App;
use crate::editor::FileId;
use crate::excerpts::{Excerpted, Excerpts};
use crate::panes::{Item, PaneId};

impl App {
    /// Brings the active worktree's excerpts forward, opening them if they
    /// are not open.
    ///
    /// There is one set of excerpts per worktree, as there is one review, so
    /// asking twice brings forward the pane that already holds them.
    pub(super) fn open_excerpts(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        self.point_at(scope);
        self.excerpts
            .entry(scope)
            .or_insert_with(|| Rc::new(RefCell::new(Excerpts::default())));
        self.share_comments(scope);
        self.refresh_excerpts_of(scope);

        let item = Item::Excerpts(scope);
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

    /// Has `scope`'s excerpts draw the comments its review holds, which are
    /// the same comments and not a copy of them.
    fn share_comments(&mut self, scope: Scope) {
        let (Some(review), Some(excerpts)) = (self.reviews.get(&scope), self.excerpts.get(&scope))
        else {
            return;
        };
        excerpts
            .borrow_mut()
            .set_comments(review.comments().clone());
    }

    /// Reads every held worktree's excerpts again from its review.
    pub(super) fn refresh_excerpts(&mut self) {
        let scopes = self.excerpts.keys().copied().collect::<Vec<_>>();
        for scope in scopes {
            self.refresh_excerpts_of(scope);
        }
    }

    /// Reads `scope`'s excerpts again from its review: which files changed,
    /// and, away from the window, what the last commit holds for each of them.
    ///
    /// A worktree whose excerpts no pane is holding is passed over: the
    /// excerpts are read again when a pane opens them.
    pub(super) fn refresh_excerpts_of(&mut self, scope: Scope) {
        if !self.excerpts.contains_key(&scope) {
            return;
        }
        self.share_comments(scope);
        self.follow_comments_in_documents(scope);
        let paths = self
            .reviews
            .get(&scope)
            .map(|review| {
                review
                    .changed()
                    .iter()
                    .map(|changed| changed.path.clone())
                    .filter(|path| path.is_file())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.reread_excerpts_later(scope, paths);
    }

    /// Puts `scope`'s excerpts together from the files `committed` names and
    /// what the last commit holds for each.
    ///
    /// The documents are the files' own, opened the way a review opens them,
    /// so an excerpt and a tab showing the same file are the same text.
    pub(super) fn set_excerpts(&mut self, scope: Scope, committed: Vec<(PathBuf, Option<String>)>) {
        let (Some(root), true) = (self.root_of(scope), self.excerpts.contains_key(&scope)) else {
            return;
        };
        let mut files = Vec::new();
        for (path, held) in committed {
            let Some(file) = self.editor.open(scope, &root, &path, true) else {
                continue;
            };
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            let name = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string();
            files.push(Excerpted::new(file, document, name, held));
        }
        if let Some(excerpts) = self.excerpts.get(&scope) {
            excerpts.borrow_mut().set_files(files);
        }
    }

    /// The documents behind every held worktree's excerpts, which stay open
    /// for as long as a pane holds the excerpts.
    pub(super) fn excerpted_files(&self) -> Vec<FileId> {
        self.excerpts
            .values()
            .flat_map(|excerpts| {
                excerpts
                    .borrow()
                    .files()
                    .iter()
                    .map(|excerpted| excerpted.file)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Puts the cursor of `pane`'s excerpts where a press landed in `file`,
    /// then treats the press as a press in that file's text.
    pub(super) fn select_excerpt(
        &mut self,
        pane: PaneId,
        phase: ResizePhase,
        file: FileId,
        anchor: Position,
        head: Position,
    ) {
        if let Some(Item::Excerpts(scope)) = self
            .panes
            .pane(pane)
            .and_then(|held| held.active(self.scope()))
            && let Some(excerpts) = self.excerpts.get(&scope)
        {
            excerpts.borrow_mut().activate(file);
        } else if let Some(Item::Search(scope)) = self
            .panes
            .pane(pane)
            .and_then(|held| held.active(self.scope()))
            && let Some(search) = self.searches.get(&scope)
        {
            search.excerpts.borrow_mut().activate(file);
            self.project_search_field = None;
        }
        self.select_text(pane, phase, anchor, head);
    }

    /// Opens the file in the `index`-th place of `pane`'s excerpts on its
    /// own, at the line of its first change.
    pub(super) fn open_excerpt_file(&mut self, pane: PaneId, index: usize) {
        let Some(Item::Excerpts(scope) | Item::Search(scope)) = self
            .panes
            .pane(pane)
            .and_then(|held| held.active(self.scope()))
        else {
            return;
        };
        let excerpts = match self
            .panes
            .pane(pane)
            .and_then(|held| held.active(self.scope()))
        {
            Some(Item::Search(_)) => self
                .searches
                .get(&scope)
                .map(|search| search.excerpts.clone()),
            _ => self.excerpts.get(&scope).cloned(),
        };
        let Some((file, line)) = excerpts.and_then(|excerpts| {
            let mut excerpts = excerpts.borrow_mut();
            let excerpted = excerpts.files_mut().get_mut(index)?;
            let line = excerpted.ranges().first().map_or(0, |range| range.start);
            Some((excerpted.file, line))
        }) else {
            return;
        };
        if let Some(from) = self.here() {
            self.trail.jumped(from);
        }
        self.editor.keep(file);
        self.show_file(pane, file, false);
        self.place_cursor(Position::new(line, 0), false);
    }

    /// Moves the cursor of the focused pane's excerpts on to the next
    /// excerpt when it has left the one it was in.
    ///
    /// Only the focused pane's are moved: the same document may be open in
    /// another pane, and a cursor moved there is not one to chase here.
    pub(super) fn settle_excerpts(&mut self) {
        let Some(Item::Excerpts(scope) | Item::Search(scope)) = self.active_tab() else {
            return;
        };
        let excerpts = match self.active_tab() {
            Some(Item::Search(_)) => self
                .searches
                .get(&scope)
                .map(|search| search.excerpts.clone()),
            _ => self.excerpts.get(&scope).cloned(),
        };
        let Some(excerpts) = excerpts else {
            return;
        };
        let Some((file, at)) = excerpts.borrow_mut().resettle() else {
            return;
        };
        if let Some(document) = self.editor.get(file) {
            document.borrow_mut().edit(|buffer| buffer.place(at, false));
        }
    }

    /// Scrolls the excerpts under the pointer by `rows`, saying whether the
    /// pointer was over any.
    pub(super) fn scroll_excerpts(&mut self, rows: isize) -> bool {
        let Some(Item::Excerpts(scope) | Item::Search(scope)) = self.item_under() else {
            return false;
        };
        let excerpts = match self.item_under() {
            Some(Item::Search(_)) => self.searches.get(&scope).map(|search| &search.excerpts),
            _ => self.excerpts.get(&scope),
        };
        let Some(excerpts) = excerpts else {
            return false;
        };
        excerpts.borrow_mut().scroll_by(rows);
        true
    }
}
