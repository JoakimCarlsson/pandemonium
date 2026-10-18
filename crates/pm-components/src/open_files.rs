//! The files open in the window beside the conversation: one tab each, a file looked at in a tab that the next one replaces, and kept once it is changed.

use pm_input::FocusId;

use pm_text::Edit;

use crate::git_prompts::Prompt;
use crate::panes::Item;
use crate::preview::{PREVIEW_LINK_BASE, PREVIEW_LINKS, PREVIEW_SHOW, Rendering, SOURCE_SHOW};
use crate::state::{Outgoing, Page, Tab, WorkspaceState};
use pm_editor::Document;

/// The text of the file being edited.
pub const EDITOR_FIELD: FocusId = FocusId(7083);

/// Writes the file being edited to the folder.
pub const EDITOR_SAVE: FocusId = FocusId(7084);

/// The first of the tabs of the open files; the rest follow it.
pub const FILE_TAB_BASE: u64 = 4_000_000;

/// The first of the buttons that close a file's tab; the rest follow it.
pub const FILE_CLOSE_BASE: u64 = 4_100_000;

/// How tall the strip above the lines is, in logical pixels.
pub const STRIP: f32 = 40.0;

/// What a file open in the window shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Contents {
    /// The server has not answered yet.
    Loading,
    /// The text of the file.
    Text(Box<Document>),
    /// The pixels of a picture file, decoded.
    Picture(pm_ui::Picture),
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
    /// Where the file was left when the window last closed, put back once its text arrives.
    pub left: Option<Left>,
    /// The place the caret goes to once the text arrives, as when the file was opened to show where something is defined.
    pub jump: Option<pm_protocol::Position>,
    /// Whether the tab is pinned: kept at the start of the strip, and left open when the tabs around it are closed.
    pub pinned: bool,
    /// Whether a file that can be rendered shows its source instead.
    pub source: bool,
    /// The text of a vector picture drawn, or why it could not be, once it has been tried.
    pub drawn: Option<Result<pm_ui::Picture, String>>,
}

/// Where a file was left when the window closed: what it held that was not saved, and the caret.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Left {
    /// The text that was not written to the folder, when the file had changes.
    pub unsaved: Option<String>,
    /// The caret, as a byte offset into the text.
    pub caret: usize,
}

impl Left {
    /// Puts the unsaved text and the caret back into `document`, which was just read from the folder.
    fn put_back(self, document: &mut Document) {
        if let Some(text) = self.unsaved.filter(|_| !document.field.is_read_only()) {
            let before = document.field.text().to_owned();
            let before_lines = document.line_index().clone();
            document.field.apply(Edit::Place(0, false));
            document.field.apply(Edit::Place(before.len(), true));
            document.field.apply(Edit::Paste(text));
            document.refresh(&before, &before_lines);
        }
        let caret = self.caret.min(document.field.text().len());
        if document.field.text().is_char_boundary(caret) {
            document.field.apply(Edit::Place(caret, false));
        }
    }
}

/// Puts the caret of `document` at `position` and returns how far down to scroll for its line to show a little below the top, rows `row` logical pixels tall holding `columns` characters when lines wrap.
pub(crate) fn jump(
    document: &mut Document,
    position: pm_protocol::Position,
    row: f32,
    columns: Option<usize>,
) -> f32 {
    let line = document.jump(position);
    (document.wrapped_row_of_line(line, columns) as f32 * row - row * 6.0).max(0.0)
}

impl Viewer {
    /// Whether the file has changes that were not written to the folder.
    pub fn is_dirty(&self) -> bool {
        matches!(&self.contents, Contents::Text(document) if document.is_dirty())
    }

    /// How the file is shown rendered, or nothing when it is shown as its source.
    pub fn rendering(&self) -> Option<Rendering> {
        Rendering::of(&self.path).filter(|rendering| !rendering.has_source() || !self.source)
    }

    /// What asks for the text of the file to be drawn as a vector picture, when it is one and is shown rendered.
    fn draw_request(&self, session: usize) -> Option<Outgoing> {
        let Contents::Text(document) = &self.contents else {
            return None;
        };
        (self.rendering() == Some(Rendering::Vector)).then(|| Outgoing::DrawVector {
            session,
            path: self.path.clone(),
            head: self.head,
            text: document.field.text().to_owned(),
        })
    }
}

impl WorkspaceState {
    /// How many characters a row of the file shown holds when long lines wrap, as soft wrap and the view's width say, or nothing when they do not.
    pub fn wrap_columns(&self) -> Option<usize> {
        let fitting = self.code_view.map(|(width, cell)| {
            let scale = pm_style::step_scale();
            let numbers = if self.editor.line_numbers { 13.0 } else { 2.0 };
            let gutter =
                3.0 + (numbers + 4.0) * 4.0 * scale + 12.0 + crate::viewer::BREAKPOINT_CELL;
            (((width - gutter) / cell.max(1.0)).floor() as usize).max(10)
        });
        match self.editor.soft_wrap {
            pm_editor::SoftWrap::Off => None,
            pm_editor::SoftWrap::Edge => fitting,
            pm_editor::SoftWrap::LineLength => Some(
                fitting.map_or(self.editor.preferred_line_length, |fitting| {
                    fitting.min(self.editor.preferred_line_length)
                }),
            ),
        }
    }

    /// The state knowing the view of the file shown is `width` logical pixels wide, with a character of code `cell` wide.
    pub fn with_code_view(mut self, width: f32, cell: f32) -> Self {
        self.code_view = Some((width, cell));
        self
    }

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
        self.recent_files.retain(|held| *held != path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(crate::listing::RECENT_FILES);
        self.settle_panes();
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
                    left: None,
                    jump: None,
                    pinned: false,
                    source: false,
                    drawn: None,
                };
                self.outbox.push(match Rendering::of(&path) {
                    Some(Rendering::Picture) => Outgoing::ReadPicture {
                        session,
                        path,
                        head,
                    },
                    _ => Outgoing::ReadFile {
                        session,
                        path,
                        head,
                    },
                });
                match self.git.viewers.iter().position(|held| held.preview) {
                    Some(index) => {
                        let replaced = std::mem::replace(&mut self.git.viewers[index], viewer);
                        self.close_document(session, &replaced);
                        self.replace_pane_item(
                            Item::File {
                                path: replaced.path.clone(),
                                head: replaced.head,
                            },
                            Item::File {
                                path: self.git.viewers[index].path.clone(),
                                head,
                            },
                        );
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
            self.pane_stash
                .insert(before, std::mem::take(&mut self.panes));
        } else {
            self.files_owner = now;
            return;
        }
        if let Some((viewers, shown)) = now.and_then(|now| self.file_stash.remove(&now)) {
            self.git.viewers = viewers;
            self.git.shown = shown;
        }
        if let Some(panes) = now.and_then(|now| self.pane_stash.remove(&now)) {
            self.panes = panes;
        }
        self.files_owner = now;
        if let (Page::Session(session), Some(tab)) =
            (self.page, now.and_then(|now| self.tab_stash.remove(&now)))
            && Some(session) == now
            && (tab != Tab::File || self.git.shown.is_some())
        {
            self.tab = tab;
        }
    }

    /// The open file at `path` of the session at `session`, read from the last commit when `head`, whether it is showing or kept aside.
    fn viewer_of(&mut self, session: usize, path: &str, head: bool) -> Option<&mut Viewer> {
        self.viewers_of(session)?
            .iter_mut()
            .find(|viewer| viewer.path == path && viewer.head == head)
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

    /// The state after the server answered for the file at `path` of the session at `session`: its text, whether it was cut short and the installed language it is written in, or why there is none.
    pub fn with_file(
        mut self,
        session: usize,
        path: &str,
        head: bool,
        read: Result<(Option<String>, bool, Option<String>), String>,
    ) -> Self {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let language = read
            .as_ref()
            .ok()
            .and_then(|(text, _, language)| text.as_ref().and(language.clone()));
        let mut draw = None;
        if let Some(viewer) = self.viewer_of(session, path, head) {
            viewer.contents = match read {
                Ok((Some(text), truncated, _)) => {
                    let mut document = Document::new(&text, head, truncated);
                    if let Some(left) = viewer.left.take() {
                        left.put_back(&mut document);
                    }
                    if let Some(position) = viewer.jump.take() {
                        viewer.scroll = jump(&mut document, position, row, columns);
                    }
                    Contents::Text(Box::new(document))
                }
                Ok((None, _, _)) => Contents::Message("This file is not text.".to_owned()),
                Err(reason) => Contents::Message(reason),
            };
            draw = viewer.draw_request(session);
        }
        self.outbox.extend(draw);
        self.ask_shown_figures();
        let opened = !head
            && self.viewers_of(session).is_some_and(|viewers| {
                viewers.iter().any(|viewer| {
                    viewer.path == path
                        && !viewer.head
                        && matches!(viewer.contents, Contents::Text(_))
                })
            });
        if opened {
            self.outbox.push(Outgoing::ReadBase {
                session,
                path: path.to_owned(),
            });
        }
        self.took_language(session, path, head, language);
        self
    }

    /// The state after the server answered for the picture file at `path` of the session at `session` with its pixels decoded, or why there are none.
    pub fn with_picture(
        mut self,
        session: usize,
        path: &str,
        head: bool,
        read: Result<pm_ui::Picture, String>,
    ) -> Self {
        if let Some(viewer) = self.viewer_of(session, path, head) {
            viewer.contents = match read {
                Ok(picture) => Contents::Picture(picture),
                Err(reason) => Contents::Message(reason),
            };
        }
        self
    }

    /// The state after the text of the vector picture at `path` of the session at `session` was drawn, or could not be.
    pub fn with_drawn(
        mut self,
        session: usize,
        path: &str,
        head: bool,
        drawn: Result<pm_ui::Picture, String>,
    ) -> Self {
        if let Some(viewer) = self.viewer_of(session, path, head) {
            viewer.drawn = Some(drawn);
        }
        self
    }

    /// Shows the file shown as its source when `source`, or else rendered, drawing it again as its text now stands.
    fn show_source(&mut self, source: bool) {
        let Some(session) = self.review_index() else {
            return;
        };
        let Some(viewer) = self
            .git
            .shown
            .and_then(|index| self.git.viewers.get_mut(index))
        else {
            return;
        };
        viewer.source = source;
        viewer.scroll = 0.0;
        let draw = viewer.draw_request(session);
        self.outbox.extend(draw);
        self.ask_shown_figures();
    }

    /// Asks for every figure of the markdown file shown rendered that is neither drawn nor asked for yet.
    pub(crate) fn ask_shown_figures(&mut self) {
        let Some(session) = self.review_index() else {
            return;
        };
        let Some(viewer) = self
            .shown_viewer()
            .filter(|viewer| viewer.rendering() == Some(Rendering::Markdown))
        else {
            return;
        };
        let Contents::Text(document) = &viewer.contents else {
            return;
        };
        let wanted = crate::figures::figures_of(document.field.text(), Some(&viewer.path));
        self.ask_figures(wanted, Some(session));
    }

    /// Follows the link at `place` among those of the markdown file shown rendered: an address on the web opens in the browser, and a file of the repository opens in a tab.
    fn follow_preview_link(&mut self, place: usize) {
        let Some(viewer) = self.shown_viewer() else {
            return;
        };
        let Contents::Text(document) = &viewer.contents else {
            return;
        };
        let Some(address) = pm_markdown::links(document.field.text())
            .into_iter()
            .nth(place)
        else {
            return;
        };
        if pm_acp::is_web_address(&address) {
            self.outbox.push(Outgoing::Open { address });
        } else if let Some(path) = crate::preview::linked_path(&viewer.path, &address) {
            let head = viewer.head;
            self.open_viewer(path, head);
        }
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
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let Some(index) = self.git.shown else {
            return self;
        };
        let Some(viewer) = self.git.viewers.get_mut(index) else {
            return self;
        };
        let Contents::Text(document) = &viewer.contents else {
            return self;
        };
        let line = document.caret_row(columns);
        let (top, bottom) = (line as f32 * row, (line + 1) as f32 * row);
        let view = (height - STRIP).max(row);
        let room = (view / row / 2.0).floor().max(0.0) as usize;
        let margin = self.editor.vertical_scroll_margin.min(room) as f32 * row;
        if top - margin < viewer.scroll {
            viewer.scroll = (top - margin).max(0.0);
        } else if bottom + margin > viewer.scroll + view {
            viewer.scroll = bottom + margin - view;
        }
        self
    }

    /// Writes the file shown to the folder.
    pub(crate) fn save_file(&mut self) {
        let Some(session) = self.review_index() else {
            return;
        };
        let (trim, final_newline) = (
            self.editor.remove_trailing_whitespace_on_save,
            self.editor.ensure_final_newline_on_save,
        );
        let tidy = self
            .shown_document()
            .filter(|document| !document.field.is_read_only())
            .and_then(|document| document.tidy_edit(trim, final_newline));
        if let Some(edit) = tidy {
            self.file_edited(edit);
        }
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
    pub(crate) fn close_file(&mut self, index: usize) {
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
        let closed = self.git.viewers.remove(index);
        if let Some(session) = self.review_index() {
            self.close_document(session, &closed);
        }
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
            self.ask_shown_figures();
            return true;
        }
        if let Some(index) = action
            .0
            .checked_sub(FILE_CLOSE_BASE)
            .filter(|index| *index < count)
        {
            let index = index as usize;
            if self.git.viewers[index].pinned {
                self.git.viewers[index].pinned = false;
            } else {
                self.close_file(index);
            }
            return true;
        }
        if let Some(place) = action
            .0
            .checked_sub(PREVIEW_LINK_BASE)
            .filter(|place| *place < PREVIEW_LINKS)
        {
            self.follow_preview_link(place as usize);
            return true;
        }
        match action {
            PREVIEW_SHOW => self.show_source(false),
            SOURCE_SHOW => self.show_source(true),
            EDITOR_SAVE => self.format_then_save(true, None),
            EDITOR_FIELD => {}
            _ => return false,
        }
        true
    }
}
