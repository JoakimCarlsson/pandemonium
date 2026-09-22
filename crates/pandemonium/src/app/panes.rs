//! What the window does to its panes: splitting, closing, and what is in them.
//!
//! The pane tree says how the window is divided and which pane has the
//! keyboard; the file store says what the documents behind the tabs are.
//! Both are the window's, so this is where a command that needs the two of
//! them together lives — and the one place a tab is opened or closed, whether
//! a keybinding, a tab menu or the file tree asked for it.

use std::collections::BTreeSet;

use pm_gfx::Rect;
use pm_ui::{Axis, Element, MenuItem, ResizeEvent, ResizePhase, Theme};

use crate::app::App;
use crate::app::drag::{DropPlace, TabDrag, highlight, unmeasured};
use crate::editor::{FileId, OpenFile};
use crate::message::Message;
use crate::panes::{self, Contents, PaneId, Saved, SavedTab, SplitDirection};
use crate::workspace::{MenuTarget, TabMenu};

impl App {
    /// The file the pane with the keyboard is showing.
    pub(super) fn active_file(&self) -> Option<OpenFile> {
        self.editor.get(self.active_tab()?)
    }

    /// The tab in front of the pane with the keyboard.
    pub(super) fn active_tab(&self) -> Option<FileId> {
        self.panes.focused()?.active()
    }

    /// Gives the keyboard to `pane`, taking it from the terminal.
    pub(super) fn focus_pane(&mut self, pane: PaneId) {
        self.panes.set_focus(pane);
        self.editor_focused = true;
        self.terminal_focused = false;
    }

    /// Opens `file` in `pane`, as a preview or to stay.
    ///
    /// A preview takes the pane's one preview tab from whatever held it,
    /// which is what makes clicking down a tree leave one tab behind rather
    /// than twenty — and it is the pane's tab, not the window's, so a file
    /// previewed on the right leaves the pane on the left as it was.
    pub(super) fn show_file(&mut self, pane: PaneId, file: FileId, preview: bool) {
        if preview {
            self.close_previews(pane, file);
        }
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.open(file);
        }
        self.focus_pane(pane);
        self.sweep();
        self.store();
    }

    /// Closes whatever `pane` is previewing, other than `keep`.
    fn close_previews(&mut self, pane: PaneId, keep: FileId) {
        let previews = self
            .panes
            .pane(pane)
            .map(|pane| {
                pane.tabs()
                    .iter()
                    .copied()
                    .filter(|file| *file != keep && self.editor.is_preview(*file))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(pane) = self.panes.pane_mut(pane) {
            for file in previews {
                pane.close(file);
            }
        }
    }

    /// Divides `pane` that way, showing `file` in the pane that opens.
    ///
    /// Without a file of its own the new pane shows what the old one was
    /// showing, which is what splitting a pane is for: the same file, twice,
    /// at two places in it.
    pub(super) fn split_pane(
        &mut self,
        pane: PaneId,
        file: Option<FileId>,
        direction: SplitDirection,
    ) {
        let file = file.or_else(|| self.panes.pane(pane).and_then(panes::Pane::active));
        let Some(fresh) = self.panes.split(pane, direction) else {
            return;
        };
        if let (Some(file), Some(fresh)) = (file, self.panes.pane_mut(fresh)) {
            fresh.open(file);
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
        if let Some(file) = self.active_tab() {
            self.close_file(pane, file);
        }
    }

    /// Closes one tab, asking first when what is in it is not on disk.
    ///
    /// A file with changes nobody has written down is not something to close
    /// quietly: the reader is asked which of the two things they meant, and
    /// the file stays open until they say.
    pub(super) fn close_file(&mut self, pane: PaneId, file: FileId) {
        if self.editor.is_dirty(file) {
            return self.open_menu(crate::workspace::MenuTarget::Unsaved(pane, file));
        }
        self.close_tabs(pane, |pane| pane.close(file));
    }

    /// Writes the file down and then closes its tab.
    pub(super) fn save_and_close(&mut self, pane: PaneId, file: FileId) {
        if let Some(root) = self.worktree_of(file) {
            self.editor.save(file, &root);
        }
        self.close_tabs(pane, |pane| pane.close(file));
        self.go_on_closing();
    }

    /// Closes the tab, losing whatever was not written down.
    pub(super) fn discard_and_close(&mut self, pane: PaneId, file: FileId) {
        self.close_tabs(pane, |pane| pane.close(file));
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
    pub(super) fn close_saved_tabs(&mut self, pane: PaneId, keep: impl Fn(FileId) -> bool) {
        let dirty = |file: FileId| self.editor.is_dirty(file);
        let held = self
            .panes
            .pane(pane)
            .map(|pane| pane.tabs().to_vec())
            .unwrap_or_default();
        let kept = held
            .into_iter()
            .filter(|file| keep(*file) || dirty(*file))
            .collect::<Vec<_>>();
        self.close_tabs(pane, |pane| pane.retain(|file| kept.contains(&file)));
    }

    /// The tabs of `pane` on one side of `file`, and `file` itself.
    pub(super) fn tabs_from(&self, pane: PaneId, file: FileId, right: bool) -> Vec<FileId> {
        let tabs = self.tabs_of(pane);
        let Some(index) = tabs.iter().position(|held| *held == file) else {
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
        if let Some(file) = self.active_tab() {
            self.editor.edit(file, edit);
        }
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
    fn remember_closed(&mut self, before: &BTreeSet<FileId>) {
        let held = self.panes.held();
        let gone = before
            .iter()
            .filter(|file| !held.contains(file))
            .filter_map(|file| {
                let document = self.editor.get(*file)?;
                let document = document.borrow();
                Some(crate::app::places::Place {
                    project: self.editor.project_of(*file)?,
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
        self.panes.save(&|file| {
            let project = self.editor.project_of(file)?;
            let root = self.open.get(project)?.root().to_path_buf();
            let document = self.editor.get(file)?;
            let document = document.borrow();
            let head = document.buffer().selection().head;
            Some(SavedTab {
                project: root,
                path: document.buffer().path().to_path_buf(),
                preview: document.is_preview(),
                scroll: document.scroll(),
                line: head.line,
                column: head.column,
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
        self.panes = crate::panes::PaneTree::restored(saved, &mut |tab| {
            let (root, project) = roots
                .iter()
                .find(|(root, _)| *root == tab.project)
                .cloned()?;
            let file = editor.open(project, &root, &tab.path, tab.preview)?;
            if let Some(document) = editor.get(file) {
                let mut document = document.borrow_mut();
                document.restore(tab.line, tab.column, tab.scroll);
            }
            Some(file)
        });
        self.sweep();
    }

    /// Closes every file no pane is holding open any more.
    pub(super) fn sweep(&mut self) {
        let held: BTreeSet<FileId> = self.panes.held();
        self.editor.retain(&held);
    }

    /// Takes the files of `project` out of every pane that was showing them.
    pub(super) fn drop_project_tabs(&mut self, project: pm_core::ProjectId) {
        let leaving = self
            .panes
            .held()
            .into_iter()
            .filter(|file| self.editor.project_of(*file) == Some(project))
            .collect::<BTreeSet<_>>();
        self.panes.retain(|file| !leaving.contains(&file));
        self.panes.close_empty();
    }

    /// Carries a tab, or lets go of it where the pointer has reached.
    ///
    /// A press that goes nowhere before it is let go of is the click that
    /// selects the tab, which is what makes one gesture out of two: the tab
    /// is picked up by the same press that would have selected it, and only
    /// travel tells the two apart.
    pub(super) fn drag_tab(&mut self, pane: PaneId, file: FileId, event: ResizeEvent) {
        let travelled = (event.current.x - event.start.x).hypot(event.current.y - event.start.y);
        let order = |pane: PaneId| self.tabs_of(pane);
        let target = self.geometry.target_at(event.current, &order);
        let drag = TabDrag {
            from: pane,
            file,
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
            self.select_tab(drag.from, drag.file);
        }
    }

    /// Shows the tab that was clicked, keeping the file on a second click.
    pub(super) fn select_tab(&mut self, pane: PaneId, file: FileId) {
        if self.tab_clicks.press(file) >= 2 {
            self.editor.keep(file);
        }
        if let Some(pane) = self.panes.pane_mut(pane) {
            pane.activate(file);
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
        let landed = match place {
            DropPlace::Split(direction) => {
                self.split_pane(target, Some(drag.file), direction);
                self.panes.focus()
            }
            DropPlace::Into => {
                if let Some(pane) = self.panes.pane_mut(target) {
                    pane.open(drag.file);
                }
                target
            }
            DropPlace::Tab(index) => {
                if let Some(pane) = self.panes.pane_mut(target) {
                    pane.place(drag.file, index);
                }
                target
            }
        };

        if landed != drag.from
            && let Some(from) = self.panes.pane_mut(drag.from)
        {
            from.close(drag.file);
        }
        self.panes.close_empty();
        self.focus_pane(landed);
        self.sweep();
        self.store();
    }

    /// The files open in `pane`, in the order its tabs are drawn.
    pub(super) fn tabs_of(&self, pane: PaneId) -> Vec<FileId> {
        self.panes
            .pane(pane)
            .map(|pane| pane.tabs().to_vec())
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
        let name = self.editor.entry(drag.file)?.name;
        Some((drag.at, name))
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
            for file in self.tabs_of(pane) {
                drawn_tabs.push((pane, file));
            }
        }
        self.geometry.keep(&drawn, &drawn_tabs);
        for pane in &drawn {
            let tabs = self
                .tabs_of(*pane)
                .into_iter()
                .map(|file| self.geometry.tab(*pane, file))
                .collect::<Vec<_>>();
            let bar = self.geometry.bar(*pane);
            if tabs.is_empty() {
                bar.set(Rect::from_xywh(0.0, 0.0, 0.0, 0.0));
            }
            cells.push((self.geometry.pane(*pane), bar, tabs));
        }

        let files = &self.editor;
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
            Contents {
                tabs: pane
                    .tabs()
                    .iter()
                    .filter_map(|file| files.entry(*file))
                    .collect(),
                file: pane.active().and_then(|file| files.get(file)),
                bounds,
                bar,
                tab_bounds,
                link: link
                    .clone()
                    .filter(|(file, _)| pane.active() == Some(*file))
                    .map(|(_, span)| span),
                hovered: talked_about
                    .clone()
                    .filter(|(file, _)| pane.active() == Some(*file))
                    .map(|(_, span)| span),
                caret,
            }
        })
    }

    /// The menu open over the panes, and what it holds.
    pub(super) fn menu_items(&self) -> Option<(TabMenu, Vec<MenuItem<Message>>)> {
        let open = self.menu?;
        let items = match open.target {
            MenuTarget::File(pane, file) => {
                let pane = self.panes.pane(pane)?;
                let tabs = pane
                    .tabs()
                    .iter()
                    .filter_map(|file| self.editor.entry(*file))
                    .collect::<Vec<_>>();
                panes::file_menu(pane, &tabs, file)
            }
            MenuTarget::Pane(pane) => panes::pane_menu(pane, self.panes.is_split()),
            MenuTarget::Text(pane) => {
                let file = self.panes.pane(pane)?.active()?;
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
