//! What the window does with the file tree, and through it to the worktree.
//!
//! The tree is worked the way every editor's is: a click selects and opens,
//! the secondary modifier and shift mark rows alongside, the arrows walk it,
//! a name is typed in place, and what is selected is cut, copied, pasted,
//! carried into another directory or taken off the disk. Everything that
//! reaches the disk goes through [`pm_core::ops`], and the tree is read again
//! afterwards because what it lists is the disk.

use std::path::{Path, PathBuf};

use pm_core::{EntryId, Scope, ops};
use pm_gfx::Point;
use pm_ui::{ResizeEvent, ResizePhase, Scrolled};
use winit::keyboard::{Key, NamedKey};

use crate::app::App;
use crate::desktop;
use crate::field::Typed;
use crate::message::Message;
use crate::panes::{Item, PaneId, SplitDirection};
use crate::prompt::{Answer, Prompt};
use crate::tree::{Clipboard, Edit, EditKind, EntryDrag, Selection};
use crate::workspace::{MenuTarget, SidebarView};

/// How many rows a page key moves the tree's keyboard.
const TREE_PAGE: isize = 10;

/// What a move, a copy or a removal in the tree came to, away from the
/// window.
pub(super) enum Shifted {
    /// These were taken off the disk.
    Removed(Vec<PathBuf>),
    /// These were put in place.
    Placed {
        /// The folder they were put into, to be opened, where there was one.
        directory: Option<PathBuf>,
        /// Where each moved entry was, and where it is now.
        moved: Vec<(PathBuf, PathBuf)>,
        /// Every entry put in place, moved or copied.
        placed: Vec<PathBuf>,
    },
}

impl App {
    /// Carries out a command of the file tree, if `message` is one.
    pub(super) fn tree_command(&mut self, message: Message) -> bool {
        match message {
            Message::PressEntry(id, event) => self.press_entry(id, event),
            Message::PressTreeSpace => self.press_tree_space(),
            Message::ShowEntryMenu(id) => {
                self.aim_at_entry(id);
                self.open_menu(MenuTarget::Entry(id));
            }
            Message::ShowTreeMenu => {
                self.press_tree_space();
                self.open_menu(MenuTarget::Tree);
            }
            Message::NewTreeFile => self.start_tree_edit(EditKind::NewFile),
            Message::NewTreeFolder => self.start_tree_edit(EditKind::NewFolder),
            Message::RenameTreeEntry => self.start_tree_edit(EditKind::Rename),
            Message::PlaceTreeEdit(caret) => {
                self.tree_focused = true;
                if let Some(edit) = self.tree_edit.as_mut() {
                    edit.field_mut().place(caret);
                }
            }
            Message::TrashTreeEntries => self.ask_to_remove(true),
            Message::DeleteTreeEntries => self.ask_to_remove(false),
            Message::ConfirmTrash => self.remove_entries(true),
            Message::ConfirmDelete => self.remove_entries(false),
            Message::CutTreeEntries => self.clip_entries(true),
            Message::CopyTreeEntries => self.clip_entries(false),
            Message::PasteTreeEntries => self.paste_entries(),
            Message::DuplicateTreeEntries => self.duplicate_entries(),
            Message::OpenTreeEntriesToSide => self.open_entries_to_side(),
            Message::CopyTreePaths => self.copy_tree_paths(false),
            Message::CopyTreeRelativePaths => self.copy_tree_paths(true),
            Message::RevealTreeEntry => desktop::reveal(&self.pointed_path()),
            Message::OpenTreeEntryInTerminal => self.open_tree_terminal(),
            Message::CollapseTree => self.collapse_tree(),
            Message::RefreshTree => self.reread_worktree(),
            _ => return false,
        }
        self.request_redraw();
        true
    }

    /// Reads the worktree the window is pointed at again, and what git makes of it.
    ///
    /// Whatever the tree had selected that is no longer on the disk is let
    /// go of, so a command given next is not aimed at a file that is gone.
    pub(super) fn reread_worktree(&mut self) {
        if let Some(scope) = self.scope() {
            if let Some(tree) = self.files.get_mut(&scope) {
                tree.reload();
            }
            if let Some(selection) = self.selections.get_mut(&scope) {
                selection.retain(|path| std::fs::symlink_metadata(path).is_ok());
            }
        }
        self.reread_changes();
    }

    /// Reads the worktree again because the reader asked to, turning the
    /// refresh control of its review while it does.
    pub(super) fn refresh_changes(&mut self) {
        self.reread_worktree();
        if let Some(review) = self.scope().and_then(|scope| self.reviews.get_mut(&scope)) {
            review.start_refresh();
        }
        self.spun = std::time::Instant::now();
    }

    /// The paths of the tree's rows, in the order they are drawn.
    fn tree_order(&self) -> Vec<PathBuf> {
        self.scope()
            .and_then(|scope| self.files.get(&scope))
            .map(|tree| {
                tree.rows()
                    .into_iter()
                    .map(|row| row.entry.path().to_path_buf())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The worktree the tree is listing.
    fn tree_root(&self) -> Option<PathBuf> {
        self.files
            .get(&self.scope()?)
            .map(|tree| tree.root().to_path_buf())
    }

    /// The tree's selection, started empty if it has none yet.
    fn selection_mut(&mut self) -> Option<&mut Selection> {
        let scope = self.scope()?;
        Some(self.selections.entry(scope).or_default())
    }

    /// The tree's selection, if it has one.
    pub(super) fn selection(&self) -> Option<&Selection> {
        self.selections.get(&self.scope()?)
    }

    /// What a command given to the tree now acts on.
    fn tree_acting_on(&self) -> Vec<PathBuf> {
        let order = self.tree_order();
        self.selection()
            .map(|selection| selection.acting_on(&order))
            .unwrap_or_default()
    }

    /// The row the tree's keyboard is on, or the worktree when it is on none.
    fn pointed_path(&self) -> PathBuf {
        self.selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf)
            .or_else(|| self.tree_root())
            .unwrap_or_default()
    }

    /// The directory a new or pasted entry goes in: the row the keyboard is
    /// on when that is a directory, and the one holding it when it is not.
    fn pointed_directory(&self) -> PathBuf {
        let path = self.pointed_path();
        match path.is_dir() {
            true => path,
            false => path.parent().map(Path::to_path_buf).unwrap_or(path),
        }
    }

    /// Where the entry `id` names sits, and whether it holds others.
    fn entry_path(&self, id: EntryId) -> Option<(PathBuf, bool)> {
        let entry = self.files.get(&self.scope()?)?.entry(id)?;
        Some((entry.path().to_path_buf(), entry.is_directory()))
    }

    /// Gives the keyboard to the tree, taking it from everything else.
    fn focus_tree(&mut self) {
        self.release_pane_focus();
        self.tree_focused = true;
    }

    /// Answers a press, a drag or a release on the row `id` names.
    ///
    /// The press settles the selection, the way a file manager does it: a
    /// plain press selects the row alone, the secondary modifier adds it or
    /// takes it out, and shift sweeps from the anchor to it. A plain press on
    /// a row already part of a larger selection leaves the selection for the
    /// release, so all of it can be carried. The release of a press that went
    /// nowhere opens a file or opens and closes a directory; one that
    /// travelled lets go of what it carried.
    fn press_entry(&mut self, id: EntryId, event: ResizeEvent) {
        match event.phase {
            ResizePhase::Started => self.pick_up_entry(id, event.current),
            ResizePhase::Moved => self.carry_entries(event),
            ResizePhase::Ended => {
                self.carry_entries(event);
                if let Some(drag) = self.entry_drag.take() {
                    self.let_go_of_entries(id, drag);
                }
            }
        }
    }

    /// Settles the selection for a press on the row `id` names.
    fn pick_up_entry(&mut self, id: EntryId, at: Point) {
        self.commit_tree_edit();
        self.focus_tree();
        let Some((path, _)) = self.entry_path(id) else {
            return;
        };
        let order = self.tree_order();
        let (marking, sweeping) = (self.primary_held(), self.modifiers.shift_key());
        let Some(selection) = self.selection_mut() else {
            return;
        };
        let plain = !marking && !sweeping;
        let deferred = plain && selection.is_selected(&path) && selection.count() > 1;

        match (sweeping, marking) {
            (true, _) => selection.extend_to(&path, &order),
            (false, true) => selection.toggle(&path),
            (false, false) if deferred => selection.place(&path),
            (false, false) => selection.select(&path),
        }
        let paths = match selection.is_selected(&path) {
            true => selection.acting_on(&order),
            false => vec![path.clone()],
        };

        self.entry_drag = Some(EntryDrag {
            pressed: path,
            paths,
            at,
            travelled: 0.0,
            target: None,
            pane: None,
            deferred,
            plain,
        });
    }

    /// Follows the pointer carrying rows, working out where they would land.
    ///
    /// A directory among the rows carried, or under one of them, is no place
    /// to put them, so it is not offered as one.
    fn carry_entries(&mut self, event: ResizeEvent) {
        let carried = self
            .entry_drag
            .as_ref()
            .map(|drag| drag.paths.clone())
            .unwrap_or_default();
        let over_tree = self.tree_showing() && self.tree_area.get().contains(event.current);
        let target = self
            .directory_under(event.current)
            .filter(|target| !carried.iter().any(|path| target.starts_with(path)));
        let pane = match over_tree {
            true => None,
            false => self.geometry.pane_at(event.current),
        };
        if let Some(drag) = self.entry_drag.as_mut() {
            drag.at = event.current;
            drag.travelled = drag
                .travelled
                .max((event.current.x - event.start.x).hypot(event.current.y - event.start.y));
            drag.target = target;
            drag.pane = pane;
        }
    }

    /// Ends a press on a row: a click when it went nowhere, a drop when not.
    fn let_go_of_entries(&mut self, id: EntryId, drag: EntryDrag) {
        if drag.is_carried() {
            return self.drop_entries(drag);
        }
        if drag.deferred
            && let Some(selection) = self.selection_mut()
        {
            selection.select(&drag.pressed);
        }
        if !drag.plain {
            return;
        }
        match drag.pressed.is_dir() {
            true => {
                if let Some(tree) = self.scope().and_then(|scope| self.files.get_mut(&scope)) {
                    tree.toggle(id);
                }
            }
            false => {
                let preview = self.tree_clicks.press(id) < 2;
                self.open_tree_file(&drag.pressed, self.panes.focus(), preview);
                if preview {
                    self.focus_tree();
                }
            }
        }
    }

    /// Lets go of carried rows: into a directory, or open in a pane.
    ///
    /// Into a directory they move, or are copied there when the modifier
    /// that copies is held as they are let go of. Over a pane, the files
    /// among them open there to stay.
    fn drop_entries(&mut self, drag: EntryDrag) {
        if let Some(directory) = drag.target {
            let copying = self.copy_modifier_held();
            return self.move_entries(&drag.paths, &directory, copying);
        }
        if let Some(pane) = drag.pane {
            for path in drag.paths.iter().filter(|path| path.is_file()) {
                self.open_tree_file(path, pane, false);
            }
        }
    }

    /// The directory rows let go of at `point` land in, if it is over the tree.
    ///
    /// A directory's row is itself; a file's row is the directory holding it,
    /// and the space below the last row is the worktree.
    pub(super) fn directory_under(&self, point: Point) -> Option<PathBuf> {
        if !self.tree_showing() || !self.tree_area.get().contains(point) {
            return None;
        }
        let tree = self.files.get(&self.scope()?)?;
        let rows = tree.rows();
        let top = self.tree_rows.get().top();
        let place = ((point.y - top) / self.theme().size.row).floor();
        let row = (place >= 0.0).then(|| rows.get(place as usize)).flatten();
        Some(match row {
            Some(row) if row.entry.is_directory() => row.entry.path().to_path_buf(),
            Some(row) => row
                .entry
                .path()
                .parent()
                .map_or_else(|| tree.root().to_path_buf(), Path::to_path_buf),
            None => tree.root().to_path_buf(),
        })
    }

    /// Whether the sidebar beside the panes is showing the tree.
    pub(super) fn tree_showing(&self) -> bool {
        self.secondary_sidebar_open && self.secondary_sidebar_view == SidebarView::Files
    }

    /// Lets go of the selection, for a press on the space below the rows.
    fn press_tree_space(&mut self) {
        self.commit_tree_edit();
        self.focus_tree();
        if let Some(selection) = self.selection_mut() {
            *selection = Selection::default();
        }
    }

    /// Points the selection at the row `id` names, for a menu opened on it.
    ///
    /// A row that is not part of what is selected takes the selection over;
    /// one that is leaves it, so the menu is about everything selected.
    fn aim_at_entry(&mut self, id: EntryId) {
        self.commit_tree_edit();
        self.focus_tree();
        let Some((path, _)) = self.entry_path(id) else {
            return;
        };
        if let Some(selection) = self.selection_mut() {
            match selection.is_selected(&path) {
                true => selection.place(&path),
                false => selection.select(&path),
            }
        }
    }

    /// The menu of what can be done to what the tree is acting on.
    pub(super) fn entry_menu(&self, id: EntryId) -> Vec<pm_ui::MenuItem<Message>> {
        let directory = self.entry_path(id).is_some_and(|(_, directory)| directory);
        crate::tree::entry_menu(
            self.tree_acting_on().len().max(1),
            directory,
            self.tree_clipboard.is_some(),
        )
    }

    /// The menu of the tree itself.
    pub(super) fn tree_menu(&self) -> Vec<pm_ui::MenuItem<Message>> {
        crate::tree::empty_menu(self.tree_clipboard.is_some())
    }

    /// Starts typing a name into the tree, for `kind`.
    ///
    /// A new entry is typed at the top of the directory it goes in, which is
    /// opened for it; a rename is typed over the row's own name.
    fn start_tree_edit(&mut self, kind: EditKind) {
        self.commit_tree_edit();
        let Some(scope) = self.scope() else {
            return;
        };
        let edit = match kind {
            EditKind::Rename => {
                let Some(cursor) = self.selection().and_then(Selection::cursor) else {
                    return;
                };
                Edit::renaming(cursor)
            }
            kind => Edit::creating(kind, &self.pointed_directory()),
        };
        if let Some(tree) = self.files.get_mut(&scope) {
            tree.reveal(&edit.at().join("_"));
            if kind != EditKind::Rename {
                tree.expand(edit.at());
            }
        }
        self.secondary_sidebar_open = true;
        self.secondary_sidebar_view = SidebarView::Files;
        let revealed = match kind {
            EditKind::Rename => edit.at().to_path_buf(),
            _ => edit.at().join("_"),
        };
        self.tree_edit = Some(edit);
        self.focus_tree();
        self.scroll_tree_to(&revealed);
    }

    /// Takes the name being typed into the tree, if it can be taken.
    ///
    /// A name that is wrong, or that would change nothing, puts the field
    /// away without touching the disk: pressing elsewhere is how a reader
    /// gives up on a name as often as it is how they finish one.
    pub(super) fn commit_tree_edit(&mut self) {
        let Some(edit) = self.tree_edit.take() else {
            return;
        };
        if edit.is_unchanged() || edit.problem().is_some() {
            return;
        }
        let target = edit.target();
        let done = match edit.kind() {
            EditKind::NewFile => ops::create_file(&target),
            EditKind::NewFolder => ops::create_dir(&target),
            EditKind::Rename => ops::rename(edit.at(), &target),
        };
        if done.is_err() {
            return;
        }
        if edit.kind() == EditKind::Rename {
            self.retarget_tabs(edit.at(), &target);
        }
        self.reread_worktree();
        if let Some(tree) = self.scope().and_then(|scope| self.files.get_mut(&scope)) {
            tree.reveal(&target);
        }
        if let Some(selection) = self.selection_mut() {
            selection.select(&target);
        }
        self.scroll_tree_to(&target);
        if edit.kind() == EditKind::NewFile {
            self.open_tree_file(&target, self.panes.focus(), false);
        }
    }

    /// Puts the name being typed into the tree away without taking it.
    fn cancel_tree_edit(&mut self) -> bool {
        self.tree_edit.take().is_some()
    }

    /// Whether a press at `point` lands on the name being typed.
    pub(super) fn presses_tree_edit(&self, point: Point) -> bool {
        self.tree_edit.is_some() && self.tree_field.get().contains(point)
    }

    /// Asks whether what the tree is acting on should go, and how.
    fn ask_to_remove(&mut self, trashing: bool) {
        let acting = self.tree_acting_on();
        if acting.is_empty() {
            return;
        }
        let names = acting
            .iter()
            .map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let asked = match (acting.len(), trashing) {
            (1, true) => format!("Are you sure you want to delete '{}'?", names[0]),
            (1, false) => format!(
                "Are you sure you want to permanently delete '{}'?",
                names[0]
            ),
            (count, true) => format!("Are you sure you want to delete these {count} items?"),
            (count, false) => {
                format!("Are you sure you want to permanently delete these {count} items?")
            }
        };
        let (detail, answer) = match trashing {
            true => (
                "You can restore them from the Trash.",
                Answer::new("Move to Trash", Message::ConfirmTrash),
            ),
            false => (
                "This cannot be undone.",
                Answer::new("Delete", Message::ConfirmDelete),
            ),
        };
        let mut shown = names.into_iter().take(10).collect::<Vec<_>>();
        shown.push(detail.to_owned());

        self.removing = acting;
        self.ask_first(Prompt::asking(asked, shown, vec![answer, Answer::cancel()]));
    }

    /// Takes off the disk what the question was asked about, away from the
    /// window, since a folder of build output is a long time deleting.
    fn remove_entries(&mut self, trashing: bool) {
        let removing = std::mem::take(&mut self.removing);
        self.shift_later(move || {
            let removed = removing
                .into_iter()
                .filter(|path| match trashing {
                    true => ops::trash(path).is_ok(),
                    false => ops::remove(path).is_ok(),
                })
                .collect();
            Shifted::Removed(removed)
        });
        self.focus_tree();
    }

    /// Runs `shift` on a thread of its own, and wakes the window with what
    /// it moved, copied or removed.
    fn shift_later(&self, shift: impl FnOnce() -> Shifted + Send + 'static) {
        let shifted = self.shifted.clone();
        let wake = self.waker(crate::app::Wake::Shifted);
        std::thread::spawn(move || {
            let done = shift();
            if let Ok(mut shifted) = shifted.lock() {
                shifted.push(done);
            }
            wake();
        });
    }

    /// Takes in every move, copy and removal that has finished, answering
    /// whether any had.
    pub(super) fn take_shifted(&mut self) -> bool {
        let shifted = self
            .shifted
            .lock()
            .map(|mut shifted| std::mem::take(&mut *shifted))
            .unwrap_or_default();
        let any = !shifted.is_empty();
        for done in shifted {
            match done {
                Shifted::Removed(removed) => {
                    for path in &removed {
                        self.close_tabs_of(path);
                    }
                    self.reread_worktree();
                }
                Shifted::Placed {
                    directory,
                    moved,
                    placed,
                } => {
                    for (from, to) in &moved {
                        self.retarget_tabs(from, to);
                    }
                    if let Some(directory) = directory
                        && let Some(tree) =
                            self.scope().and_then(|scope| self.files.get_mut(&scope))
                    {
                        tree.expand(&directory);
                    }
                    self.settle_on(&placed);
                }
            }
        }
        any
    }

    /// Puts what the tree is acting on on its clipboard, to move or to copy.
    fn clip_entries(&mut self, cut: bool) {
        let paths = self.tree_acting_on();
        if paths.is_empty() {
            return;
        }
        self.tree_clipboard = Some(Clipboard { paths, cut });
    }

    /// Copies or moves what is on the tree's clipboard where the tree points.
    ///
    /// A cut is pasted once: the files have moved, so the clipboard has
    /// nothing left to say about where they were.
    fn paste_entries(&mut self) {
        let Some(clipboard) = self.tree_clipboard.clone() else {
            return;
        };
        let directory = self.pointed_directory();
        self.move_entries(&clipboard.paths, &directory, !clipboard.cut);
        if clipboard.cut {
            self.tree_clipboard = None;
        }
    }

    /// Copies what the tree is acting on beside itself.
    fn duplicate_entries(&mut self) {
        let acting = self.tree_acting_on();
        self.shift_later(move || Shifted::Placed {
            directory: None,
            moved: Vec::new(),
            placed: acting
                .iter()
                .filter_map(|path| ops::copy_into(path, path.parent()?).ok())
                .collect(),
        });
    }

    /// Moves `paths` into `directory`, or copies them there, away from the
    /// window.
    pub(super) fn move_entries(&mut self, paths: &[PathBuf], directory: &Path, copying: bool) {
        let paths = paths.to_vec();
        let directory = directory.to_path_buf();
        self.shift_later(move || {
            let mut moved = Vec::new();
            let mut placed = Vec::new();
            for path in paths {
                let done = match copying {
                    true => ops::copy_into(&path, &directory),
                    false => ops::move_into(&path, &directory),
                };
                if let Ok(to) = done {
                    if !copying && to != path {
                        moved.push((path, to.clone()));
                    }
                    placed.push(to);
                }
            }
            Shifted::Placed {
                directory: Some(directory),
                moved,
                placed,
            }
        });
    }

    /// Reads the tree again and selects `placed`, what was just put there.
    fn settle_on(&mut self, placed: &[PathBuf]) {
        self.reread_worktree();
        if let Some(tree) = self.scope().and_then(|scope| self.files.get_mut(&scope)) {
            for path in placed {
                tree.reveal(path);
            }
        }
        let order = self.tree_order();
        if let Some(selection) = self.selection_mut() {
            selection.clear();
            for (place, path) in placed.iter().enumerate() {
                match place {
                    0 => selection.select(path),
                    _ => selection.toggle(path),
                }
            }
            if let Some(first) = placed.first() {
                selection.place(first);
            }
            selection.retain(|path| order.iter().any(|row| row == path));
        }
        if let Some(first) = placed.first() {
            self.scroll_tree_to(first);
        }
        self.focus_tree();
    }

    /// Opens every file the tree is acting on in a pane of its own, to the side.
    fn open_entries_to_side(&mut self) {
        let files = self
            .tree_acting_on()
            .into_iter()
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        let Some(first) = files.first() else {
            return;
        };
        let Some(item) = self.item_for(first) else {
            return;
        };
        let pane = self.panes.focus();
        self.split_pane(pane, Some(item), SplitDirection::Right);
        let beside = self.panes.focus();
        for path in files.iter().skip(1) {
            self.open_tree_file(path, beside, false);
        }
        self.sweep();
    }

    /// The tab that would show `path`, opening the file behind it if need be.
    fn item_for(&mut self, path: &Path) -> Option<Item> {
        let (scope, root) = self.worktree_holding(path)?;
        if crate::image::Images::is_picture(path) {
            return Some(Item::Image(self.images.open(scope, path, false)));
        }
        self.editor.open(scope, &root, path, false).map(Item::File)
    }

    /// Opens the file at `path` in `pane`, as a preview or to stay.
    ///
    /// Reaching another file this way is a jump like any other, so where the
    /// pane was is taken down on the trail and going back returns to it.
    fn open_tree_file(&mut self, path: &Path, pane: PaneId, preview: bool) {
        let Some((scope, root)) = self.worktree_holding(path) else {
            return;
        };
        if self.open_picture(pane, scope, path, preview) {
            return;
        }
        if let Some(file) = self.editor.open(scope, &root, path, preview) {
            if self.active_tab() != Some(Item::File(file))
                && let Some(from) = self.here()
            {
                self.trail.jumped(from);
            }
            self.show_file(pane, file, preview);
        }
    }

    /// Opens the file at the row the tree's keyboard is on, or opens and
    /// closes the directory there.
    fn open_tree_cursor(&mut self, preview: bool) {
        let Some(cursor) = self
            .selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf)
        else {
            return;
        };
        match cursor.is_dir() {
            true => {
                if let Some(tree) = self.scope().and_then(|scope| self.files.get_mut(&scope)) {
                    match tree.is_expanded(&cursor) {
                        true => tree.collapse(&cursor),
                        false => tree.expand(&cursor),
                    }
                }
            }
            false => {
                self.open_tree_file(&cursor, self.panes.focus(), preview);
                if preview {
                    self.focus_tree();
                }
            }
        }
    }

    /// Moves every tab showing a file at or under `from` to where it went.
    ///
    /// A file with changes that are not on disk keeps its tab where it is,
    /// so nothing typed into it is lost to a rename.
    pub(super) fn retarget_tabs(&mut self, from: &Path, to: &Path) {
        let moving = self
            .panes
            .held()
            .into_iter()
            .filter_map(|item| {
                let file = item.file()?;
                let path = self.editor.path(file)?;
                (path.starts_with(from) && !self.editor.is_dirty(file)).then_some((item, path))
            })
            .collect::<Vec<_>>();
        if moving.is_empty() {
            return;
        }
        let items = moving.iter().map(|(item, _)| *item).collect::<Vec<_>>();
        self.panes.retain(|item| !items.contains(&item));
        self.panes.close_empty();
        self.sweep();
        for (_, path) in moving {
            let moved = to.join(path.strip_prefix(from).unwrap_or(Path::new("")));
            self.open_tree_file(&moved, self.panes.focus(), false);
        }
        self.store();
    }

    /// Closes every tab showing a file at or under `path`, having nothing
    /// unsaved in it.
    pub(super) fn close_tabs_of(&mut self, path: &Path) {
        let gone = self
            .panes
            .held()
            .into_iter()
            .filter(|item| {
                item.file().is_some_and(|file| {
                    !self.editor.is_dirty(file)
                        && self
                            .editor
                            .path(file)
                            .is_some_and(|open| open.starts_with(path))
                })
            })
            .collect::<Vec<_>>();
        if gone.is_empty() {
            return;
        }
        self.panes.retain(|item| !gone.contains(&item));
        self.panes.close_empty();
        self.sweep();
        self.store();
    }

    /// Starts a shell in the directory the tree is pointed at.
    fn open_tree_terminal(&mut self) {
        let directory = self.pointed_directory();
        let Some((scope, _)) = self.worktree_holding(&directory) else {
            return;
        };
        let env = self.worktree_env(scope);
        self.terminals.start(scope, &directory, &env);
        self.show_panel(crate::panel::PanelView::Terminal);
        self.terminal_focused = true;
        self.tree_focused = false;
        self.editor_focused = false;
    }

    /// Puts the paths of what the tree is acting on on the clipboard, a line each.
    fn copy_tree_paths(&mut self, relative: bool) {
        let root = self.tree_root();
        let written = self
            .tree_acting_on()
            .iter()
            .map(|path| match (&root, relative) {
                (Some(root), true) => path
                    .strip_prefix(root)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
                _ => path.display().to_string(),
            })
            .collect::<Vec<_>>();
        if !written.is_empty() {
            desktop::copy(written.join("\n"));
        }
    }

    /// Closes every directory of the tree, and scrolls it back to the top.
    fn collapse_tree(&mut self) {
        if let Some(scope) = self.scope() {
            if let Some(tree) = self.files.get_mut(&scope) {
                tree.collapse_all();
            }
            if let Some(selection) = self.selections.get_mut(&scope) {
                let root = self.files.get(&scope).map(|tree| tree.root().to_path_buf());
                selection.retain(|path| {
                    root.as_deref()
                        .is_some_and(|root| path.parent() == Some(root))
                });
            }
        }
        let scroll = self.tree_scroll();
        let mut moved = scroll.get();
        moved.by(f32::MAX / 2.0);
        scroll.set(moved);
    }

    /// Shows the file the focused pane holds in the tree, and gives it the keyboard.
    pub(super) fn reveal_in_tree(&mut self) {
        self.secondary_sidebar_open = true;
        self.secondary_sidebar_view = SidebarView::Files;
        let shown = self
            .active_file_id()
            .and_then(|file| self.editor.path(file));
        if let Some(path) = shown {
            if let Some(tree) = self.scope().and_then(|scope| self.files.get_mut(&scope)) {
                tree.reveal(&path);
            }
            if let Some(selection) = self.selection_mut() {
                selection.select(&path);
            }
            self.scroll_tree_to(&path);
        }
        self.focus_tree();
        self.store();
    }

    /// The scroll of the tree the window is pointed at.
    pub(super) fn tree_scroll(&mut self) -> Scrolled {
        match self.scope() {
            Some(scope) => self.tree_scrolls.entry(scope).or_default().clone(),
            None => Scrolled::default(),
        }
    }

    /// Scrolls the tree by `delta` when the pointer is over it, saying whether it was.
    pub(super) fn scroll_tree(&mut self, delta: f32) -> bool {
        let over = self
            .pointer
            .is_some_and(|pointer| self.tree_area.get().contains(pointer));
        if !self.tree_showing() || !over {
            return false;
        }
        let scroll = self.tree_scroll();
        let mut moved = scroll.get();
        moved.by(delta);
        scroll.set(moved);
        true
    }

    /// Scrolls the tree as little as it takes to show the row at `path`.
    fn scroll_tree_to(&mut self, path: &Path) {
        let order = self.tree_order();
        let place = order
            .iter()
            .position(|row| row == path)
            .or_else(|| {
                let parent = path.parent()?;
                order.iter().position(|row| row == parent).map(|at| at + 1)
            })
            .unwrap_or(0);
        let row = self.theme().size.row;
        let height = self.tree_area.get().size.height;
        let scroll = self.tree_scroll();
        let mut moved = scroll.get();
        let offset = -moved.origin().y;
        let top = place as f32 * row;
        if top < offset {
            moved.by(offset - top);
        } else if height > 0.0 && top + row > offset + height {
            moved.by(offset + height - top - row);
        }
        scroll.set(moved);
    }

    /// Whether the modifier that marks rows alongside is held.
    fn primary_held(&self) -> bool {
        self.modifiers.control_key() || self.modifiers.super_key()
    }

    /// Whether the modifier that copies rather than moves a drop is held.
    fn copy_modifier_held(&self) -> bool {
        match cfg!(target_os = "macos") {
            true => self.modifiers.alt_key(),
            false => self.modifiers.control_key(),
        }
    }

    /// Sends a keypress to the name being typed into the tree, if one is.
    ///
    /// Enter takes the name and Escape gives it up; every other key is the
    /// field's, apart from the window's own chords.
    pub(super) fn send_to_tree_edit(&mut self, key: &Key<&str>) -> bool {
        if self.tree_edit.is_none() || self.is_window_chord_over_text(key) {
            return false;
        }
        let modifiers = self.modifiers;
        match key {
            Key::Named(NamedKey::Enter) => {
                if self
                    .tree_edit
                    .as_ref()
                    .is_some_and(|edit| edit.is_unchanged() || edit.problem().is_none())
                {
                    self.commit_tree_edit();
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.cancel_tree_edit();
            }
            key => {
                if let Some(edit) = self.tree_edit.as_mut()
                    && edit.field_mut().press(key, modifiers) == Typed::Ignored
                {
                    return !self.primary_held();
                }
            }
        }
        true
    }

    /// Sends a keypress to the tree, when the tree has the keyboard.
    ///
    /// The keys are the ones every file tree answers: the arrows walk it and
    /// shift with them sweeps, left and right close and open directories,
    /// Enter opens, F2 renames, Delete takes to the trash and shift with it
    /// deletes for good, the clipboard keys cut, copy and paste files, and a
    /// letter jumps to the next row whose name starts with it.
    pub(super) fn send_to_tree(&mut self, key: &Key<&str>) -> bool {
        if !self.tree_focused
            || !self.tree_showing()
            || self.is_window_chord()
            || self.tree_edit.is_some()
        {
            return false;
        }
        let (primary, shift) = (self.primary_held(), self.modifiers.shift_key());
        match key {
            Key::Named(NamedKey::ArrowUp) => self.step_tree(-1, shift),
            Key::Named(NamedKey::ArrowDown) => self.step_tree(1, shift),
            Key::Named(NamedKey::PageUp) => self.step_tree(-TREE_PAGE, shift),
            Key::Named(NamedKey::PageDown) => self.step_tree(TREE_PAGE, shift),
            Key::Named(NamedKey::Home) => self.step_tree(isize::MIN / 2, shift),
            Key::Named(NamedKey::End) => self.step_tree(isize::MAX / 2, shift),
            Key::Named(NamedKey::ArrowLeft) => self.tree_left(),
            Key::Named(NamedKey::ArrowRight) => self.tree_right(),
            Key::Named(NamedKey::Enter) if primary => self.open_entries_to_side(),
            Key::Named(NamedKey::Enter) => self.open_tree_cursor(false),
            Key::Named(NamedKey::Space) => self.open_tree_cursor(true),
            Key::Named(NamedKey::F2) => self.start_tree_edit(EditKind::Rename),
            Key::Named(NamedKey::Delete) => self.ask_to_remove(!shift),
            Key::Named(NamedKey::Backspace) if primary => self.ask_to_remove(true),
            Key::Named(NamedKey::Escape) => self.escape_tree(),
            Key::Character(typed) if primary => match typed.to_ascii_lowercase().as_str() {
                "c" => self.clip_entries(false),
                "x" => self.clip_entries(true),
                "v" => self.paste_entries(),
                "a" => {
                    let order = self.tree_order();
                    if let Some(selection) = self.selection_mut() {
                        selection.select_all(&order);
                    }
                }
                _ => return false,
            },
            Key::Character(typed) if !self.modifiers.alt_key() => self.jump_tree_to(typed),
            _ => return false,
        }
        true
    }

    /// Moves the tree's keyboard `steps` rows, sweeping if asked.
    fn step_tree(&mut self, steps: isize, sweeping: bool) {
        let order = self.tree_order();
        if let Some(selection) = self.selection_mut() {
            selection.step(&order, steps, sweeping);
        }
        self.follow_tree_cursor();
    }

    /// Scrolls the tree to wherever its keyboard is.
    fn follow_tree_cursor(&mut self) {
        if let Some(cursor) = self
            .selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf)
        {
            self.scroll_tree_to(&cursor);
        }
    }

    /// Closes the directory the keyboard is on, or goes up to the one holding it.
    fn tree_left(&mut self) {
        let Some(cursor) = self
            .selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf)
        else {
            return self.step_tree(0, false);
        };
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(tree) = self.files.get_mut(&scope) else {
            return;
        };
        if tree.is_expanded(&cursor) {
            return tree.collapse(&cursor);
        }
        let root = tree.root().to_path_buf();
        if let Some(parent) = cursor.parent().filter(|parent| *parent != root) {
            let parent = parent.to_path_buf();
            if let Some(selection) = self.selection_mut() {
                selection.select(&parent);
            }
            self.scroll_tree_to(&parent);
        }
    }

    /// Opens the directory the keyboard is on, or goes down into it.
    fn tree_right(&mut self) {
        let Some(cursor) = self
            .selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf)
        else {
            return self.step_tree(0, false);
        };
        if !cursor.is_dir() {
            return;
        }
        let Some(tree) = self.scope().and_then(|scope| self.files.get_mut(&scope)) else {
            return;
        };
        if !tree.is_expanded(&cursor) {
            return tree.expand(&cursor);
        }
        let order = self.tree_order();
        let inside = order
            .iter()
            .skip_while(|row| **row != cursor)
            .nth(1)
            .filter(|row| row.parent() == Some(cursor.as_path()))
            .cloned();
        if let Some(inside) = inside {
            if let Some(selection) = self.selection_mut() {
                selection.select(&inside);
            }
            self.scroll_tree_to(&inside);
        }
    }

    /// Narrows the selection to the keyboard's row, or lets go of the tree.
    fn escape_tree(&mut self) {
        if self
            .tree_clipboard
            .as_ref()
            .is_some_and(|clipboard| clipboard.cut)
        {
            self.tree_clipboard = None;
            return;
        }
        let cursor = self
            .selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf);
        let many = self
            .selection()
            .is_some_and(|selection| selection.count() > 1);
        match (many, cursor) {
            (true, Some(cursor)) => {
                if let Some(selection) = self.selection_mut() {
                    selection.select(&cursor);
                }
            }
            _ => self.tree_focused = false,
        }
    }

    /// Moves the keyboard to the next row whose name starts with `typed`.
    fn jump_tree_to(&mut self, typed: &str) {
        let typed = typed.to_lowercase();
        let order = self.tree_order();
        let cursor = self
            .selection()
            .and_then(Selection::cursor)
            .map(Path::to_path_buf);
        let start = cursor
            .and_then(|cursor| order.iter().position(|row| *row == cursor))
            .map_or(0, |at| at + 1);
        let named = |row: &&PathBuf| {
            row.file_name()
                .is_some_and(|name| name.to_string_lossy().to_lowercase().starts_with(&typed))
        };
        let found = order
            .iter()
            .skip(start)
            .find(named)
            .or_else(|| order.iter().take(start).find(named))
            .cloned();
        if let Some(found) = found {
            if let Some(selection) = self.selection_mut() {
                selection.select(&found);
            }
            self.scroll_tree_to(&found);
        }
    }

    /// What the pointer is carrying out of the tree, and where it is.
    pub(super) fn carried_entries(&self) -> Option<(Point, String)> {
        let drag = self.entry_drag.as_ref().filter(|drag| drag.is_carried())?;
        let label = match drag.paths.as_slice() {
            [only] => only
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            many => format!("{} items", many.len()),
        };
        Some((drag.at, label))
    }

    /// The pane rows carried out of the tree would open in, for its wash.
    pub(super) fn entry_drop_pane(&self) -> Option<PaneId> {
        self.entry_drag
            .as_ref()
            .filter(|drag| drag.is_carried() && drag.target.is_none())
            .and_then(|drag| drag.pane)
    }

    /// The worktree the window is holding that `path` lives in.
    fn worktree_holding(&self, path: &Path) -> Option<(Scope, PathBuf)> {
        self.worktrees()
            .into_iter()
            .filter(|(_, root)| path.starts_with(root))
            .max_by_key(|(_, root)| root.as_os_str().len())
    }
}
