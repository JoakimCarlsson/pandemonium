//! What the window does to its panes: splitting, closing, and what is in them.
//!
//! The pane tree says how the window is divided and which pane has the
//! keyboard; the file store says what the documents behind the tabs are.
//! Both are the window's, so this is where a command that needs the two of
//! them together lives — and the one place a tab is opened or closed, whether
//! a keybinding, a tab menu or the file tree asked for it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use pm_core::{FileStatus, Scope};
use pm_gfx::Rect;
use pm_ui::{Axis, Element, IconName, MenuItem, ResizeEvent, ResizePhase, Theme};

use crate::app::App;
use crate::app::drag::{DropPlace, TabDrag, highlight, unmeasured};
use crate::editor::{Display, FileEntry, FileId, OpenFile};
use crate::keymap::Action;
use crate::message::Message;
use crate::panes::{
    self, Content, Contents, Item, PaneId, Saved, SavedKind, SavedTab, Shortcut, SplitDirection,
    TabEntry, Tool,
};
use crate::workspace::{MenuTarget, TabMenu};

/// The commands an empty pane offers, in the order it lists them.
const EMPTY_PANE_COMMANDS: [Action; 4] = [
    Action::ShowFiles,
    Action::ShowCommands,
    Action::NewAgentSession,
    Action::NewTerminal,
];

/// The most characters of a conversation's title a tab shows.
const TAB_TITLE: usize = 32;

impl App {
    /// The worktree the panes are showing: the active project's, or a session's.
    ///
    /// A tab belongs to the worktree its file was opened from and is drawn
    /// only while that worktree is the one the window is pointed at, the way
    /// a shell is. Every question about what a pane holds is asked of this,
    /// and it is the one answer to "where am I" the whole window reads.
    pub(super) fn scope(&self) -> Option<Scope> {
        let project = self.open.active()?.id();
        let session = self.session.filter(|session| {
            self.sessions
                .get(*session)
                .is_some_and(|session| session.project() == project)
        });
        Some(match session {
            Some(session) => Scope::of(project, session),
            None => Scope::checkout(project),
        })
    }

    /// Where the worktree `scope` names sits on disk.
    pub(super) fn root_of(&self, scope: Scope) -> Option<PathBuf> {
        match scope.session() {
            Some(session) => self
                .sessions
                .get(session)
                .map(|session| session.root().to_path_buf()),
            None => self
                .open
                .get(scope.project())
                .map(|project| project.root().to_path_buf()),
        }
    }

    /// Every worktree the window is holding, with where it sits on disk.
    pub(super) fn worktrees(&self) -> Vec<(Scope, PathBuf)> {
        self.scopes()
            .into_iter()
            .filter_map(|scope| Some((scope, self.root_of(scope)?)))
            .collect()
    }

    /// Every worktree the window is holding: each checkout, and each session.
    pub(super) fn scopes(&self) -> Vec<Scope> {
        self.open
            .iter()
            .flat_map(|project| {
                let checkout = Scope::checkout(project.id());
                let sessions = self
                    .sessions
                    .of(project.id())
                    .map(|session| Scope::of(project.id(), session.id()))
                    .collect::<Vec<_>>();
                std::iter::once(checkout).chain(sessions)
            })
            .collect()
    }

    /// The file the pane with the keyboard is showing, when it shows one.
    pub(super) fn active_file(&self) -> Option<OpenFile> {
        self.editor.get(self.active_file_id()?)
    }

    /// What the pane with the keyboard is showing.
    pub(super) fn active_tab(&self) -> Option<Item> {
        self.panes.focused()?.active(self.scope())
    }

    /// The file it is showing, for the commands that are a file's.
    ///
    /// A pane of excerpts is showing many files and editing one of them —
    /// the one its cursor is in — so that is the file a command means there.
    pub(super) fn active_file_id(&self) -> Option<FileId> {
        self.file_in(self.active_tab()?)
    }

    /// The file keystrokes go to in a pane showing `item`, if they go to one.
    pub(super) fn file_in(&self, item: Item) -> Option<FileId> {
        match item {
            Item::Excerpts(scope) => self.excerpts.get(&scope)?.borrow().active(),
            Item::Search(scope) => self.searches.get(&scope)?.excerpts.borrow().active(),
            item => item.file(),
        }
    }

    /// Gives the keyboard to `pane`, taking it from the terminal.
    pub(super) fn focus_pane(&mut self, pane: PaneId) {
        if pane != self.panes.focus() {
            if let Some(ui) = self.ui.as_mut() {
                ui.clear_text_selection();
            }
            self.agents.clear_selections();
            self.dismiss_prediction();
        }
        self.panes.set_focus(pane);
        self.follow_focused_file();
        self.editor_focused = true;
        self.terminal_focused = false;
        self.changes_focused = false;
        self.tree_focused = false;
        match self.active_tab() {
            Some(Item::Tool(Tool::Files)) => {
                self.editor_focused = false;
                self.tree_focused = true;
            }
            Some(Item::Tool(Tool::Changes)) => {
                self.editor_focused = false;
                self.changes_focused = true;
            }
            Some(Item::Tool(Tool::Terminal)) => {
                self.editor_focused = false;
                self.terminal_focused = true;
            }
            Some(Item::Tool(_)) => {}
            _ => self.content_pane = Some(pane),
        }
    }

    /// Remembers the active file after focus or a tab changes.
    pub(super) fn follow_focused_file(&mut self) {
        if let (Some(scope), Some(file)) = (self.scope(), self.active_file_id()) {
            self.outlines.follow(scope, file, self.panes.focus());
        }
    }

    /// Opens `file` in `pane`, as a preview or to stay.
    ///
    /// A preview takes the pane's one preview tab from whatever held it,
    /// which is what makes clicking down a tree leave one tab behind rather
    /// than twenty — and it is the pane's tab, not the window's, so a file
    /// previewed on the right leaves the pane on the left as it was.
    pub(super) fn show_file(&mut self, pane: PaneId, file: FileId, preview: bool) {
        let Some(scope) = self.editor.scope_of(file) else {
            return;
        };
        self.show_item(pane, scope, Item::File(file), preview);
    }

    /// Shows `item` of `scope` in `pane`, opening a tab for it if need be.
    pub(super) fn show_item(&mut self, pane: PaneId, scope: Scope, item: Item, preview: bool) {
        let pane = if self
            .panes
            .pane(pane)
            .and_then(|pane| pane.active(self.scope()))
            .is_some_and(|active| {
                matches!(active, Item::Tool(_))
                    && !(active == Item::Tool(Tool::Chat) && matches!(item, Item::Agent(_, _)))
            }) {
            self.document_pane()
        } else {
            pane
        };
        if let Some(ui) = self.ui.as_mut() {
            ui.clear_text_selection();
        }
        self.agents.clear_selections();
        if preview {
            self.close_previews(pane, item);
        }
        self.point_at(scope);
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.open(scope, item);
        }
        self.focus_pane(pane);
        self.sweep();
        self.store();
    }

    /// Closes whatever `pane` is previewing, other than `keep`.
    fn close_previews(&mut self, pane: PaneId, keep: Item) {
        let previews = self
            .panes
            .pane(pane)
            .map(|pane| {
                pane.items()
                    .filter(|item| *item != keep && self.is_preview(*item))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(pane) = self.panes.pane_mut(pane) {
            for item in previews {
                pane.close(item);
            }
        }
    }

    /// Whether `item` is only being looked at, not kept open.
    fn is_preview(&self, item: Item) -> bool {
        match item {
            Item::File(file) => self.editor.is_preview(file),
            Item::Image(image) => self.images.is_preview(image),
            Item::Change(scope, change) => self.is_change_preview(scope, change),
            Item::Rendered(_)
            | Item::Outline(_)
            | Item::Review(_)
            | Item::Excerpts(_)
            | Item::Search(_)
            | Item::Agent(..)
            | Item::Tool(_) => false,
        }
    }

    /// Whether `item` holds changes that are not on disk.
    fn is_dirty(&self, item: Item) -> bool {
        match item {
            Item::Excerpts(scope) => self.excerpts_dirty(scope),
            Item::Search(scope) => self.searches.get(&scope).is_some_and(|search| {
                search.held.iter().any(|file| self.editor.is_dirty(*file))
                    || search
                        .excerpts
                        .borrow()
                        .files()
                        .iter()
                        .any(|file| self.editor.is_dirty(file.file))
            }),
            item => item.file().is_some_and(|file| self.editor.is_dirty(file)),
        }
    }

    /// Whether any file `scope`'s excerpts show has changes not on disk.
    fn excerpts_dirty(&self, scope: Scope) -> bool {
        self.excerpts.get(&scope).is_some_and(|excerpts| {
            excerpts
                .borrow()
                .files()
                .iter()
                .any(|excerpted| self.editor.is_dirty(excerpted.file))
        })
    }

    /// Divides `pane` that way, showing `file` in the pane that opens.
    ///
    /// Without a file of its own the new pane shows what the old one was
    /// showing, which is what splitting a pane is for: the same file, twice,
    /// at two places in it. A tool moves to the new pane instead of opening
    /// a second instance. A pane showing nothing is not split at all: the
    /// pane it would open could show nothing either, and two empty panes
    /// divide the window for nothing.
    pub(super) fn split_pane(
        &mut self,
        pane: PaneId,
        item: Option<Item>,
        direction: SplitDirection,
    ) {
        let Some(item) = item.or_else(|| self.panes.pane(pane)?.active(self.scope())) else {
            return;
        };
        let scope = self.scope_of(item).or_else(|| self.scope());
        let Some(fresh) = self.panes.split(pane, direction) else {
            return;
        };
        if let Some(fresh) = self.panes.pane_mut(fresh) {
            fresh.open(scope, item);
        }
        if matches!(item, Item::Tool(_)) {
            if let Some(source) = self.panes.pane_mut(pane) {
                source.close(item);
            }
            self.panes.close_if_empty(pane);
        }
        self.focus_pane(fresh);
        self.store();
    }

    /// Closes `pane`, leaving the window as it was when it has only the one.
    pub(super) fn close_pane(&mut self, pane: PaneId) {
        if let Some(search) = self
            .tabs_of(pane)
            .into_iter()
            .find(|item| matches!(item, Item::Search(_)) && self.is_dirty(*item))
        {
            self.close_item(pane, search);
            return;
        }
        if self.panes.close(pane) {
            self.focus_pane(self.panes.focus());
            self.sweep();
            self.store();
        }
    }

    /// Closes the tab in front of the pane with the keyboard.
    pub(super) fn close_active_tab(&mut self) {
        let pane = self.panes.focus();
        if let Some(item) = self.active_tab() {
            self.close_item(pane, item);
        }
    }

    /// The worktree `item` belongs to, which is none for the window's own.
    pub(super) fn scope_of(&self, item: Item) -> Option<Scope> {
        match item {
            Item::File(file) | Item::Rendered(file) => self.editor.scope_of(file),
            Item::Image(image) => self.images.scope_of(image),
            Item::Review(scope)
            | Item::Outline(scope)
            | Item::Change(scope, _)
            | Item::Excerpts(scope)
            | Item::Search(scope)
            | Item::Agent(scope, _) => Some(scope),
            Item::Tool(_) => None,
        }
    }

    /// Closes one tab, asking first when what is in it is not on disk.
    ///
    /// A file with changes nobody has written down is not something to close
    /// quietly: the reader is asked which of the two things they meant, and
    /// the file stays open until they say.
    pub(super) fn close_item(&mut self, pane: PaneId, item: Item) {
        if let Some(file) = item.file().filter(|file| self.editor.is_dirty(*file)) {
            return self.open_menu(crate::workspace::MenuTarget::Unsaved(pane, file));
        }
        if let Item::Search(scope) = item
            && let Some(search) = self.searches.get(&scope)
        {
            let dirty = search
                .held
                .iter()
                .copied()
                .chain(
                    search
                        .excerpts
                        .borrow()
                        .files()
                        .iter()
                        .map(|file| file.file),
                )
                .filter(|file| self.editor.is_dirty(*file))
                .collect::<BTreeSet<_>>();
            for file in dirty {
                self.editor.keep(file);
                if let Some(open) = self.panes.pane_mut(pane) {
                    open.open(scope, Item::File(file));
                }
            }
        }
        self.close_tabs(pane, |pane| pane.close(item));
    }

    /// Writes the file down and then closes its tab.
    pub(super) fn save_and_close(&mut self, pane: PaneId, file: FileId) {
        if let Some(root) = self.worktree_of(file) {
            self.editor.save(file, &root);
        }
        self.close_tabs(pane, |pane| pane.close(Item::File(file)));
        self.go_on_closing();
    }

    /// Closes the tab, losing whatever was not written down.
    pub(super) fn discard_and_close(&mut self, pane: PaneId, file: FileId) {
        self.close_tabs(pane, |pane| pane.close(Item::File(file)));
        self.go_on_closing();
    }

    /// Closes every tab of `pane`, asking about each one that is not on disk.
    ///
    /// The unsaved ones are asked about one at a time: the answer closes that
    /// file and brings up the next, so closing a bar of twenty tabs with
    /// three unsaved among them is three questions and not twenty.
    pub(super) fn close_every_tab(&mut self, pane: PaneId) {
        self.closing = Some(pane);
        self.close_saved_tabs(pane, |_| false);
        self.go_on_closing();
    }

    /// Asks about the next unsaved tab of the pane being closed, if there is one.
    fn go_on_closing(&mut self) {
        let Some(pane) = self.closing else {
            return;
        };
        let next = self
            .tabs_of(pane)
            .into_iter()
            .filter_map(Item::file)
            .find(|file| self.editor.is_dirty(*file));
        match next {
            Some(file) => self.open_menu(crate::workspace::MenuTarget::Unsaved(pane, file)),
            None => self.closing = None,
        }
    }

    /// Closes every tab of `pane` that `keep` does not name and is on disk.
    ///
    /// Closing a bar of tabs at once leaves behind the ones with changes
    /// nobody has written down: there is no one file to ask about, and
    /// quietly losing the lot is the one answer that cannot be taken back.
    pub(super) fn close_saved_tabs(&mut self, pane: PaneId, keep: impl Fn(Item) -> bool) {
        let closing = self
            .tabs_of(pane)
            .into_iter()
            .filter(|item| !keep(*item) && !self.is_dirty(*item))
            .collect::<Vec<_>>();
        self.close_tabs(pane, |pane| pane.retain(|item| !closing.contains(&item)));
    }

    /// The tabs of `pane` on one side of `item`, and `item` itself.
    pub(super) fn tabs_from(&self, pane: PaneId, item: Item, right: bool) -> Vec<Item> {
        let tabs = self.tabs_of(pane);
        let Some(index) = tabs.iter().position(|held| *held == item) else {
            return tabs;
        };
        match right {
            true => tabs[..=index].to_vec(),
            false => tabs[index..].to_vec(),
        }
    }

    /// Moves the keyboard to the pane `forward` of the focused one on `axis`.
    pub(super) fn focus_neighbour(&mut self, axis: Axis, forward: bool) {
        if let Some(neighbour) = self.panes.neighbour(self.panes.focus(), axis, forward) {
            self.focus_pane(neighbour);
        }
    }

    /// Applies `edit` to the tab in front of the pane with the keyboard.
    pub(super) fn edit_active(&mut self, edit: impl FnOnce(&mut pm_text::Buffer)) {
        if let Some(typed) = self.typed_into() {
            typed.borrow_mut().edit(edit);
            return;
        }
        if let Some(file) = self.active_file_id() {
            self.editor.edit(file, edit);
        }
    }

    /// The single-line field that has the keyboard, if one does.
    ///
    /// The picker first, then a name being typed into the tree. The search
    /// bar lives in the open document, so it is reached through
    /// [`Self::edit_focused_field`].
    pub(super) fn focused_field_mut(&mut self) -> Option<&mut crate::field::Field> {
        if let Some(picker) = self.picker.as_mut() {
            return Some(picker.field_mut());
        }
        self.tree_edit.as_mut().map(|edit| edit.field_mut())
    }

    /// Puts `edit` through the single-line field that has the keyboard, if
    /// one does.
    ///
    /// The picker first, then a name being typed into the tree, then the
    /// search bar's query or replacement. This is the one place that says
    /// which field has the keyboard.
    pub(super) fn edit_focused_field(
        &mut self,
        edit: impl FnOnce(&mut crate::field::Field),
    ) -> bool {
        if let Some(field) = self.focused_field_mut() {
            edit(field);
            return true;
        }
        if let Some(Item::Search(scope)) = self.active_tab()
            && let Some(which) = self.project_search_field
            && let Some(search) = self.searches.get_mut(&scope)
        {
            let field = match which {
                crate::editor::SearchField::Query => &mut search.query,
                crate::editor::SearchField::Replacement => &mut search.replacement,
            };
            let before = field.value().to_owned();
            edit(field);
            if which == crate::editor::SearchField::Query && before != field.value() {
                self.run_project_search(scope);
            }
            return true;
        }
        if self.search_focused {
            let Some(file) = self.active_file() else {
                return false;
            };
            file.borrow_mut()
                .search_with(|search, buffer| search.edit_field(edit, buffer));
            return true;
        }
        false
    }

    /// The buffer being typed into that is not a pane's file, if there is one.
    ///
    /// A pane's file is not the only thing a reader writes in: an agent's
    /// prompt is a buffer too, and a command that edits text means whichever
    /// of them has the keyboard, not the file behind it.
    pub(super) fn typed_into(&self) -> Option<crate::editor::OpenFile> {
        match self.writing? {
            crate::app::Writing::Commit => Some(self.review()?.message()?.text()),
            crate::app::Writing::Prompt(session) => Some(self.agents.get(session)?.prompt().text()),
            crate::app::Writing::Console(scope) => {
                Some(self.debuggers.get(scope)?.console().text())
            }
            crate::app::Writing::McpSearch => Some(self.mcp_search.text()),
            crate::app::Writing::AgentSearch => Some(self.agent_search.text()),
            crate::app::Writing::LanguageServerField(index) => {
                Some(self.languages.editor.as_ref()?.fields.get(index)?.text())
            }
            crate::app::Writing::FormField(field) => {
                Some(self.server_form.as_ref()?.input(field)?.text())
            }
            crate::app::Writing::Answer(session, ticket, place) => Some(
                self.agents
                    .get(session)?
                    .forms()
                    .iter()
                    .find(|form| form.id() == ticket)?
                    .text_box(place)?
                    .text(),
            ),
            crate::app::Writing::Comment(scope) => {
                Some(self.reviews.get(&scope)?.comments().composing()?.text)
            }
        }
    }

    /// Changes the tabs of `pane` and closes whatever that left with nothing.
    pub(super) fn close_tabs(&mut self, pane: PaneId, close: impl FnOnce(&mut panes::Pane)) {
        let before = self.panes.held();
        if let Some(pane) = self.panes.pane_mut(pane) {
            close(pane);
        }
        self.panes.close_if_empty(pane);
        self.remember_closed(&before);
        self.sweep();
        self.focus_pane(self.panes.focus());
        self.store();
    }

    /// Takes down every file that was held before and is not held now.
    ///
    /// A tab is worth reopening whichever way it was closed — one tab, the
    /// others, everything to the right — so what closed it is not asked; the
    /// difference between what was open and what is open says it.
    fn remember_closed(&mut self, before: &BTreeSet<Item>) {
        let held = self.panes.held();
        let gone = before
            .iter()
            .filter(|item| !held.contains(item))
            .filter_map(|item| {
                let file = item.file()?;
                let document = self.editor.get(file)?;
                let document = document.borrow();
                Some(crate::app::places::Place {
                    scope: self.editor.scope_of(file)?,
                    path: document.buffer().path().to_path_buf(),
                    position: document.buffer().selection().head,
                })
            })
            .collect::<Vec<_>>();
        for place in gone {
            self.trail.closed(place);
        }
    }

    /// The panes as they stand, in the shape a launch restores them from.
    ///
    /// A pane names its files by where they live rather than by the id this
    /// run gave them, which is the only thing the next launch can act on.
    pub(super) fn saved_panes(&self) -> Saved {
        self.panes.save(self.scope(), &|item| {
            if let Item::Tool(tool) = item {
                return Some(SavedTab {
                    kind: SavedKind::Tool,
                    tool: Some(tool),
                    ..SavedTab::default()
                });
            }
            let scope = self.scope_of(item)?;
            let project = self.open.get(scope.project())?.root().to_path_buf();
            let worktree = self.root_of(scope)?;
            if let Some(talk) = item.session().and_then(|talk| self.agents.get(talk)) {
                return Some(SavedTab {
                    kind: SavedKind::Agent,
                    project,
                    worktree,
                    agent: talk.agent().id.to_owned(),
                    session: talk.resumable().unwrap_or_default(),
                    title: talk.title().unwrap_or_default().to_owned(),
                    ..SavedTab::default()
                });
            }
            if let Some(change) = item.change() {
                let path = self.reviews.get(&scope)?.path_of(change)?;
                return Some(SavedTab {
                    kind: SavedKind::Change,
                    project,
                    worktree,
                    path: path.to_path_buf(),
                    ..SavedTab::default()
                });
            }
            if let Some(image) = item.image() {
                return Some(SavedTab {
                    kind: SavedKind::Image,
                    project,
                    worktree,
                    path: self.images.path_of(image)?.to_path_buf(),
                    preview: self.images.is_preview(image),
                    ..SavedTab::default()
                });
            }
            if let Some(file) = item.rendered() {
                return Some(SavedTab {
                    kind: SavedKind::Rendered,
                    project,
                    worktree,
                    path: self.editor.path(file)?,
                    ..SavedTab::default()
                });
            }
            if item.excerpts().is_some() {
                return Some(SavedTab {
                    kind: SavedKind::Excerpts,
                    project,
                    worktree,
                    ..SavedTab::default()
                });
            }
            if matches!(item, Item::Outline(_)) {
                return Some(SavedTab {
                    kind: SavedKind::Outline,
                    project,
                    worktree,
                    path: self
                        .outlines
                        .followed(scope)
                        .and_then(|file| self.editor.path(file))
                        .unwrap_or_default(),
                    ..SavedTab::default()
                });
            }
            if matches!(item, Item::Search(_)) {
                return Some(SavedTab {
                    kind: SavedKind::Search,
                    project,
                    worktree,
                    ..SavedTab::default()
                });
            }
            let Some(file) = item.file() else {
                return Some(SavedTab {
                    kind: SavedKind::Review,
                    project,
                    worktree,
                    ..SavedTab::default()
                });
            };
            let document = self.editor.get(file)?;
            let document = document.borrow();
            let head = document.buffer().selection().head;
            Some(SavedTab {
                kind: SavedKind::File,
                project,
                worktree,
                path: document.buffer().path().to_path_buf(),
                preview: document.is_preview(),
                scroll: document.scroll(),
                line: head.line,
                column: head.column,
                ..SavedTab::default()
            })
        })
    }

    /// Opens again everything the last launch had open, where it had it.
    ///
    /// A file whose project is no longer open, or which is no longer on
    /// disk, is left behind: the window comes back as much like itself as
    /// what is still there allows.
    pub(super) fn restore_panes(&mut self, saved: &Saved) {
        let projects = self
            .open
            .iter()
            .map(|project| (project.root().to_path_buf(), project.id()))
            .collect::<Vec<_>>();
        let editor = &mut self.editor;
        let images = &mut self.images;
        let excerpts = &mut self.excerpts;
        let searches = &mut self.searches;
        let reviews = &mut self.reviews;
        let agents = &mut self.agents;
        let sessions = &self.sessions;
        let bootstrap = &self.preferences.bootstrap;
        let mut followed_outlines = Vec::new();
        let mut tools = BTreeSet::new();
        self.panes = crate::panes::PaneTree::restored(saved, &mut |tab| {
            if tab.kind == SavedKind::Tool {
                let tool = tab.tool?;
                return tools.insert(tool).then_some((None, Item::Tool(tool)));
            }
            if tab.kind == SavedKind::Settings {
                return None;
            }
            let (checkout, project) = projects
                .iter()
                .find(|(root, _)| *root == tab.project)
                .cloned()?;
            let held = sessions
                .of(project)
                .find(|session| session.root() == tab.worktree);
            let env = bootstrap.env(held.and_then(pm_core::Session::port));
            let session = held.map(pm_core::Session::id);
            let scope = match session {
                Some(session) => Scope::of(project, session),
                None => Scope::checkout(project),
            };
            let root = match session {
                Some(_) => tab.worktree.clone(),
                None => checkout,
            };

            if tab.kind == SavedKind::Agent {
                let agent = pm_acp::Agent::named(&tab.agent)?;
                let talk = match tab.session.is_empty() {
                    true => agents.start(project, session, &root, &env, agent)?,
                    false => agents.resume(project, session, &root, &env, agent, &tab.session)?,
                };
                if let Some(opened) = agents.get_mut(talk) {
                    opened.entitle(&tab.title);
                }
                return Some((Some(scope), Item::Agent(scope, talk)));
            }
            if tab.kind == SavedKind::Review {
                return Some((Some(scope), Item::Review(scope)));
            }
            if tab.kind == SavedKind::Outline {
                if !tab.path.as_os_str().is_empty() {
                    followed_outlines.push((scope, tab.path.clone()));
                }
                return Some((Some(scope), Item::Outline(scope)));
            }
            if tab.kind == SavedKind::Excerpts {
                reviews
                    .entry(scope)
                    .or_insert_with(|| crate::review::Review::of(&root));
                excerpts.entry(scope).or_insert_with(|| {
                    std::rc::Rc::new(std::cell::RefCell::new(crate::excerpts::Excerpts::default()))
                });
                return Some((Some(scope), Item::Excerpts(scope)));
            }
            if tab.kind == SavedKind::Search {
                searches
                    .entry(scope)
                    .or_insert_with(super::search::ProjectSearch::new);
                return Some((Some(scope), Item::Search(scope)));
            }
            if tab.kind == SavedKind::Image {
                let image = images.open(scope, &tab.path, tab.preview);
                return Some((Some(scope), Item::Image(image)));
            }
            if tab.kind == SavedKind::Rendered {
                let file = editor.open(scope, &root, &tab.path, false)?;
                return Some((Some(scope), Item::Rendered(file)));
            }
            if tab.kind == SavedKind::Change {
                let review = reviews
                    .entry(scope)
                    .or_insert_with(|| crate::review::Review::of(&root));
                let change = review.name(&tab.path);
                review.keep(change);
                return Some((Some(scope), Item::Change(scope, change)));
            }
            let file = editor.open(scope, &root, &tab.path, tab.preview)?;
            if let Some(document) = editor.get(file) {
                let mut document = document.borrow_mut();
                document.restore(tab.line, tab.column, tab.scroll);
            }
            Some((Some(scope), Item::File(file)))
        });
        self.content_pane = self.panes.panes().into_iter().find(|id| {
            self.panes.pane(*id).is_some_and(|pane| {
                pane.is_empty() || pane.items().any(|item| !matches!(item, Item::Tool(_)))
            })
        });
        for (scope, path) in followed_outlines {
            let Some(file) = self.editor.opened(scope, &path) else {
                continue;
            };
            let pane = self.panes.panes().into_iter().find(|pane| {
                self.panes
                    .pane(*pane)
                    .is_some_and(|pane| pane.items().any(|item| item == Item::File(file)))
            });
            if let Some(pane) = pane {
                self.outlines.follow(scope, file, pane);
            }
        }
        let unread = self
            .reviews
            .iter()
            .filter(|(_, review)| !review.is_read())
            .map(|(scope, _)| *scope)
            .collect::<Vec<_>>();
        for scope in unread {
            self.reread_review_later(scope);
        }
        self.refresh_excerpts();
        self.sweep();
        self.focus_pane(self.panes.focus());
        if super::settings::settings_was_open(&saved.root) {
            self.open_settings();
        }
    }

    /// Opens what the reviews in the panes show, then closes every file no
    /// pane is holding open any more.
    pub(super) fn sweep(&mut self) {
        self.open_reviewed_files();
        let held = self.panes.held();
        self.excerpts
            .retain(|scope, _| held.contains(&Item::Excerpts(*scope)));
        self.searches
            .retain(|scope, _| held.contains(&Item::Search(*scope)));
        for search in self.searches.values_mut() {
            search.held.retain(|file| self.editor.is_dirty(*file));
        }
        let rendered = held
            .iter()
            .copied()
            .filter_map(Item::rendered)
            .collect::<BTreeSet<_>>();
        let images = held
            .iter()
            .copied()
            .filter_map(Item::image)
            .collect::<BTreeSet<_>>();
        let mut files = held
            .iter()
            .copied()
            .filter_map(Item::file)
            .collect::<BTreeSet<_>>();
        files.extend(&rendered);
        files.extend(self.reviewed_files());
        files.extend(self.excerpted_files());
        files.extend(self.searches.values().flat_map(|search| {
            search
                .excerpts
                .borrow()
                .files()
                .iter()
                .map(|file| file.file)
                .collect::<Vec<_>>()
        }));
        files.extend(
            self.searches
                .values()
                .flat_map(|search| search.held.iter().copied()),
        );
        self.images.retain(&images);
        self.renders.retain(&rendered);
        let sessions = held
            .iter()
            .copied()
            .filter_map(Item::session)
            .collect::<BTreeSet<_>>();
        self.editor.retain(&files);
        self.outlines.retain(|file| files.contains(&file));
        self.agents.retain(&sessions);
        self.sweep_errands();
        if let Some(crate::app::Writing::Prompt(open)) = self.writing
            && !sessions.contains(&open)
        {
            self.writing = None;
        }
    }

    /// Takes the files of `project` out of every pane that was showing them.
    pub(super) fn drop_project_tabs(&mut self, project: pm_core::ProjectId) {
        self.drop_tabs(&|scope| scope.project() == project);
    }

    /// Closes every tab of the worktrees `leaving` names, wherever they are.
    pub(super) fn drop_tabs(&mut self, leaving: &dyn Fn(Scope) -> bool) {
        let gone = self
            .panes
            .held()
            .into_iter()
            .filter(|item| self.scope_of(*item).is_some_and(leaving))
            .collect::<BTreeSet<_>>();
        self.panes.retain(|item| !gone.contains(&item));
        self.panes.close_empty();
    }

    /// Carries a tab, or lets go of it where the pointer has reached.
    ///
    /// A press that goes nowhere before it is let go of is the click that
    /// selects the tab, which is what makes one gesture out of two: the tab
    /// is picked up by the same press that would have selected it, and only
    /// travel tells the two apart.
    pub(super) fn drag_tab(&mut self, pane: PaneId, item: Item, event: ResizeEvent) {
        let travelled = (event.current.x - event.start.x).hypot(event.current.y - event.start.y);
        let order = |pane: PaneId| self.tabs_of(pane);
        let target = self.geometry.target_at(event.current, &order);
        let drag = TabDrag {
            from: pane,
            item,
            at: event.current,
            travelled,
            target,
        };

        if event.phase != ResizePhase::Ended {
            self.drag = Some(drag);
            return;
        }
        self.drag = None;
        self.let_go_of(drag);
    }

    /// Lets go of the tab the pointer is carrying, wherever it was noticed.
    ///
    /// The release is answered here as well as through the drag itself, so a
    /// button let go of over something that was never a drop target still
    /// ends the gesture rather than leaving a tab stuck to the pointer.
    pub(super) fn release_drag(&mut self) {
        if let Some(drag) = self.drag.take() {
            self.let_go_of(drag);
        }
    }

    /// Selects the tab a press went nowhere on, or drops one that travelled.
    fn let_go_of(&mut self, drag: TabDrag) {
        if drag.is_carried() {
            self.drop_tab(drag);
        } else {
            self.select_tab(drag.from, drag.item);
        }
    }

    /// Shows the tab that was clicked, keeping the file on a second click.
    pub(super) fn select_tab(&mut self, pane: PaneId, item: Item) {
        if self.tab_clicks.press(item) >= 2 {
            match item {
                Item::File(file) => self.editor.keep(file),
                Item::Image(image) => self.images.keep(image),
                Item::Change(project, change) => self.keep_change(project, change),
                Item::Rendered(_)
                | Item::Outline(_)
                | Item::Review(_)
                | Item::Excerpts(_)
                | Item::Search(_)
                | Item::Agent(..)
                | Item::Tool(_) => {}
            }
        }
        self.activate_tab(pane, item);
    }

    /// Pins `pane`'s tab for `file` in every project, or lets it go again.
    ///
    /// A pinned tab is drawn whichever project the window is showing, which
    /// is the one way a file stays in front of every worktree at once.
    pub(super) fn toggle_pin(&mut self, pane: PaneId, item: Item) {
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.toggle_pin(item);
        }
        self.store();
    }

    /// Brings `pane`'s tab for `file` in front, and gives the pane the keyboard.
    ///
    /// Leaving one tab for another is a jump like following a definition is,
    /// so the place left behind goes on the trail: going back returns to the
    /// tab that was in front, at the line it was left at.
    pub(super) fn activate_tab(&mut self, pane: PaneId, item: Item) {
        if let Some(ui) = self.ui.as_mut() {
            ui.clear_text_selection();
        }
        self.agents.clear_selections();
        self.dismiss_prediction();
        if self
            .scope()
            .and_then(|scope| self.panes.pane(pane)?.active(scope))
            != Some(item)
            && let Some(from) = self.place_in(pane)
        {
            self.trail.jumped(from);
        }
        let scope = self.scope();
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.activate(scope, item);
        }
        self.focus_pane(pane);
        self.store();
    }

    /// Lets go of a carried tab where the pointer has reached.
    ///
    /// The tab moves rather than copies: it leaves the pane it came from,
    /// the way dragging a tab does everywhere, and the pane it leaves empty
    /// gives its room back to its neighbours.
    fn drop_tab(&mut self, drag: TabDrag) {
        let Some((target, place)) = drag.target else {
            return;
        };
        let scope = self.scope();
        if target == drag.from
            && let DropPlace::Tab(index) = place
        {
            if let Some(pane) = self.panes.pane_mut(target) {
                pane.place(scope, drag.item, index);
            }
            self.focus_pane(target);
            return self.store();
        }

        let (landed, index) = match place {
            DropPlace::Split(direction) => match self.panes.split(target, direction) {
                Some(fresh) => (fresh, None),
                None => return,
            },
            DropPlace::Into => (target, None),
            DropPlace::Tab(index) => (target, Some(index)),
        };
        let carried = self
            .panes
            .pane_mut(drag.from)
            .and_then(|pane| pane.take(drag.item));
        if let (Some(tab), Some(pane)) = (carried, self.panes.pane_mut(landed)) {
            match index {
                Some(index) => pane.insert(tab, scope, index),
                None => pane.append(tab, scope),
            }
        }
        self.panes.close_if_empty(drag.from);
        self.focus_pane(landed);
        self.sweep();
        self.store();
    }

    /// What is open in `pane`, in the order its tabs are drawn.
    pub(super) fn tabs_of(&self, pane: PaneId) -> Vec<Item> {
        self.panes
            .pane(pane)
            .map(|pane| pane.tabs(self.scope()))
            .unwrap_or_default()
    }

    /// The part of the window a carried tab would take, if it were let go of.
    pub(super) fn drop_highlight(&self) -> Option<Rect> {
        let drag = self.drag.as_ref()?;
        if !drag.is_carried() {
            return None;
        }
        let (pane, place) = drag.target?;
        match place {
            DropPlace::Tab(index) => self.geometry.caret(pane, index, &self.tabs_of(pane)),
            place => Some(highlight(self.geometry.pane_bounds(pane)?, place)),
        }
    }

    /// The tab being carried right now, and what it is called.
    pub(super) fn carried_tab(&self) -> Option<(pm_gfx::Point, String)> {
        let drag = self.drag.as_ref().filter(|drag| drag.is_carried())?;
        Some((drag.at, self.tab_entry(drag.item)?.name))
    }

    /// One tab of a pane as its bar presents it.
    ///
    /// The bar draws the same three marks whatever a tab holds; what it holds
    /// is only where the name, the icon and the marks are read from.
    pub(super) fn tab_entry(&self, item: Item) -> Option<TabEntry> {
        match item {
            Item::Tool(tool) => Some(TabEntry {
                item,
                name: tool.label().to_owned(),
                icon: tool.icon(),
                dirty: false,
                preview: false,
                pinned: false,
            }),
            Item::File(file) => {
                let FileEntry {
                    name,
                    dirty,
                    preview,
                } = self.editor.entry(file)?;
                Some(TabEntry {
                    item,
                    name,
                    icon: IconName::File,
                    dirty,
                    preview,
                    pinned: false,
                })
            }
            Item::Image(image) => Some(TabEntry {
                item,
                name: self.images.name(image)?,
                icon: IconName::Image,
                dirty: false,
                preview: self.images.is_preview(image),
                pinned: false,
            }),
            Item::Rendered(file) => Some(TabEntry {
                item,
                name: format!("Preview {}", self.editor.entry(file)?.name),
                icon: IconName::Eye,
                dirty: false,
                preview: false,
                pinned: false,
            }),
            Item::Outline(scope) => Some(TabEntry {
                item,
                name: self
                    .outlines
                    .followed(scope)
                    .and_then(|file| self.editor.entry(file))
                    .map_or_else(
                        || "Outline".to_owned(),
                        |entry| format!("Outline · {}", entry.name),
                    ),
                icon: IconName::Collapse,
                dirty: false,
                preview: false,
                pinned: false,
            }),
            Item::Excerpts(scope) => Some(TabEntry {
                item,
                name: match self.open.get(scope.project()) {
                    Some(_) => "Edit Changes".to_owned(),
                    None => return None,
                },
                icon: IconName::GitCompare,
                dirty: self.excerpts_dirty(scope),
                preview: false,
                pinned: false,
            }),
            Item::Search(scope) => Some(TabEntry {
                item,
                name: "Search: Replace in Project".to_owned(),
                icon: IconName::Search,
                dirty: self.is_dirty(Item::Search(scope)),
                preview: false,
                pinned: false,
            }),
            Item::Review(scope) => Some(TabEntry {
                item,
                name: match self.open.get(scope.project()) {
                    Some(_) => "Uncommitted Changes".to_owned(),
                    None => return None,
                },
                icon: IconName::GitCompare,
                dirty: false,
                preview: false,
                pinned: false,
            }),
            Item::Agent(_, session) => {
                let talk = self.agents.get(session)?;
                Some(TabEntry {
                    item,
                    name: talk
                        .title()
                        .map_or_else(|| talk.agent().name.to_owned(), shortened),
                    icon: IconName::Sparkle,
                    dirty: false,
                    preview: false,
                    pinned: false,
                })
            }
            Item::Change(project, change) => {
                let review = self.reviews.get(&project)?;
                let path = review.path_of(change)?;
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some(TabEntry {
                    item,
                    name: format!("{name} (diff)"),
                    icon: IconName::GitCompare,
                    dirty: false,
                    preview: review.is_preview(change),
                    pinned: false,
                })
            }
        }
    }

    /// Builds the tree of panes as the screen draws it.
    ///
    /// Building the frame is also when the panes and tabs of it are handed
    /// the cells they report their bounds in, so what the window hit-tests a
    /// drop against is exactly what the last frame drew.
    pub(super) fn pane_view(&mut self, theme: &Theme) -> Box<dyn Element<Message>> {
        self.tree_scroll();
        let mut drawn = Vec::new();
        let mut drawn_tabs = Vec::new();
        let mut cells = Vec::new();
        self.panes.keep_focus_drawn(self.scope());
        for pane in self.panes.drawn(self.scope()) {
            drawn.push(pane);
            for item in self.tabs_of(pane) {
                drawn_tabs.push((pane, item));
            }
        }
        self.geometry.keep(&drawn, &drawn_tabs);
        for pane in &drawn {
            let tabs = self
                .tabs_of(*pane)
                .into_iter()
                .map(|item| self.geometry.tab(*pane, item))
                .collect::<Vec<_>>();
            let bar = self.geometry.bar(*pane);
            if tabs.is_empty() {
                bar.set(Rect::from_xywh(0.0, 0.0, 0.0, 0.0));
            }
            cells.push((self.geometry.pane(*pane), bar, tabs));
        }

        let scope = self.scope();
        let link = self.link_target();
        let talked_about = self.hovered_name();
        let caret = self.caret_solid();
        let display = self.preferences.display;
        let shortcuts = self.empty_pane_shortcuts();
        let cells = drawn.into_iter().zip(cells).collect::<Vec<_>>();
        panes::pane_tree(
            theme,
            &self.panes,
            scope,
            self.editor_focused
                || self.tree_focused
                || self.changes_focused
                || self.terminal_focused,
            &|pane| {
                let (bounds, bar, tab_bounds) = cells
                    .iter()
                    .find(|(id, _)| *id == pane.id())
                    .map(|(_, cells)| cells.clone())
                    .unwrap_or_else(|| (unmeasured(), unmeasured(), Vec::new()));
                let active = pane.active(scope);
                let file = active.and_then(Item::file);
                let display = self.display_of(file, display);
                let conflicted = file.is_some_and(|file| {
                    let Some(scope) = self.editor.scope_of(file) else {
                        return false;
                    };
                    let Some(document) = self.editor.get(file) else {
                        return false;
                    };
                    self.reviews
                        .get(&scope)
                        .and_then(|review| review.mark(document.borrow().buffer().path()))
                        == Some(FileStatus::Conflicted)
                });
                Contents {
                    tabs: pane
                        .tabs(scope)
                        .into_iter()
                        .filter_map(|item| {
                            let mut entry = self.tab_entry(item)?;
                            entry.pinned = pane.is_pinned(item);
                            Some(entry)
                        })
                        .collect(),
                    active,
                    content: self.shown(theme, pane.id(), active, bounds.get().size.width),
                    conflicted,
                    bounds,
                    bar,
                    tab_bounds,
                    link: link
                        .clone()
                        .filter(|(open, _)| file == Some(*open))
                        .map(|(_, span)| span),
                    hovered: talked_about
                        .clone()
                        .filter(|(open, _)| file == Some(*open))
                        .map(|(_, span)| span),
                    found: self.found_in(file),
                    breakpoints: file
                        .map(|file| self.breakpoints_of(file))
                        .unwrap_or_default(),
                    stopped: file.and_then(|file| self.stopped_in(file)),
                    caret,
                    prediction_visible: self.preferences.edit_predictions.enabled
                        && self.completions.is_none(),
                    display,
                    crumbs: file
                        .filter(|_| display.breadcrumbs)
                        .and_then(|file| self.crumbs_of(file)),
                    shortcuts: shortcuts.clone(),
                }
            },
        )
    }

    /// The commands an empty pane offers, each with the keys it answers to
    /// here, leaving out any that nothing is bound to.
    fn empty_pane_shortcuts(&self) -> Vec<Shortcut> {
        let context = self.context();
        EMPTY_PANE_COMMANDS
            .into_iter()
            .filter_map(|action| {
                Some(Shortcut {
                    title: action.title(),
                    keys: self.keys_for(action, &context)?,
                })
            })
            .collect()
    }

    /// Where `file` is in its worktree, and where its cursor is in it.
    fn crumbs_of(&self, file: FileId) -> Option<crate::editor::Crumbs> {
        let root = self.worktree_of(file)?;
        let document = self.editor.get(file)?;
        let document = document.borrow();
        Some(crate::editor::Crumbs::of(document.buffer(), &root))
    }

    /// The matches of modal editing's search on the lines `file`'s pane
    /// shows, none when the pane shows no file.
    fn found_in(&self, file: Option<FileId>) -> Vec<std::ops::Range<pm_text::Position>> {
        let Some(document) = file.and_then(|file| self.editor.get(file)) else {
            return Vec::new();
        };
        let (top, rows) = {
            let document = document.borrow();
            (document.scroll(), document.rows())
        };
        self.vim_matches(&document, top..top + rows.max(1) * 2 + 1)
    }

    /// What a pane showing `file` draws around its text: `display`, with the
    /// cursor shaped by the file's mode while modal editing is on.
    fn display_of(&self, file: Option<FileId>, display: Display) -> Display {
        let Some(document) = file
            .filter(|_| self.preferences.vim_mode)
            .and_then(|file| self.editor.get(file))
        else {
            return display;
        };
        let document = document.borrow();
        let modal = document.modal();
        Display {
            cursor_shape: display.cursor_shape.modal(modal.shape()),
            whole_lines: modal.mode() == pm_vim::Mode::VisualLine,
            ..display
        }
    }

    /// What a pane showing `item` draws beneath its bar of tabs.
    fn shown(&self, theme: &Theme, pane: PaneId, item: Option<Item>, width: f32) -> Content {
        match item {
            Some(Item::Tool(tool)) => {
                Content::Built(Box::new(self.tool_content(theme, tool, width)))
            }
            Some(Item::File(file)) => match self.editor.get(file) {
                Some(document) => Content::File(document),
                None => Content::Empty,
            },
            Some(Item::Review(project)) => match self.reviews.get(&project) {
                Some(review) => Content::Built(Box::new(crate::review::review_pane(
                    theme,
                    review,
                    self.writing == Some(crate::app::Writing::Commit),
                    self.caret_solid(),
                    self.preferences.split_diff,
                    self.remarking(project),
                ))),
                None => Content::Empty,
            },
            Some(Item::Agent(_, session)) => match self.agents.get(session) {
                Some(talk) => Content::Built(Box::new(crate::agent::agent_pane(
                    theme,
                    talk,
                    self.writing == Some(crate::app::Writing::Prompt(session)),
                    match self.writing {
                        Some(crate::app::Writing::Answer(answered, ticket, place))
                            if answered == session =>
                        {
                            Some((ticket, place))
                        }
                        _ => None,
                    },
                    self.caret_solid(),
                    width,
                ))),
                None => Content::Empty,
            },
            Some(Item::Change(project, change)) => match self.reviews.get(&project) {
                Some(review) => Content::Built(Box::new(crate::review::change_pane(
                    theme,
                    review,
                    change,
                    self.preferences.split_diff,
                    self.remarking(project),
                ))),
                None => Content::Empty,
            },
            Some(Item::Image(image)) => {
                let shown = self
                    .images
                    .scope_of(image)
                    .and_then(|scope| self.root_of(scope))
                    .and_then(|root| self.images.shown(image, &root));
                match shown {
                    Some(shown) => Content::Built(Box::new(crate::image::image_pane(theme, shown))),
                    None => Content::Empty,
                }
            }
            Some(Item::Rendered(file)) => match self.editor.get(file) {
                Some(document) => {
                    let document = document.borrow();
                    let buffer = document.buffer();
                    let blocks = self
                        .renders
                        .blocks(file, buffer.version(), || buffer.contents());
                    Content::Built(Box::new(crate::markdown::rendered_pane(
                        theme,
                        &blocks,
                        self.renders.scroll(file),
                        file,
                        self.window
                            .as_ref()
                            .map_or(1.0, |window| window.scale_factor() as f32),
                        buffer.path(),
                        &self.renders,
                    )))
                }
                None => Content::Empty,
            },
            Some(Item::Outline(scope)) => {
                Content::Built(Box::new(self.outline_content(theme, pane, scope)))
            }
            Some(Item::Excerpts(scope)) => match self.excerpts.get(&scope) {
                Some(excerpts) => Content::Excerpts(excerpts.clone(), self.remarking(scope)),
                None => Content::Empty,
            },
            Some(Item::Search(scope)) if self.searches.contains_key(&scope) => {
                Content::Built(self.project_search_content(theme, pane, scope))
            }
            Some(Item::Search(_)) => Content::Empty,
            None => Content::Empty,
        }
    }

    /// The menu open over the panes, and what it holds.
    pub(super) fn menu_items(&self) -> Option<(TabMenu, Vec<MenuItem<Message>>)> {
        let open = self.menu?;
        let items = match open.target {
            MenuTarget::Tab(pane, item) => {
                let tabs = self
                    .tabs_of(pane)
                    .into_iter()
                    .filter_map(|item| self.tab_entry(item))
                    .collect::<Vec<_>>();
                let pane = self.panes.pane(pane)?;
                let tabs = tabs
                    .into_iter()
                    .map(|mut entry| {
                        entry.pinned = pane.is_pinned(entry.item);
                        entry
                    })
                    .collect::<Vec<_>>();
                panes::tab_menu(pane, &tabs, item)
            }
            MenuTarget::Pane(pane) => {
                let mut items = self.tool_menu(Some(pane));
                items.push(pm_ui::menu_separator());
                items.extend(panes::pane_menu(pane, self.panes.is_split()));
                items
            }
            MenuTarget::Tools => {
                let mut items = self.tool_menu(None);
                items.push(pm_ui::menu_separator());
                items.push(pm_ui::menu_entry(
                    "Reset Window Layout",
                    Some(Message::ResetWindowLayout),
                ));
                items
            }
            MenuTarget::Project(project) => {
                let mut items = crate::workspace::project_menu_items(
                    self.open.get(project)?,
                    &self.session_bases,
                    self.showing_bases,
                );
                items.extend(self.project_group_items(project));
                items
            }
            MenuTarget::ProjectGroup(index) => {
                self.project_groups.get(index)?;
                vec![
                    pm_ui::menu_entry("Rename Group…", Some(Message::RenameProjectGroup(index))),
                    pm_ui::menu_entry("Remove Group", Some(Message::RemoveProjectGroup(index))),
                ]
            }
            MenuTarget::Projects => crate::workspace::add_project_items(),
            MenuTarget::Session(session) => {
                let held = self.sessions.get(session)?;
                let scope = Scope::of(held.project(), session);
                crate::workspace::session_menu_items(session, scope, self.checks.detail(scope))
            }
            MenuTarget::Text(pane) => {
                let file = self.file_in(self.panes.pane(pane)?.active(self.scope()?)?)?;
                let document = self.editor.get(file)?;
                let document = document.borrow();
                crate::editor::text_menu(&crate::editor::TextMenu {
                    pane,
                    file,
                    selected: !document.buffer().selection().is_empty(),
                    debugging: self
                        .editor
                        .scope_of(file)
                        .is_some_and(|scope| self.debuggers.live(scope).is_some()),
                    served: document.is_served(),
                    tracked: document.is_tracked(),
                })
            }
            MenuTarget::Breakpoint(pane, line) => {
                let file = self.file_in(self.panes.pane(pane)?.active(self.scope()?)?)?;
                let scope = self.editor.scope_of(file)?;
                let path = self.editor.path(file)?;
                let present = self.debuggers.breakpoint(scope, &path, line).is_some();
                vec![
                    pm_ui::menu_entry(
                        if present {
                            "Remove Breakpoint"
                        } else {
                            "Add Breakpoint"
                        },
                        Some(Message::ToggleBreakpoint(
                            pane,
                            pm_text::Position::new(line, 0),
                        )),
                    ),
                    pm_ui::menu_entry(
                        "Edit Condition…",
                        Some(Message::EditBreakpoint(
                            pane,
                            line,
                            crate::picker::Kind::BreakpointCondition,
                        )),
                    ),
                    pm_ui::menu_entry(
                        "Edit Hit Count…",
                        Some(Message::EditBreakpoint(
                            pane,
                            line,
                            crate::picker::Kind::BreakpointHits,
                        )),
                    ),
                    pm_ui::menu_entry(
                        "Edit Log Message…",
                        Some(Message::EditBreakpoint(
                            pane,
                            line,
                            crate::picker::Kind::BreakpointLog,
                        )),
                    ),
                ]
            }
            MenuTarget::AgentServer(index) => vec![
                pm_ui::menu_entry("Edit…", Some(Message::EditAgentServer(index))),
                pm_ui::menu_separator(),
                pm_ui::menu_entry("Remove", Some(Message::RemoveAgentServer(index))),
            ],
            MenuTarget::McpServer(index) => {
                let server = self.mcp_servers.get(index)?;
                let toggle = match server.enabled {
                    true => "Disable",
                    false => "Enable",
                };
                let website =
                    (!server.website.is_empty()).then_some(Message::OpenMcpWebsite(index));
                vec![
                    pm_ui::menu_entry(toggle, Some(Message::ToggleMcpServer(index))),
                    pm_ui::menu_entry("Edit…", Some(Message::EditMcpServer(index))),
                    pm_ui::menu_separator(),
                    pm_ui::menu_entry(
                        "Copy Configuration (JSON)",
                        Some(Message::CopyMcpConfiguration(index)),
                    ),
                    pm_ui::menu_entry("Show Settings File", Some(Message::RevealSettingsFile)),
                    pm_ui::menu_entry("Open Website", website),
                    pm_ui::menu_separator(),
                    pm_ui::menu_entry("Uninstall", Some(Message::RemoveMcpServer(index))),
                ]
            }
            MenuTarget::AgentMcp(session) => {
                let servers = self
                    .agents
                    .get(session)
                    .map(|talk| talk.mcp_servers())
                    .unwrap_or_default();
                let heading = match servers.is_empty() {
                    true => "No MCP servers are set up",
                    false => "Given to this agent when it started",
                };
                std::iter::once(pm_ui::menu_entry(heading, None))
                    .chain(servers.iter().map(|server| {
                        let note = match server.given {
                            true => "",
                            false => " · not supported by this agent",
                        };
                        pm_ui::menu_entry(format!("{} · {}{note}", server.name, server.kind), None)
                    }))
                    .chain([
                        pm_ui::menu_separator(),
                        pm_ui::menu_entry("Manage MCP Servers…", Some(Message::ManageMcpServers)),
                    ])
                    .collect()
            }
            MenuTarget::AgentText(session, reply) => {
                let selected = self
                    .agents
                    .get(session)
                    .is_some_and(|talk| talk.selection().is_some());
                let mut entries = Vec::new();
                if let Some(block) = reply {
                    entries.extend([
                        pm_ui::menu_entry(
                            "Copy Reply",
                            Some(Message::CopyAgentReply(session, block, Some(false))),
                        ),
                        pm_ui::menu_entry(
                            "Copy Reply Formatted",
                            Some(Message::CopyAgentReply(session, block, Some(true))),
                        ),
                        pm_ui::menu_separator(),
                    ]);
                }
                entries.extend([
                    pm_ui::menu_entry("Copy", selected.then_some(Message::CopyAgentText(session))),
                    pm_ui::menu_separator(),
                    pm_ui::menu_entry("Select All", Some(Message::SelectAllAgentText(session))),
                ]);
                entries
            }
            MenuTarget::ReadingText => vec![
                pm_ui::menu_entry("Copy", Some(Message::CopyText)),
                pm_ui::menu_separator(),
                pm_ui::menu_entry("Select All", Some(Message::SelectAllText)),
            ],
            MenuTarget::Input => {
                let selected = self
                    .typed_into()
                    .is_some_and(|text| !text.borrow().buffer().selection().is_empty());
                crate::input::input_menu(selected)
            }
            MenuTarget::Change => crate::review::change_menu(self.review()?),
            MenuTarget::Commit => {
                let operation = self
                    .review()
                    .and_then(|review| review.head())
                    .and_then(|head| head.operation.as_ref());
                commit_menu_items(operation)
            }
            MenuTarget::SourceControl => {
                let operation = self
                    .review()
                    .and_then(|review| review.head())
                    .and_then(|head| head.operation.as_ref());
                let has_stashes = self.stash_available;
                let mut items = vec![
                    pm_ui::menu_entry("Pull", Some(Message::Pull)),
                    pm_ui::menu_entry("Push", Some(Message::PushBranch)),
                    pm_ui::menu_entry("Clone Repository…", Some(Message::CloneProject)),
                    pm_ui::menu_entry("Checkout…", Some(Message::ShowStatusBranches)),
                    pm_ui::menu_entry("Fetch", Some(Message::Fetch)),
                    pm_ui::menu_separator(),
                    pm_ui::menu_entry("Stage All Changes", Some(Message::StageAll)),
                    pm_ui::menu_entry("Unstage All Changes", Some(Message::UnstageAll)),
                    pm_ui::menu_entry("Pull (Rebase)", Some(Message::PullRebase)),
                    pm_ui::menu_entry("Force Push", Some(Message::ForcePush)),
                    pm_ui::menu_separator(),
                    pm_ui::menu_entry("Refresh", Some(Message::RefreshChanges)),
                ];
                items.extend(commit_menu_items(operation));
                items.extend([
                    pm_ui::menu_separator(),
                    pm_ui::menu_entry("Stash…", Some(Message::StashPush)),
                    pm_ui::menu_entry(
                        "Apply Stash…",
                        has_stashes
                            .then_some(Message::ShowStashes(crate::review::StashAction::Apply)),
                    ),
                    pm_ui::menu_entry(
                        "Pop Stash…",
                        has_stashes
                            .then_some(Message::ShowStashes(crate::review::StashAction::Pop)),
                    ),
                    pm_ui::menu_entry(
                        "Drop Stash…",
                        has_stashes
                            .then_some(Message::ShowStashes(crate::review::StashAction::Drop)),
                    ),
                ]);
                items
            }
            MenuTarget::Agents(standing) => self.agents_menu(standing),
            MenuTarget::HistoryRefs => vec![
                pm_ui::menu_entry("Auto", Some(Message::SetHistoryFilter(false))),
                pm_ui::menu_entry("All", Some(Message::SetHistoryFilter(true))),
            ],
            MenuTarget::History(repository, _) => {
                let review = self.review()?;
                let object = self.history_menu_object.as_ref()?;
                if self.scope() != self.history_menu_scope {
                    return None;
                }
                let head = review.head();
                let can_pick = review.active() == repository
                    && head.is_some_and(|head| head.operation.is_none())
                    && !review.contains_commit(object);
                let branch = head.map(pm_core::Head::name).unwrap_or_default();
                vec![
                    pm_ui::menu_entry(
                        format!("Cherry-Pick into {branch}"),
                        can_pick.then_some(Message::CherryPickHistory),
                    ),
                    pm_ui::menu_entry("Copy Commit Hash", Some(Message::CopyCommitHash)),
                ]
            }
            MenuTarget::Unsaved(pane, file) => {
                let name = self.editor.entry(file)?.name;
                panes::unsaved_menu(pane, file, &name)
            }
            MenuTarget::Entry(id) => self.entry_menu(id),
            MenuTarget::Tree => self.tree_menu(),
            MenuTarget::CodeActions => self
                .code_actions
                .iter()
                .enumerate()
                .map(|(index, action)| {
                    pm_ui::menu_entry(
                        action.action.title.clone(),
                        Some(Message::TakeCodeAction(index)),
                    )
                })
                .collect(),
            MenuTarget::Terminal(shell) => {
                let shells = self
                    .scope()
                    .map(|scope| self.terminals.list(scope))
                    .unwrap_or_default();
                crate::workspace::terminal_menu(&shells, shell)
            }
            MenuTarget::Screen => {
                let selected = self
                    .focused_shell()
                    .is_some_and(|shell| shell.borrow().selection_span().is_some());
                crate::terminal::screen_menu(selected)
            }
        };
        Some((open, items))
    }
}

/// The commit and operation commands shared by both source control menus.
fn commit_menu_items(operation: Option<&pm_core::Operation>) -> Vec<MenuItem<Message>> {
    let title = match operation {
        Some(pm_core::Operation::Merge(_)) => "Commit Merge",
        Some(pm_core::Operation::Rebase(_)) => "Continue Rebase",
        Some(pm_core::Operation::CherryPick(_)) => "Continue Cherry-Pick",
        None => "Commit",
    };
    let mut items = vec![
        pm_ui::menu_entry(title, Some(Message::Commit)),
        pm_ui::menu_entry(
            "Commit and Push",
            operation.is_none().then_some(Message::CommitAndPush),
        ),
    ];
    if let Some(operation) = operation {
        items.push(pm_ui::menu_entry(
            format!("Abort {}", operation.name()),
            Some(Message::AbortMerge),
        ));
        if !matches!(operation, pm_core::Operation::Merge(_)) {
            items.push(pm_ui::menu_entry(
                "Skip Commit",
                Some(Message::SkipOperation),
            ));
        }
    } else {
        items.push(pm_ui::menu_entry("Amend Last Commit", Some(Message::Amend)));
    }
    items
}

/// `title` held to what a tab has room for, cut at a word where it can be.
fn shortened(title: &str) -> String {
    if title.chars().count() <= TAB_TITLE {
        return title.to_owned();
    }
    let cut = title.chars().take(TAB_TITLE).collect::<String>();
    let kept = cut.rsplit_once(' ').map_or(cut.as_str(), |(kept, _)| kept);
    format!("{}…", kept.trim_end())
}
