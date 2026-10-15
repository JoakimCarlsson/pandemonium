//! The files open in the window beside the conversation: one tab each, a file looked at in a tab that the next one replaces, and kept once it is changed.

use pm_input::FocusId;
use pm_text::Edit;

use crate::document::Document;
use crate::git_prompts::Prompt;
use crate::state::{Outgoing, Tab, WorkspaceState};

/// The text of the file being edited.
pub const EDITOR_FIELD: FocusId = FocusId(7083);

/// Writes the file being edited to the folder.
pub const EDITOR_SAVE: FocusId = FocusId(7084);

/// The first of the tabs of the open files; the rest follow it.
pub const FILE_TAB_BASE: u64 = 4_000_000;

/// The first of the buttons that close a file's tab; the rest follow it.
pub const FILE_CLOSE_BASE: u64 = 4_100_000;

/// How tall a line of the file is, in logical pixels.
pub const ROW: f32 = 20.0;

/// How tall the strip above the lines is, in logical pixels.
pub const STRIP: f32 = 40.0;

/// What a file open in the window shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Contents {
    /// The server has not answered yet.
    Loading,
    /// The text of the file.
    Text(Box<Document>),
    /// Why there is nothing to show.
    Message(String),
}

/// A file open in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewer {
    /// The file, from the root of the repository.
    pub path: String,
    /// Whether it is read from the last commit rather than from the folder.
    pub head: bool,
    /// What it holds.
    pub contents: Contents,
    /// How far down it is scrolled, in logical pixels.
    pub scroll: f32,
    /// Whether it is only being looked at: its tab is given up to the next file looked at, until the file is changed.
    pub preview: bool,
}

impl Viewer {
    /// Whether the file has changes that were not written to the folder.
    pub fn is_dirty(&self) -> bool {
        matches!(&self.contents, Contents::Text(document) if document.is_dirty())
    }
}

impl WorkspaceState {
    /// The file whose tab is showing, if one is.
    pub fn shown_viewer(&self) -> Option<&Viewer> {
        self.git
            .shown
            .and_then(|index| self.git.viewers.get(index))
            .filter(|_| self.tab == Tab::File)
    }

    /// The text of the file whose tab is showing, if it holds text.
    pub fn shown_document(&self) -> Option<&Document> {
        match &self.shown_viewer()?.contents {
            Contents::Text(document) => Some(document),
            _ => None,
        }
    }

    /// Looks at the file at `path` in the window, as it is in the last commit when `head`, and asks the server for it; a file already open is shown, and any other takes the tab of the file looked at before.
    pub fn open_viewer(&mut self, path: String, head: bool) {
        let Some(session) = self.review_index() else {
            return;
        };
        let held = self
            .git
            .viewers
            .iter()
            .position(|viewer| viewer.path == path && viewer.head == head);
        let shown = match held {
            Some(index) => index,
            None => {
                let viewer = Viewer {
                    path: path.clone(),
                    head,
                    contents: Contents::Loading,
                    scroll: 0.0,
                    preview: true,
                };
                self.outbox.push(Outgoing::ReadFile {
                    session,
                    path,
                    head,
                });
                match self.git.viewers.iter().position(|held| held.preview) {
                    Some(index) => {
                        self.git.viewers[index] = viewer;
                        index
                    }
                    None => {
                        self.git.viewers.push(viewer);
                        self.git.viewers.len() - 1
                    }
                }
            }
        };
        self.git.shown = Some(shown);
        self.tab = Tab::File;
    }

    /// Starts the controls over, keeping the files open, which belong to the folder being worked in and are swapped by [`Self::sync_files_owner`].
    pub(crate) fn fresh_git(&mut self) {
        let viewers = std::mem::take(&mut self.git.viewers);
        let shown = self.git.shown;
        self.git = crate::git_controls::GitControls::default();
        self.git.viewers = viewers;
        self.git.shown = shown;
    }

    /// Makes the open files the ones of the folder the review panel is about: those of the folder left are kept aside, under its session, and those kept aside for the one arrived at are brought back, so every workspace and session shows its own files.
    pub(crate) fn sync_files_owner(&mut self) {
        let now = self.review_index();
        if now == self.files_owner {
            return;
        }
        if let Some(before) = self.files_owner {
            let tabs = (std::mem::take(&mut self.git.viewers), self.git.shown.take());
            if !tabs.0.is_empty() {
                self.file_stash.insert(before, tabs);
            }
        } else {
            self.files_owner = now;
            return;
        }
        if let Some((viewers, shown)) = now.and_then(|now| self.file_stash.remove(&now)) {
            self.git.viewers = viewers;
            self.git.shown = shown;
        }
        self.files_owner = now;
    }

    /// The open files of the session at `session`, whether they are showing or kept aside.
    fn viewers_of(&mut self, session: usize) -> Option<&mut Vec<Viewer>> {
        if self.files_owner == Some(session) {
            Some(&mut self.git.viewers)
        } else {
            self.file_stash.get_mut(&session).map(|tabs| &mut tabs.0)
        }
    }

    /// The state after the file shown is kept open because the reader pressed its tab or its row twice in quick succession, which is how a file looked at is kept without changing it.
    pub fn with_file_kept(mut self, id: FocusId) -> Self {
        let tab =
            id.0.checked_sub(FILE_TAB_BASE)
                .filter(|index| *index < self.git.viewers.len() as u64)
                .map(|index| index as usize);
        let row = (self.tree_row_of(id).is_some_and(|target| !target.folder))
            .then_some(self.git.shown)
            .flatten();
        if let Some(viewer) = tab
            .or(row)
            .and_then(|index| self.git.viewers.get_mut(index))
        {
            viewer.preview = false;
        }
        self
    }

    /// The state after the server answered for the file at `path` of the session at `session`: its text and whether it was cut short, or why there is none.
    pub fn with_file(
        mut self,
        session: usize,
        path: &str,
        head: bool,
        read: Result<(Option<String>, bool), String>,
    ) -> Self {
        if let Some(viewer) = self.viewers_of(session).and_then(|viewers| {
            viewers
                .iter_mut()
                .find(|viewer| viewer.path == path && viewer.head == head)
        }) {
            viewer.contents = match read {
                Ok((Some(text), truncated)) => {
                    Contents::Text(Box::new(Document::new(&text, head, truncated)))
                }
                Ok((None, _)) => Contents::Message("This file is not text.".to_owned()),
                Err(reason) => Contents::Message(reason),
            };
        }
        self
    }

    /// The state after the file shown is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_viewer(mut self, delta: f32, room: f32) -> Self {
        if let Some(index) = self.git.shown
            && let Some(viewer) = self.git.viewers.get_mut(index)
        {
            viewer.scroll = (viewer.scroll + delta).clamp(0.0, room);
        }
        self
    }

    /// The state with the file shown scrolled just far enough to show the caret, in a view `height` logical pixels tall.
    pub fn with_caret_shown(mut self, height: f32) -> Self {
        let Some(index) = self.git.shown else {
            return self;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return self;
        };
        let Contents::Text(document) = &viewer.contents else {
            return self;
        };
        let line = document.line_of(document.field.caret());
        let (top, bottom) = (line as f32 * ROW, (line + 1) as f32 * ROW);
        let view = (height - STRIP).max(ROW);
        if top < viewer.scroll {
            viewer.scroll = top;
        } else if bottom + ROW > viewer.scroll + view {
            viewer.scroll = bottom + ROW - view;
        }
        self
    }

    /// Applies `edit` to the file shown; changing its text keeps its tab open.
    pub(crate) fn file_edited(&mut self, edit: Edit) {
        let Some(index) = self.git.shown else {
            return;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return;
        };
        let Contents::Text(document) = &mut viewer.contents else {
            return;
        };
        document.field.apply(edit);
        if document.refresh() {
            viewer.preview = false;
        }
    }

    /// Writes the file shown to the folder.
    fn save_file(&mut self) {
        let Some(session) = self.review_index() else {
            return;
        };
        let Some(index) = self.git.shown else {
            return;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return;
        };
        let Contents::Text(document) = &mut viewer.contents else {
            return;
        };
        if document.field.is_read_only() || !document.is_dirty() {
            return;
        }
        document.mark_saved();
        let text = document.to_save();
        let path = viewer.path.clone();
        self.git.trouble = None;
        self.outbox.push(Outgoing::Write {
            session,
            path,
            text,
        });
    }

    /// The state after the server answered a write of the file at `path` of the session at `session`: a file that could not be written is changed again as far as the window can tell.
    pub fn with_file_written(mut self, session: usize, path: &str, failed: bool) -> Self {
        if failed && let Some(viewers) = self.viewers_of(session) {
            for viewer in viewers
                .iter_mut()
                .filter(|viewer| viewer.path == path && !viewer.head)
            {
                if let Contents::Text(document) = &mut viewer.contents {
                    document.mark_unsaved();
                }
            }
        }
        self
    }

    /// Closes the tab at `index`, first asking when its file has changes that were not saved.
    fn close_file(&mut self, index: usize) {
        if self.git.viewers.get(index).is_some_and(Viewer::is_dirty) {
            self.git.shown = Some(index);
            self.tab = Tab::File;
            self.ask(Prompt::Unsaved(index));
        } else {
            self.discard_file(index);
        }
    }

    /// Closes the tab at `index`, whatever its file holds, and shows the tab beside it or else the view before.
    pub(crate) fn discard_file(&mut self, index: usize) {
        if index >= self.git.viewers.len() {
            return;
        }
        self.git.viewers.remove(index);
        self.git.shown = match self.git.shown {
            _ if self.git.viewers.is_empty() => None,
            Some(shown) if shown > index => Some(shown - 1),
            Some(shown) if shown == index => Some(index.min(self.git.viewers.len() - 1)),
            other => other,
        };
        if self.git.shown.is_none() && self.tab == Tab::File {
            self.tab = match self.review_session() {
                Some(session) if !session.changes.is_empty() => Tab::Diff,
                _ => Tab::Chat,
            };
        }
    }

    /// Closes every tab of the file at `path`, or of a file in it, because it was renamed or taken off the disk.
    pub(crate) fn drop_viewer_of(&mut self, path: &str) {
        let inside = |held: &str| {
            held == path
                || held
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        while let Some(index) = self
            .git
            .viewers
            .iter()
            .position(|viewer| !viewer.head && inside(&viewer.path))
        {
            self.discard_file(index);
        }
    }

    /// Carries out `action` if it is a tab of an open file, a button of one or the file's own keys, and says whether it was.
    pub(crate) fn press_files(&mut self, action: FocusId) -> bool {
        let count = self.git.viewers.len() as u64;
        if let Some(index) = action
            .0
            .checked_sub(FILE_TAB_BASE)
            .filter(|index| *index < count)
        {
            self.git.shown = Some(index as usize);
            self.tab = Tab::File;
            return true;
        }
        if let Some(index) = action
            .0
            .checked_sub(FILE_CLOSE_BASE)
            .filter(|index| *index < count)
        {
            self.close_file(index as usize);
            return true;
        }
        match action {
            EDITOR_SAVE => self.save_file(),
            EDITOR_FIELD => {}
            _ => return false,
        }
        true
    }
}
