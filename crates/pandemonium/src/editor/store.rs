//! The files the window has open, listed per project.
//!
//! A file is opened once per project and shown in that project's pane: the
//! same path in two projects is two documents, because it is two worktrees.
//! This is the one seam a file is opened, edited, saved and closed through,
//! so the language server hears about every change exactly once.

use std::cell::RefCell;
use std::collections::BTreeMap;
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

/// One open file as the bar of tabs presents it.
pub struct FileEntry {
    /// Which file this tab is.
    pub id: FileId,
    /// What the tab calls it: the file's own name.
    pub name: String,
    /// Whether it has changes that are not on disk.
    pub dirty: bool,
    /// Whether this is the file the pane is showing.
    pub active: bool,
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
    /// A previewed file holds the one preview tab of its project and gives
    /// it up to the next file previewed. Editing it, or asking for it a
    /// second time, is what keeps it.
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

/// One project's open files and which of them its pane is showing.
#[derive(Default)]
struct ProjectFiles {
    /// The files, in the order they were opened.
    open: Vec<(FileId, OpenFile)>,
    /// The one the pane is showing.
    active: Option<FileId>,
}

impl ProjectFiles {
    /// The file `id` names, if it is still open.
    fn get(&self, id: FileId) -> Option<OpenFile> {
        self.open
            .iter()
            .find(|(open, _)| *open == id)
            .map(|(_, file)| file.clone())
    }

    /// The file `path` is open as, if it is open at all.
    fn find(&self, path: &Path) -> Option<FileId> {
        self.open
            .iter()
            .find(|(_, file)| file.borrow().buffer().path() == path)
            .map(|(id, _)| *id)
    }

    /// The file the pane is showing.
    fn active(&self) -> Option<OpenFile> {
        self.active.and_then(|id| self.get(id))
    }

    /// Shows another file when the one that was showing has been closed.
    fn settle(&mut self) {
        if self.active().is_none() {
            self.active = self.open.last().map(|(id, _)| *id);
        }
    }
}

/// Every file the window has open, and the servers behind them.
#[derive(Default)]
pub struct Files {
    /// The open files, by the project whose worktree they belong to.
    projects: BTreeMap<ProjectId, ProjectFiles>,
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

    /// Opens `path` in `project`, or shows it if it is already open.
    ///
    /// A file opened for preview takes the project's one preview tab from
    /// whatever held it, which is what makes clicking through a tree leave
    /// one tab behind rather than twenty. Opening the same file again
    /// without `preview` keeps it where it is.
    ///
    /// A file that cannot be read does not open and does not complain: the
    /// tree lists what is on disk, and a directory entry that turns out not
    /// to be a readable file is the tree's business, not the pane's.
    pub fn open(
        &mut self,
        project: ProjectId,
        root: &Path,
        path: &Path,
        preview: bool,
    ) -> Option<FileId> {
        if let Some(id) = self.projects.get(&project).and_then(|open| open.find(path)) {
            self.activate(project, id);
            if !preview {
                self.keep(project, id);
            }
            return Some(id);
        }

        let buffer = Buffer::open(path).ok()?;
        let server = buffer
            .language()
            .and_then(|language| self.servers.open(root, language));

        if preview {
            self.close_preview(project);
        }

        let id = self.next;
        self.next = FileId(id.0 + 1);
        let files = self.projects.entry(project).or_default();
        files.open.push((
            id,
            Rc::new(RefCell::new(Document::new(buffer, preview, server))),
        ));
        files.active = Some(id);
        Some(id)
    }

    /// Keeps the file `id` names open, so nothing else takes its tab.
    pub fn keep(&mut self, project: ProjectId, id: FileId) {
        if let Some(file) = self.projects.get(&project).and_then(|files| files.get(id)) {
            file.borrow_mut().keep();
        }
    }

    /// Closes the file `project` is previewing, if it is previewing one.
    fn close_preview(&mut self, project: ProjectId) {
        self.retain_files(project, |file| !file.borrow().is_preview());
    }

    /// The file `project` is showing, if it has one.
    pub fn active(&self, project: ProjectId) -> Option<OpenFile> {
        self.projects.get(&project)?.active()
    }

    /// Shows the file `id` names.
    pub fn activate(&mut self, project: ProjectId, id: FileId) {
        if let Some(files) = self.projects.get_mut(&project)
            && files.get(id).is_some()
        {
            files.active = Some(id);
        }
    }

    /// Every open file of `project`, in the order they were opened.
    pub fn list(&self, project: ProjectId) -> Vec<FileEntry> {
        let Some(files) = self.projects.get(&project) else {
            return Vec::new();
        };
        files
            .open
            .iter()
            .map(|(id, file)| {
                let document = file.borrow();
                FileEntry {
                    id: *id,
                    name: document.buffer().name(),
                    dirty: document.buffer().is_dirty(),
                    active: files.active == Some(*id),
                    preview: document.is_preview(),
                }
            })
            .collect()
    }

    /// Applies `edit` to the file `project` is showing.
    pub fn edit(&mut self, project: ProjectId, edit: impl FnOnce(&mut Buffer)) {
        if let Some(file) = self.active(project) {
            file.borrow_mut().edit(edit);
        }
    }

    /// Writes the file `project` is showing to disk.
    pub fn save(&mut self, project: ProjectId) {
        if let Some(file) = self.active(project) {
            file.borrow_mut().save();
        }
    }

    /// Writes every open file of every project to disk.
    pub fn save_all(&mut self) {
        for files in self.projects.values() {
            for (_, file) in &files.open {
                file.borrow_mut().save();
            }
        }
    }

    /// Where the file `id` names lives, if it is open at all.
    pub fn path(&self, project: ProjectId, id: FileId) -> Option<PathBuf> {
        let file = self.projects.get(&project)?.get(id)?;
        let path = file.borrow().buffer().path().to_path_buf();
        Some(path)
    }

    /// Closes every file of `project` but the one `id` names.
    pub fn close_others(&mut self, project: ProjectId, id: FileId) {
        self.retain(project, |open| open == id);
    }

    /// Closes the files of `project` opened before the one `id` names.
    pub fn close_left(&mut self, project: ProjectId, id: FileId) {
        let Some(index) = self.index_of(project, id) else {
            return;
        };
        let mut seen = 0;
        self.retain(project, |_| {
            let keep = seen >= index;
            seen += 1;
            keep
        });
    }

    /// Closes the files of `project` opened after the one `id` names.
    pub fn close_right(&mut self, project: ProjectId, id: FileId) {
        let Some(index) = self.index_of(project, id) else {
            return;
        };
        let mut seen = 0;
        self.retain(project, |_| {
            let keep = seen <= index;
            seen += 1;
            keep
        });
    }

    /// Closes the files of `project` that are the same as they are on disk.
    pub fn close_saved(&mut self, project: ProjectId) {
        self.retain_files(project, |file| file.borrow().buffer().is_dirty());
    }

    /// Closes every file of `project`.
    pub fn close_all(&mut self, project: ProjectId) {
        self.retain(project, |_| false);
    }

    /// Where the file `id` names sits in the project's list of open files.
    fn index_of(&self, project: ProjectId, id: FileId) -> Option<usize> {
        self.projects
            .get(&project)?
            .open
            .iter()
            .position(|(open, _)| *open == id)
    }

    /// Keeps the files of `project` whose id `keep` accepts.
    fn retain(&mut self, project: ProjectId, mut keep: impl FnMut(FileId) -> bool) {
        let Some(files) = self.projects.get_mut(&project) else {
            return;
        };
        files.open.retain(|(id, _)| keep(*id));
        files.settle();
    }

    /// Keeps the files of `project` that `keep` accepts.
    fn retain_files(&mut self, project: ProjectId, mut keep: impl FnMut(&OpenFile) -> bool) {
        let Some(files) = self.projects.get_mut(&project) else {
            return;
        };
        files.open.retain(|(_, file)| keep(file));
        files.settle();
    }

    /// Closes the file `id` names, showing another of the project's instead.
    pub fn close(&mut self, project: ProjectId, id: FileId) {
        let Some(files) = self.projects.get_mut(&project) else {
            return;
        };
        files.open.retain(|(open, _)| *open != id);
        files.settle();
    }

    /// Closes every file of `project` and ends the servers over `root`.
    pub fn close_project(&mut self, project: ProjectId, root: &Path) {
        self.projects.remove(&project);
        self.servers.close(root);
    }

    /// Takes in what the servers have said, and says whether anything is new.
    pub fn refresh(&mut self) -> bool {
        if !self.servers.take_fresh() {
            return false;
        }
        for files in self.projects.values() {
            for (_, file) in &files.open {
                file.borrow_mut().refresh();
            }
        }
        true
    }
}
