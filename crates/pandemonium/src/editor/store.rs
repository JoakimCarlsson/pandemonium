//! The files the window has open, whichever pane is showing them.
//!
//! A file is opened once per project: the same path in two projects is two
//! documents, because it is two worktrees. Which pane shows which of them is
//! the pane tree's business — this is only where the documents live, and the
//! one seam a file is opened, edited, saved and closed through, so the
//! language server hears about every change exactly once.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use pm_core::ProjectId;
use pm_text::{Buffer, Client, Servers};

/// One open file, shared between the window and the pane drawing it.
///
/// The element tree is rebuilt every frame and may not borrow the window's
/// state, so the pane holds the document itself rather than a reference to
/// where the window keeps it.
pub type OpenFile = Rc<RefCell<Document>>;

/// An open file's identity for as long as it is open.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileId(u64);

/// One open file as a bar of tabs presents it.
pub struct FileEntry {
    /// Which file this tab is.
    pub id: FileId,
    /// What the tab calls it: the file's own name.
    pub name: String,
    /// Whether it has changes that are not on disk.
    pub dirty: bool,
    /// Whether it is only being previewed, and will give its tab up.
    pub preview: bool,
}

/// One open file: its buffer, where the pane is looking, and who serves it.
pub struct Document {
    /// The text and everything the editor knows about it.
    buffer: Buffer,
    /// The first line the pane shows.
    scroll: usize,
    /// How many lines the pane last had room for.
    rows: usize,
    /// Whether the file is only being looked at, not kept open.
    ///
    /// A previewed file holds the one preview tab of the pane it was opened
    /// in and gives it up to the next file previewed there. Editing it, or
    /// asking for it a second time, is what keeps it.
    preview: bool,
    /// The language server this file is open in, when it has one.
    server: Option<Arc<Client>>,
}

impl Document {
    /// Opens `buffer`, telling `server` that it is open.
    fn new(buffer: Buffer, preview: bool, server: Option<Arc<Client>>) -> Self {
        if let Some(server) = server.as_ref() {
            server.did_open(buffer.path(), buffer.version(), &buffer.contents());
        }

        Self {
            buffer,
            scroll: 0,
            rows: 0,
            preview,
            server,
        }
    }

    /// The text and everything the editor knows about it.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// The buffer, to read a highlight or a line out of while painting.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// The first line the pane shows.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Shows the file from `line` down, as far as there is file to show.
    pub fn scroll_to(&mut self, line: usize) {
        let last = self.buffer.line_count().saturating_sub(1);
        self.scroll = line.min(last);
    }

    /// Scrolls `lines` down, or up when `lines` is negative.
    pub fn scroll_by(&mut self, lines: isize) {
        self.scroll_to(self.scroll.saturating_add_signed(lines));
    }

    /// How many lines the pane last had room for.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Takes down how many lines the pane has room for, and follows the cursor.
    ///
    /// Only the pane knows how tall a line came out, so this is where the
    /// view told from: a keypress moves the cursor without knowing whether
    /// the place it moved to is on screen, and the next frame brings it back.
    pub fn set_rows(&mut self, rows: usize) {
        self.rows = rows;
        let head = self.buffer.selection().head.line;
        if head < self.scroll {
            self.scroll = head;
        } else if rows > 0 && head >= self.scroll + rows {
            self.scroll = head + 1 - rows;
        }
    }

    /// Whether the file is only being looked at, not kept open.
    pub fn is_preview(&self) -> bool {
        self.preview
    }

    /// Keeps the file open, whether it was being previewed or not.
    pub fn keep(&mut self) {
        self.preview = false;
    }

    /// Applies `edit` to the buffer and tells the server what it now holds.
    ///
    /// A keypress that only moves the cursor is an edit as far as the pane
    /// is concerned and none at all as far as the server is: the version is
    /// what says which of the two happened. A keypress that does change the
    /// text also keeps the file: what has been written in is not something
    /// the next file previewed may close.
    pub fn edit(&mut self, edit: impl FnOnce(&mut Buffer)) {
        let version = self.buffer.version();
        edit(&mut self.buffer);
        if version == self.buffer.version() {
            return;
        }
        self.preview = false;
        if let Some(server) = self.server.as_ref() {
            server.did_change(
                self.buffer.path(),
                self.buffer.version(),
                &self.buffer.contents(),
            );
        }
    }

    /// Writes the file to disk and tells the server it was written.
    pub fn save(&mut self) {
        if self.buffer.save().is_err() {
            return;
        }
        if let Some(server) = self.server.as_ref() {
            server.did_save(self.buffer.path(), &self.buffer.contents());
        }
    }

    /// Takes in what the server has last said about this file.
    fn refresh(&mut self) {
        if let Some(server) = self.server.as_ref() {
            self.buffer
                .set_diagnostics(server.diagnostics(self.buffer.path()));
        }
    }
}

impl Drop for Document {
    /// Tells the server the file is no longer open.
    fn drop(&mut self) {
        if let Some(server) = self.server.as_ref() {
            server.did_close(self.buffer.path());
        }
    }
}

/// One open file: the worktree it belongs to and the document itself.
struct Entry {
    /// The project whose worktree the file was opened from.
    project: ProjectId,
    /// The document, shared with whichever panes are drawing it.
    document: OpenFile,
}

/// Every file the window has open, and the servers behind them.
#[derive(Default)]
pub struct Files {
    /// The open files, by the id the panes name them with.
    open: BTreeMap<FileId, Entry>,
    /// The id the next file opened will be given.
    next: FileId,
    /// The language servers those files are open in.
    servers: Servers,
}

impl Files {
    /// Wakes the window through `notify` when a server has something to say.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.servers.set_notify(notify);
    }

    /// Opens `path` in `project`, or hands back the file if it is open already.
    ///
    /// A file that cannot be read does not open and does not complain: the
    /// tree lists what is on disk, and a directory entry that turns out not
    /// to be a readable file is the tree's business, not the store's.
    pub fn open(
        &mut self,
        project: ProjectId,
        root: &Path,
        path: &Path,
        preview: bool,
    ) -> Option<FileId> {
        if let Some(id) = self.find(project, path) {
            if !preview {
                self.keep(id);
            }
            return Some(id);
        }

        let buffer = Buffer::open(path).ok()?;
        let server = buffer
            .language()
            .and_then(|language| self.servers.open(root, language));

        let id = self.next;
        self.next = FileId(id.0 + 1);
        self.open.insert(
            id,
            Entry {
                project,
                document: Rc::new(RefCell::new(Document::new(buffer, preview, server))),
            },
        );
        Some(id)
    }

    /// The file `path` is open as in `project`, if it is open at all.
    fn find(&self, project: ProjectId, path: &Path) -> Option<FileId> {
        self.open
            .iter()
            .find(|(_, entry)| {
                entry.project == project && entry.document.borrow().buffer().path() == path
            })
            .map(|(id, _)| *id)
    }

    /// The document `id` names, if it is still open.
    pub fn get(&self, id: FileId) -> Option<OpenFile> {
        self.open.get(&id).map(|entry| entry.document.clone())
    }

    /// The project whose worktree the file `id` names was opened from.
    pub fn project_of(&self, id: FileId) -> Option<ProjectId> {
        self.open.get(&id).map(|entry| entry.project)
    }

    /// The file `id` names as a bar of tabs presents it.
    pub fn entry(&self, id: FileId) -> Option<FileEntry> {
        let document = self.open.get(&id)?.document.borrow();
        Some(FileEntry {
            id,
            name: document.buffer().name(),
            dirty: document.buffer().is_dirty(),
            preview: document.is_preview(),
        })
    }

    /// Whether the file `id` names is only being looked at.
    pub fn is_preview(&self, id: FileId) -> bool {
        self.open
            .get(&id)
            .is_some_and(|entry| entry.document.borrow().is_preview())
    }

    /// Whether the file `id` names has changes that are not on disk.
    pub fn is_dirty(&self, id: FileId) -> bool {
        self.open
            .get(&id)
            .is_some_and(|entry| entry.document.borrow().buffer().is_dirty())
    }

    /// Keeps the file `id` names open, so nothing else takes its tab.
    pub fn keep(&mut self, id: FileId) {
        if let Some(entry) = self.open.get(&id) {
            entry.document.borrow_mut().keep();
        }
    }

    /// Applies `edit` to the file `id` names.
    pub fn edit(&mut self, id: FileId, edit: impl FnOnce(&mut Buffer)) {
        if let Some(entry) = self.open.get(&id) {
            entry.document.borrow_mut().edit(edit);
        }
    }

    /// Writes the file `id` names to disk.
    pub fn save(&mut self, id: FileId) {
        if let Some(entry) = self.open.get(&id) {
            entry.document.borrow_mut().save();
        }
    }

    /// Writes every open file to disk.
    pub fn save_all(&mut self) {
        for entry in self.open.values() {
            entry.document.borrow_mut().save();
        }
    }

    /// Where the file `id` names lives, if it is open at all.
    pub fn path(&self, id: FileId) -> Option<PathBuf> {
        let entry = self.open.get(&id)?;
        let path = entry.document.borrow().buffer().path().to_path_buf();
        Some(path)
    }

    /// Closes every file no pane is holding open any more.
    ///
    /// A document lives as long as a tab somewhere names it, so closing a tab
    /// is the pane tree's business alone and the store is swept afterwards:
    /// the same file open in two panes survives one of them being closed.
    pub fn retain(&mut self, held: &BTreeSet<FileId>) {
        self.open.retain(|id, _| held.contains(id));
    }

    /// Closes every file of `project` and ends the servers over `root`.
    pub fn close_project(&mut self, project: ProjectId, root: &Path) {
        self.open.retain(|_, entry| entry.project != project);
        self.servers.close(root);
    }

    /// Takes in what the servers have said, and says whether anything is new.
    pub fn refresh(&mut self) -> bool {
        if !self.servers.take_fresh() {
            return false;
        }
        for entry in self.open.values() {
            entry.document.borrow_mut().refresh();
        }
        true
    }
}
