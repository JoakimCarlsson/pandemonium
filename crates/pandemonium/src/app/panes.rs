//! What the window does to its panes: splitting, closing, and what is in them.
//!
//! The pane tree says how the window is divided and which pane has the
//! keyboard; the file store says what the documents behind the tabs are.
//! Both are the window's, so this is where a command that needs the two of
//! them together lives — and the one place a tab is opened or closed, whether
//! a keybinding, a tab menu or the file tree asked for it.

use std::collections::BTreeSet;

use pm_core::ProjectId;
use pm_gfx::Rect;
use pm_ui::{Axis, Element, IconName, MenuItem, ResizeEvent, ResizePhase, Theme};

use crate::app::App;
use crate::app::drag::{DropPlace, TabDrag, highlight, unmeasured};
use crate::editor::{FileEntry, FileId, OpenFile};
use crate::message::Message;
use crate::panes::{
    self, Content, Contents, Item, PaneId, Saved, SavedKind, SavedTab, SplitDirection, TabEntry,
};
use crate::workspace::{MenuTarget, TabMenu};

impl App {
    /// The project the panes are showing, which is the active one.
    ///
    /// A tab belongs to the project its file was opened from and is drawn
    /// only while that project is the one the window is pointed at, the way
    /// a shell is. Every question about what a pane holds is asked of this.
    pub(super) fn scope(&self) -> Option<ProjectId> {
        self.open.active().map(pm_core::Project::id)
    }

    /// The file the pane with the keyboard is showing, when it shows one.
    pub(super) fn active_file(&self) -> Option<OpenFile> {
        self.editor.get(self.active_tab()?.file()?)
    }

    /// What the pane with the keyboard is showing.
    pub(super) fn active_tab(&self) -> Option<Item> {
        self.panes.focused()?.active(self.scope()?)
    }

    /// The file it is showing, for the commands that are a file's.
    pub(super) fn active_file_id(&self) -> Option<FileId> {
        self.active_tab()?.file()
    }

    /// Gives the keyboard to `pane`, taking it from the terminal.
    pub(super) fn focus_pane(&mut self, pane: PaneId) {
        self.panes.set_focus(pane);
        self.editor_focused = true;
        self.terminal_focused = false;
        self.changes_focused = false;
    }

    /// Opens `file` in `pane`, as a preview or to stay.
    ///
    /// A preview takes the pane's one preview tab from whatever held it,
    /// which is what makes clicking down a tree leave one tab behind rather
    /// than twenty — and it is the pane's tab, not the window's, so a file
    /// previewed on the right leaves the pane on the left as it was.
    pub(super) fn show_file(&mut self, pane: PaneId, file: FileId, preview: bool) {
        let Some(project) = self.editor.project_of(file) else {
            return;
        };
        self.show_item(pane, project, Item::File(file), preview);
    }

    /// Shows `item` of `project` in `pane`, opening a tab for it if need be.
    pub(super) fn show_item(
        &mut self,
        pane: PaneId,
        project: pm_core::ProjectId,
        item: Item,
        preview: bool,
    ) {
        if preview {
            self.close_previews(pane, item);
        }
        self.open.activate(project);
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.open(project, item);
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
            Item::Change(project, change) => self.is_change_preview(project, change),
            Item::Review(_) | Item::Agent(..) => false,
        }
    }

    /// Whether `item` holds changes that are not on disk.
    fn is_dirty(&self, item: Item) -> bool {
        item.file().is_some_and(|file| self.editor.is_dirty(file))
    }

    /// Divides `pane` that way, showing `file` in the pane that opens.
    ///
    /// Without a file of its own the new pane shows what the old one was
    /// showing, which is what splitting a pane is for: the same file, twice,
    /// at two places in it.
    pub(super) fn split_pane(
        &mut self,
        pane: PaneId,
        item: Option<Item>,
        direction: SplitDirection,
    ) {
        let item = item.or_else(|| {
            let scope = self.scope()?;
            self.panes.pane(pane)?.active(scope)
        });
        let project = item.and_then(|item| self.project_of(item));
        let Some(fresh) = self.panes.split(pane, direction) else {
            return;
        };
        if let (Some(item), Some(project), Some(fresh)) =
            (item, project, self.panes.pane_mut(fresh))
        {
            fresh.open(project, item);
        }
        self.editor_focused = true;
        self.terminal_focused = false;
        self.store();
    }

    /// Closes `pane`, leaving the window as it was when it has only the one.
    pub(super) fn close_pane(&mut self, pane: PaneId) {
        if self.panes.close(pane) {
            self.editor_focused = true;
            self.terminal_focused = false;
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

    /// The project `item` belongs to.
    pub(super) fn project_of(&self, item: Item) -> Option<ProjectId> {
        match item {
            Item::File(file) => self.editor.project_of(file),
            Item::Review(project) | Item::Change(project, _) | Item::Agent(project, _) => {
                Some(project)
            }
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

    /// The buffer being typed into that is not a pane's file, if there is one.
    ///
    /// A pane's file is not the only thing a reader writes in: an agent's
    /// prompt is a buffer too, and a command that edits text means whichever
    /// of them has the keyboard, not the file behind it.
    pub(super) fn typed_into(&self) -> Option<crate::editor::OpenFile> {
        let session = self.prompt_focused?;
        self.agents.get(session).map(crate::agent::Talk::prompt)
    }

    /// Changes the tabs of `pane` and closes whatever that left with nothing.
    pub(super) fn close_tabs(&mut self, pane: PaneId, close: impl FnOnce(&mut panes::Pane)) {
        let before = self.panes.held();
        if let Some(pane) = self.panes.pane_mut(pane) {
            close(pane);
        }
        self.panes.close_empty();
        self.remember_closed(&before);
        self.sweep();
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
                    project: self.editor.project_of(file)?,
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
        self.panes.save(&|item| {
            let project = self.project_of(item)?;
            let root = self.open.get(project)?.root().to_path_buf();
            if let Some(session) = item.session() {
                let talk = self.agents.get(session)?;
                return Some(SavedTab {
                    kind: SavedKind::Agent,
                    project: root,
                    agent: talk.agent().id.to_owned(),
                    session: talk.resumable().unwrap_or_default(),
                    ..SavedTab::default()
                });
            }
            if let Some(change) = item.change() {
                let path = self.reviews.get(&project)?.path_of(change)?;
                return Some(SavedTab {
                    kind: SavedKind::Change,
                    project: root,
                    path: path.to_path_buf(),
                    ..SavedTab::default()
                });
            }
            let Some(file) = item.file() else {
                return Some(SavedTab {
                    kind: SavedKind::Review,
                    project: root,
                    ..SavedTab::default()
                });
            };
            let document = self.editor.get(file)?;
            let document = document.borrow();
            let head = document.buffer().selection().head;
            Some(SavedTab {
                kind: SavedKind::File,
                project: root,
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
        let roots = self
            .open
            .iter()
            .map(|project| (project.root().to_path_buf(), project.id()))
            .collect::<Vec<_>>();
        let editor = &mut self.editor;
        let reviews = &mut self.reviews;
        let agents = &mut self.agents;
        self.panes = crate::panes::PaneTree::restored(saved, &mut |tab| {
            let (root, project) = roots
                .iter()
                .find(|(root, _)| *root == tab.project)
                .cloned()?;
            if tab.kind == SavedKind::Agent {
                let agent = pm_acp::Agent::named(&tab.agent)?;
                let session = match tab.session.is_empty() {
                    true => agents.start(project, &root, agent)?,
                    false => agents.resume(project, &root, agent, &tab.session)?,
                };
                return Some((project, Item::Agent(project, session)));
            }
            if tab.kind == SavedKind::Review {
                return Some((project, Item::Review(project)));
            }
            if tab.kind == SavedKind::Change {
                let review = reviews.get_mut(&project)?;
                let change = review.name(&tab.path);
                review.keep(change);
                return Some((project, Item::Change(project, change)));
            }
            let file = editor.open(project, &root, &tab.path, tab.preview)?;
            if let Some(document) = editor.get(file) {
                let mut document = document.borrow_mut();
                document.restore(tab.line, tab.column, tab.scroll);
            }
            Some((project, Item::File(file)))
        });
        self.sweep();
    }

    /// Closes every file no pane is holding open any more.
    pub(super) fn sweep(&mut self) {
        let held = self.panes.held();
        let files = held
            .iter()
            .copied()
            .filter_map(Item::file)
            .collect::<BTreeSet<_>>();
        let sessions = held
            .iter()
            .copied()
            .filter_map(Item::session)
            .collect::<BTreeSet<_>>();
        self.editor.retain(&files);
        self.agents.retain(&sessions);
        if self
            .prompt_focused
            .is_some_and(|open| !sessions.contains(&open))
        {
            self.prompt_focused = None;
        }
    }

    /// Takes the files of `project` out of every pane that was showing them.
    pub(super) fn drop_project_tabs(&mut self, project: pm_core::ProjectId) {
        let leaving = self
            .panes
            .held()
            .into_iter()
            .filter(|item| self.project_of(*item) == Some(project))
            .collect::<BTreeSet<_>>();
        self.panes.retain(|item| !leaving.contains(&item));
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
                Item::Change(project, change) => self.keep_change(project, change),
                Item::Review(_) | Item::Agent(..) => {}
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
        if self
            .scope()
            .and_then(|scope| self.panes.pane(pane)?.active(scope))
            != Some(item)
            && let Some(from) = self.place_in(pane)
        {
            self.trail.jumped(from);
        }
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.activate(item);
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
        let (Some((target, place)), Some(scope)) = (drag.target, self.scope()) else {
            return;
        };
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
                None => pane.append(tab),
            }
        }
        self.panes.close_empty();
        self.focus_pane(landed);
        self.sweep();
        self.store();
    }

    /// What is open in `pane`, in the order its tabs are drawn.
    pub(super) fn tabs_of(&self, pane: PaneId) -> Vec<Item> {
        let Some(scope) = self.scope() else {
            return Vec::new();
        };
        self.panes
            .pane(pane)
            .map(|pane| pane.tabs(scope))
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
            Item::Review(project) => Some(TabEntry {
                item,
                name: match self.open.get(project) {
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
                    name: talk.agent().name.to_owned(),
                    icon: IconName::Sparkle,
                    dirty: talk.is_busy(),
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
        let mut drawn = Vec::new();
        let mut drawn_tabs = Vec::new();
        let mut cells = Vec::new();
        for pane in self.panes.panes() {
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
        let caret = self.blink.is_solid();
        let cells = drawn.into_iter().zip(cells).collect::<Vec<_>>();
        panes::pane_tree(theme, &self.panes, self.editor_focused, &|pane| {
            let (bounds, bar, tab_bounds) = cells
                .iter()
                .find(|(id, _)| *id == pane.id())
                .map(|(_, cells)| cells.clone())
                .unwrap_or_else(|| (unmeasured(), unmeasured(), Vec::new()));
            let active = scope.and_then(|scope| pane.active(scope));
            let file = active.and_then(Item::file);
            Contents {
                tabs: scope
                    .map(|scope| pane.tabs(scope))
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| {
                        let mut entry = self.tab_entry(item)?;
                        entry.pinned = pane.is_pinned(item);
                        Some(entry)
                    })
                    .collect(),
                active,
                content: self.shown(theme, active, bounds.get().size.width),
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
                caret,
            }
        })
    }

    /// What a pane showing `item` draws beneath its bar of tabs.
    fn shown(&self, theme: &Theme, item: Option<Item>, width: f32) -> Content {
        match item {
            Some(Item::File(file)) => match self.editor.get(file) {
                Some(document) => Content::File(document),
                None => Content::Empty,
            },
            Some(Item::Review(project)) => match self.reviews.get(&project) {
                Some(review) => Content::Built(Box::new(crate::review::review_pane(
                    theme,
                    review,
                    self.commit_focused,
                ))),
                None => Content::Empty,
            },
            Some(Item::Agent(_, session)) => match self.agents.get(session) {
                Some(talk) => Content::Built(Box::new(crate::agent::agent_pane(
                    theme,
                    talk,
                    self.prompt_focused == Some(session),
                    width,
                ))),
                None => Content::Empty,
            },
            Some(Item::Change(project, change)) => match self.reviews.get(&project) {
                Some(review) => {
                    Content::Built(Box::new(crate::review::change_pane(theme, review, change)))
                }
                None => Content::Empty,
            },
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
            MenuTarget::Pane(pane) => panes::pane_menu(pane, self.panes.is_split()),
            MenuTarget::Text(pane) => {
                let file = self.panes.pane(pane)?.active(self.scope()?)?.file()?;
                let document = self.editor.get(file)?;
                let document = document.borrow();
                crate::editor::text_menu(&crate::editor::TextMenu {
                    pane,
                    file,
                    selected: !document.buffer().selection().is_empty(),
                    served: document.is_served(),
                    tracked: document.is_tracked(),
                })
            }
            MenuTarget::Change => crate::review::change_menu(self.review()?),
            MenuTarget::Remote => vec![
                pm_ui::menu_entry("Fetch", Some(Message::Fetch)),
                pm_ui::menu_entry("Fetch From", Some(Message::ChooseFetchRemote)),
                pm_ui::menu_entry("Pull", Some(Message::Pull)),
                pm_ui::menu_entry("Pull (Rebase)", Some(Message::PullRebase)),
                pm_ui::menu_separator(),
                pm_ui::menu_entry("Push", Some(Message::PushBranch)),
                pm_ui::menu_entry("Push To", Some(Message::ChoosePushRemote)),
                pm_ui::menu_entry("Force Push", Some(Message::ForcePush)),
            ],
            MenuTarget::Unsaved(pane, file) => {
                let name = self.editor.entry(file)?.name;
                panes::unsaved_menu(pane, file, &name)
            }
            MenuTarget::Entry(id) => {
                let project = self.open.active()?.id();
                let tree = self.files.get(&project)?;
                crate::tree::entry_menu(crate::tree::entry_of(tree, id)?)
            }
            MenuTarget::CodeActions => self
                .code_actions
                .iter()
                .enumerate()
                .map(|(index, action)| {
                    pm_ui::menu_entry(action.title.clone(), Some(Message::TakeCodeAction(index)))
                })
                .collect(),
            MenuTarget::Terminal(shell) => {
                let shells = self
                    .open
                    .active()
                    .map(|project| self.terminals.list(project.id()))
                    .unwrap_or_default();
                crate::workspace::terminal_menu(&shells, shell)
            }
        };
        Some((open, items))
    }
}
