//! The files open in the session view: their tabs beside the conversation's
//! and the diff's, the preview tab a file opens in until it is kept, the tab
//! chosen, and the files closed lately, which can be opened again.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use std::sync::Arc;

use pm_editor::Document;
use pm_treesitter::{Grammar, Syntax, registry};

/// The most closed files remembered to open again.
const CLOSED_KEPT: usize = 20;

/// A document shared between the model and the editor drawing it, which
/// edits it in place.
#[derive(Clone)]
pub struct SharedDocument(pub Rc<RefCell<Document>>);

impl SharedDocument {
    /// A shared document holding `document`.
    pub fn new(document: Document) -> Self {
        Self(Rc::new(RefCell::new(document)))
    }
}

impl Default for SharedDocument {
    /// An empty document that can be edited.
    fn default() -> Self {
        Self::new(Document::new("", false, false))
    }
}

impl PartialEq for SharedDocument {
    /// Whether both are the same document.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for SharedDocument {
    /// Names the document by its length, not its text.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let length = self
            .0
            .try_borrow()
            .map_or(0, |held| held.field.text().len());
        write!(formatter, "SharedDocument({length} bytes)")
    }
}

/// What an open file shows while it is read and once it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum FileBody {
    /// Its contents are on their way.
    #[default]
    Loading,
    /// Its text, in a document.
    Text,
    /// A picture's encoded bytes.
    Picture(Rc<Vec<u8>>),
    /// Why it cannot be shown.
    Failed(String),
}

/// A file open in a tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenFile {
    /// The folder it belongs to.
    pub directory: String,
    /// Its path from the root of the folder.
    pub path: String,
    /// Whether its tab is the preview tab, which the next file opened takes over.
    pub preview: bool,
    /// What it shows.
    pub body: FileBody,
    /// Its text, the caret and the syntax.
    pub document: SharedDocument,
    /// Whether a markdown file shows rendered rather than as text.
    pub rendered: bool,
    /// The line, counted from 1, to put the caret on once the text arrives.
    pub line: Option<usize>,
    /// What the last commit holds of it, which the change marks beside the
    /// line numbers compare against; `None` while unknown or uncommitted.
    pub base: Option<String>,
    /// The bar its editor opens with over the text, when one is asked for.
    pub bar: Option<EditorBar>,
    /// The changes the language server offered at the caret last, which the lightbulb lists.
    pub actions: Vec<pm_lsp::CodeAction>,
    /// Changes the language server made to the file before its text arrived, made once it does.
    pub pending: Vec<pm_lsp::TextChange>,
    /// The popup its editor opens with, when one is asked for.
    pub popup: Option<EditorPopup>,
    /// What renaming at the caret starts from, while its field is open: the
    /// stretch holding the name, as bytes of the text, and the name.
    pub rename: Option<(std::ops::Range<usize>, String)>,
    /// The comparison shown in place of the text, while one is.
    pub compare: Option<Comparison>,
    /// Unsaved text kept from the last run, put in place of the text read
    /// from the folder once it arrives.
    pub restored: Option<String>,
    /// Whether its tab is pinned: kept first in the strip and left open by Close others, Close to the right and Close all.
    pub pinned: bool,
}

/// A popup an editor can be asked to open with over its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorPopup {
    /// The changes the lightbulb offers.
    Actions,
    /// The colour picker over the colour at this place of the file's list of colours.
    Colour(usize),
}

/// The file compared with another text, shown as a diff.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Comparison {
    /// What the other side is called, such as the clipboard or a path.
    pub label: String,
    /// The difference, as a unified diff from its first hunk on: the other
    /// side's lines as the old ones and the file's as the new.
    pub diff: String,
}

impl Comparison {
    /// The comparison of `other`, called `label`, on the old side with
    /// `text` on the new.
    pub fn between(label: impl Into<String>, other: &str, text: &str) -> Self {
        let diff = similar::TextDiff::from_lines(other, text)
            .unified_diff()
            .context_radius(3)
            .to_string();
        Self {
            label: label.into(),
            diff,
        }
    }

    /// Whether both sides are the same.
    pub fn is_same(&self) -> bool {
        self.diff.trim().is_empty()
    }
}

/// A bar an editor can be asked to open with over its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorBar {
    /// The find bar, looking for this text.
    Find(String),
    /// The find bar with its replace row, looking for this text.
    Replace(String),
    /// The go-to-line bar, with this typed.
    GoTo(String),
}

impl OpenFile {
    /// The name its tab reads: the last part of its path.
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Whether its text differs from what was last read or saved.
    pub fn is_dirty(&self) -> bool {
        self.body == FileBody::Text
            && self
                .document
                .0
                .try_borrow()
                .is_ok_and(|held| held.is_dirty())
    }

    /// Takes in what the file holds: `text`, cut short when `truncated`,
    /// written in the installed language `language`, highlighted at once
    /// when its grammar is loaded.
    pub fn take_text(&mut self, text: &str, truncated: bool, language: Option<String>) {
        let mut document = Document::new(text, false, truncated);
        let grammar = language.as_deref().and_then(registry::called);
        document.language = language;
        if let Some(syntax) = grammar.and_then(|grammar| Syntax::parse(grammar, text)) {
            document.with_syntax(syntax);
        }
        document.set_base(self.base.clone());
        let pending = std::mem::take(&mut self.pending);
        if !pending.is_empty() {
            for edit in document.change_edits(pending) {
                document.edit(edit);
            }
        }
        if let Some(restored) = self.restored.take() {
            let whole = document.field.text().len();
            document.edit(pm_text::Edit::Replace(0..whole, restored, (0, 0)));
        }
        self.document = SharedDocument::new(document);
        self.body = FileBody::Text;
        if let Some(line) = self.line.take() {
            self.go_to(line);
        }
    }

    /// Takes in what the last commit holds of the file, `base`, or that no
    /// commit holds it, and marks the lines that differ from it.
    pub fn take_base(&mut self, base: Option<String>) {
        self.base = base;
        if self.body == FileBody::Text {
            self.document.0.borrow_mut().set_base(self.base.clone());
        }
    }

    /// Puts the caret on `line`, counted from 1, now when the text is here
    /// and otherwise once it arrives.
    pub fn go_to(&mut self, line: usize) {
        if self.body != FileBody::Text {
            self.line = Some(line);
            return;
        }
        let mut document = self.document.0.borrow_mut();
        let edit = document.go_to_line_edit(line);
        document.edit(edit);
    }

    /// Highlights the file with `grammar` when it is written in its language
    /// and not highlighted yet.
    pub fn highlight_with(&mut self, grammar: &Arc<Grammar>) {
        let Ok(mut document) = self.document.0.try_borrow_mut() else {
            return;
        };
        let written_in = document
            .language
            .as_deref()
            .is_some_and(|language| language.eq_ignore_ascii_case(grammar.name()));
        if !written_in || document.syntax.is_some() {
            return;
        }
        if let Some(syntax) = Syntax::parse(grammar.clone(), document.field.text()) {
            document.with_syntax(syntax);
            document.repaint();
        }
    }

    /// Whether it is a markdown file, which can show rendered.
    pub fn is_markdown(&self) -> bool {
        let lower = self.path.to_ascii_lowercase();
        lower.ends_with(".md") || lower.ends_with(".markdown")
    }
}

/// Whether the file at `path` is a picture, shown rather than read as text.
pub fn is_picture(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".ico"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// A tab of the session view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionTab {
    /// The conversation.
    #[default]
    Chat,
    /// The session's changes.
    Diff,
    /// The open file at this position.
    File(usize),
    /// The browser.
    Browser,
    /// The lines around the hits of a search, from every file, edited in place.
    Excerpts,
    /// The notes of the newer release, and the button that installs it.
    Update,
}

/// The menu open over the tab strip or a file's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabMenu {
    /// The menu of the tab of the open file at this position.
    Tab(usize),
    /// The menu of what can be done with the open file at this position.
    File(usize),
}

/// One of the two panes the session view splits into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    /// The left pane, which the tab strip's own tab shows in.
    #[default]
    Left,
    /// The right pane, shown while the view is split.
    Right,
}

/// What the tab strip or its menu asks of the open files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    /// Shows a tab.
    Select(SessionTab),
    /// Closes the file at this position.
    Close(usize),
    /// Closes every file but the one at this position.
    CloseOthers(usize),
    /// Closes the files right of the one at this position.
    CloseRight(usize),
    /// Closes every file.
    CloseAll,
    /// Keeps the file at this position open rather than as the preview.
    Keep(usize),
    /// Shows the next tab, round to the first.
    Next,
    /// Shows the previous tab, round to the last.
    Previous,
    /// Opens the file closed last again.
    Reopen,
    /// Shows a markdown file at this position rendered, or as text again.
    ToggleRendered(usize),
    /// Opens this menu, or closes the one open.
    Menu(Option<TabMenu>),
    /// Splits the view, showing the tab showing in the right pane as well.
    Split,
    /// Joins the panes again, keeping the left one.
    Unsplit,
    /// Shows the tab in the pane, splitting the view when it is not, as
    /// when a tab is dragged there.
    MoveTo(SessionTab, Pane),
    /// Makes the pane the one tabs chosen show in.
    Focus(Pane),
    /// Opens a tab that is not a file's, such as the release notes.
    Open(SessionTab),
    /// Closes a tab that is not a file's.
    Dismiss(SessionTab),
    /// Stops showing the comparison of the open file at this position.
    EndCompare(usize),
    /// Pins the file at this position, or unpins it.
    TogglePin(usize),
    /// Gives the left pane this share of the split view's width, in thousandths.
    Resize(u32),
    /// Answers the question asked before closing modified files.
    Unsaved(Unsaved),
}

/// An answer to the question asked before closing files with unsaved changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsaved {
    /// Saves the modified files, then closes them.
    Save,
    /// Closes the files, throwing their changes away.
    Discard,
    /// Keeps the files open.
    Cancel,
}

/// An open file as it is kept between runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SavedFile {
    /// The folder it belongs to.
    pub directory: String,
    /// Its path from the root of the folder.
    pub path: String,
    /// The line of its caret, counted from 1.
    pub line: usize,
    /// Its text, when it had changes that were not saved.
    pub unsaved: Option<String>,
}

/// The narrowest share of the split view either pane may take.
pub const PANE_SHARE_MIN: f32 = 0.15;

/// The files open in the session view and the tab chosen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenFiles {
    /// The open files, in the order of their tabs.
    pub files: Vec<OpenFile>,
    /// The tab showing.
    pub tab: SessionTab,
    /// The files closed lately, as folder and path, the latest last.
    pub closed: Vec<(String, String)>,
    /// Whether files are edited with vim's keys.
    pub vim: bool,
    /// The menu open over the tab strip or a file's header, when one is.
    pub menu: Option<TabMenu>,
    /// The tab the right pane shows, while the view is split.
    pub split: Option<SessionTab>,
    /// The pane tabs chosen show in.
    pub focus: Pane,
    /// Whether the tab of the editable excerpts of a search is open.
    pub excerpts_tab: bool,
    /// Whether the tab of the newer release's notes is open.
    pub update_tab: bool,
    /// The languages the reader declined to install, by name.
    pub declined: Vec<String>,
    /// The close waiting while the reader is asked what to do with the
    /// unsaved changes of the files it would close.
    pub asking: Option<TabAction>,
    /// The share of the split view's width the left pane takes, or 0 for half.
    pub share: f32,
}

impl OpenFiles {
    /// The open files as they are kept between runs: where each is, the
    /// line of its caret and its text when it has unsaved changes.
    pub fn saved(&self) -> Vec<SavedFile> {
        self.files
            .iter()
            .filter(|file| !matches!(file.body, FileBody::Failed(_)))
            .map(|file| {
                let text = (file.body == FileBody::Text)
                    .then(|| file.document.0.try_borrow().ok())
                    .flatten();
                let line = text
                    .as_ref()
                    .map(|held| held.line_of(held.field.caret()) + 1)
                    .or(file.line)
                    .unwrap_or(1);
                let unsaved = text
                    .filter(|held| held.is_dirty())
                    .map(|held| held.field.text().to_owned())
                    .or_else(|| file.restored.clone());
                SavedFile {
                    directory: file.directory.clone(),
                    path: file.path.clone(),
                    line,
                    unsaved,
                }
            })
            .collect()
    }

    /// Opens again the file `saved` describes in a kept tab, with its caret
    /// line and unsaved text put back once it is read; returns its position
    /// when it has to be read, being new.
    pub fn reopen(&mut self, saved: &SavedFile) -> Option<usize> {
        let index = self.open(&saved.directory, &saved.path, true)?;
        let file = &mut self.files[index];
        file.line = Some(saved.line);
        file.restored = saved.unsaved.clone();
        Some(index)
    }

    /// The position of the file at `path` in `directory`, when it is open.
    pub fn position(&self, directory: &str, path: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|file| file.directory == directory && file.path == path)
    }

    /// The file showing, when a file tab is chosen.
    pub fn shown(&self) -> Option<&OpenFile> {
        match self.tab {
            SessionTab::File(index) => self.files.get(index),
            _ => None,
        }
    }

    /// Shows the file at `path` in `directory`, in the preview tab unless
    /// `keep`; returns the position of a tab that has to be read, being new.
    pub fn open(&mut self, directory: &str, path: &str, keep: bool) -> Option<usize> {
        if let Some(index) = self.position(directory, path) {
            if keep {
                self.files[index].preview = false;
            }
            self.tab = SessionTab::File(index);
            return None;
        }
        let file = OpenFile {
            directory: directory.to_owned(),
            path: path.to_owned(),
            preview: !keep,
            ..OpenFile::default()
        };
        let replaced = self
            .files
            .iter()
            .position(|held| held.preview && !held.pinned && !held.is_dirty());
        let index = match replaced {
            Some(index) => {
                self.files[index] = file;
                index
            }
            None => {
                self.files.push(file);
                self.files.len() - 1
            }
        };
        self.tab = SessionTab::File(index);
        Some(index)
    }

    /// Carries out `action`; returns the folder and path of a file to read
    /// again when it opened one closed earlier.
    pub fn act(&mut self, action: TabAction) -> Option<(String, String)> {
        if !self.unsaved_closing(action).is_empty() {
            self.asking = Some(action);
            self.menu = None;
            return None;
        }
        match action {
            TabAction::Select(tab) => self.show(tab),
            TabAction::Close(_)
            | TabAction::CloseOthers(_)
            | TabAction::CloseRight(_)
            | TabAction::CloseAll => {
                let pinned = self.pinned_set();
                self.close_where(|at| Self::closes(action, at, &pinned));
            }
            TabAction::Unsaved(answer) => {
                let held = self.asking.take();
                if let (Some(close), Unsaved::Save | Unsaved::Discard) = (held, answer) {
                    let pinned = self.pinned_set();
                    self.close_where(|at| Self::closes(close, at, &pinned));
                }
            }
            TabAction::TogglePin(index) => self.toggle_pin(index),
            TabAction::Resize(thousandths) => {
                let share = thousandths as f32 / 1000.0;
                self.share = share.clamp(PANE_SHARE_MIN, 1.0 - PANE_SHARE_MIN);
            }
            TabAction::Keep(index) => {
                if let Some(file) = self.files.get_mut(index) {
                    file.preview = false;
                }
            }
            TabAction::Next => self.step(true),
            TabAction::Previous => self.step(false),
            TabAction::Reopen => {
                let (directory, path) = self.closed.pop()?;
                return self
                    .open(&directory, &path, true)
                    .map(|_| (directory, path));
            }
            TabAction::ToggleRendered(index) => {
                if let Some(file) = self.files.get_mut(index) {
                    file.rendered = !file.rendered;
                }
            }
            TabAction::Menu(menu) => self.menu = menu,
            TabAction::Split => {
                self.split = Some(self.tab);
                self.focus = Pane::Right;
            }
            TabAction::Unsplit => {
                self.split = None;
                self.focus = Pane::Left;
            }
            TabAction::MoveTo(tab, pane) => {
                self.focus = pane;
                match pane {
                    Pane::Left => self.tab = tab,
                    Pane::Right => self.split = Some(tab),
                }
            }
            TabAction::Focus(pane) => self.focus = pane,
            TabAction::Open(tab) => {
                match tab {
                    SessionTab::Excerpts => self.excerpts_tab = true,
                    SessionTab::Update => self.update_tab = true,
                    _ => {}
                }
                self.show(tab);
            }
            TabAction::Dismiss(tab) => {
                match tab {
                    SessionTab::Excerpts => self.excerpts_tab = false,
                    SessionTab::Update => self.update_tab = false,
                    _ => {}
                }
                if self.tab == tab {
                    self.tab = SessionTab::Chat;
                }
                if self.split == Some(tab) {
                    self.split = Some(SessionTab::Chat);
                }
            }
            TabAction::EndCompare(index) => {
                if let Some(file) = self.files.get_mut(index) {
                    file.compare = None;
                }
            }
        }
        if !matches!(action, TabAction::Menu(_)) {
            self.menu = None;
        }
        None
    }

    /// Whether `action` closes the file at position `at`, given the positions of the `pinned` files, which only Close itself closes.
    fn closes(action: TabAction, at: usize, pinned: &[usize]) -> bool {
        match action {
            TabAction::Close(index) => at == index,
            TabAction::CloseOthers(index) => at != index && !pinned.contains(&at),
            TabAction::CloseRight(index) => at > index && !pinned.contains(&at),
            TabAction::CloseAll => !pinned.contains(&at),
            _ => false,
        }
    }

    /// The positions of the modified files `action` would close.
    pub fn unsaved_closing(&self, action: TabAction) -> Vec<usize> {
        let pinned = self.pinned_set();
        self.files
            .iter()
            .enumerate()
            .filter(|(at, file)| Self::closes(action, *at, &pinned) && file.is_dirty())
            .map(|(at, _)| at)
            .collect()
    }

    /// The share of the split view's width the left pane takes.
    pub fn left_share(&self) -> f32 {
        if self.share <= 0.0 { 0.5 } else { self.share }
    }

    /// The positions of the pinned files.
    fn pinned_set(&self) -> Vec<usize> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, file)| file.pinned)
            .map(|(at, _)| at)
            .collect()
    }

    /// Pins the file at `index`, moving it after the pinned files, or unpins it, moving it after them too.
    fn toggle_pin(&mut self, index: usize) {
        let Some(file) = self.files.get_mut(index) else {
            return;
        };
        file.pinned = !file.pinned;
        file.preview = false;
        let pinned = file.pinned;
        let others = self
            .files
            .iter()
            .enumerate()
            .filter(|(at, file)| *at != index && file.pinned)
            .count();
        let to = if pinned {
            others
        } else {
            others.min(self.files.len() - 1)
        };
        self.move_file(index, to);
    }

    /// Moves the file at `from` to `to`, keeping the tabs shown in either pane on the same files.
    fn move_file(&mut self, from: usize, to: usize) {
        if from == to || from >= self.files.len() || to >= self.files.len() {
            return;
        }
        let file = self.files.remove(from);
        self.files.insert(to, file);
        let moved = |at: usize| {
            if at == from {
                to
            } else if from < at && at <= to {
                at - 1
            } else if to <= at && at < from {
                at + 1
            } else {
                at
            }
        };
        if let SessionTab::File(at) = self.tab {
            self.tab = SessionTab::File(moved(at));
        }
        if let Some(SessionTab::File(at)) = self.split {
            self.split = Some(SessionTab::File(moved(at)));
        }
    }

    /// Stops offering to install the language called `name`.
    pub fn decline(&mut self, name: &str) {
        if !self.declined.iter().any(|held| held == name) {
            self.declined.push(name.to_owned());
        }
    }

    /// Shows `tab` in the pane tabs chosen show in.
    fn show(&mut self, tab: SessionTab) {
        match (self.focus, self.split) {
            (Pane::Right, Some(_)) => self.split = Some(tab),
            _ => self.tab = tab,
        }
    }

    /// The tab the right pane shows, while the view is split, kept to a
    /// file that is still open.
    pub fn right_tab(&self) -> Option<SessionTab> {
        self.split.map(|tab| match tab {
            SessionTab::File(index) if index >= self.files.len() => SessionTab::Chat,
            tab => tab,
        })
    }

    /// The tabs in strip order: the conversation, the diff, each file, then
    /// the excerpts and the release notes when their tabs are open.
    fn tabs(&self) -> Vec<SessionTab> {
        [SessionTab::Chat, SessionTab::Diff]
            .into_iter()
            .chain((0..self.files.len()).map(SessionTab::File))
            .chain(self.excerpts_tab.then_some(SessionTab::Excerpts))
            .chain(self.update_tab.then_some(SessionTab::Update))
            .collect()
    }

    /// Shows the tab after the one showing when `forward`, otherwise the one before it.
    fn step(&mut self, forward: bool) {
        let tabs = self.tabs();
        let current = match (self.focus, self.split) {
            (Pane::Right, Some(tab)) => tab,
            _ => self.tab,
        };
        let at = tabs.iter().position(|tab| *tab == current).unwrap_or(0);
        let next = if forward {
            (at + 1) % tabs.len()
        } else {
            (at + tabs.len() - 1) % tabs.len()
        };
        self.show(tabs[next]);
    }

    /// Closes the files whose positions `closes` picks, remembering them to
    /// open again, and keeps a sensible tab showing.
    fn close_where(&mut self, closes: impl Fn(usize) -> bool) {
        let shown = match self.tab {
            SessionTab::File(index) => Some(index),
            _ => None,
        };
        let right = match self.split {
            Some(SessionTab::File(index)) => Some(index),
            _ => None,
        };
        let mut right_now = None;
        let mut kept = Vec::new();
        let mut shown_now = None;
        let mut nearest = None;
        for (index, file) in std::mem::take(&mut self.files).into_iter().enumerate() {
            if closes(index) {
                self.closed.push((file.directory, file.path));
                continue;
            }
            if Some(index) == shown {
                shown_now = Some(kept.len());
            }
            if Some(index) == right {
                right_now = Some(kept.len());
            }
            if shown.is_some_and(|at| index < at) || nearest.is_none() {
                nearest = Some(kept.len());
            }
            kept.push(file);
        }
        let excess = self.closed.len().saturating_sub(CLOSED_KEPT);
        self.closed.drain(..excess);
        self.files = kept;
        if right.is_some() {
            self.split = Some(right_now.map_or(SessionTab::Chat, SessionTab::File));
        }
        if shown.is_some() {
            self.tab = match shown_now.or(nearest) {
                Some(index) => SessionTab::File(index),
                None => SessionTab::Chat,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Files opened as previews take each other's tab; a kept one stays.
    #[test]
    fn previews_replace_each_other() {
        let mut files = OpenFiles::default();
        assert_eq!(files.open("d", "a.rs", false), Some(0));
        assert_eq!(files.open("d", "b.rs", false), Some(0));
        assert_eq!(files.files.len(), 1);
        files.act(TabAction::Keep(0));
        assert_eq!(files.open("d", "c.rs", false), Some(1));
        assert_eq!(files.files.len(), 2);
        assert_eq!(files.tab, SessionTab::File(1));
    }

    /// Closing a file remembers it, and reopening brings it back.
    #[test]
    fn closing_and_reopening() {
        let mut files = OpenFiles::default();
        files.open("d", "a.rs", true);
        files.open("d", "b.rs", true);
        files.act(TabAction::Close(1));
        assert_eq!(files.tab, SessionTab::File(0));
        assert_eq!(
            files.act(TabAction::Reopen),
            Some(("d".to_owned(), "b.rs".to_owned()))
        );
        assert_eq!(files.files.len(), 2);
        files.act(TabAction::CloseAll);
        assert_eq!(files.tab, SessionTab::Chat);
    }

    /// A pinned tab moves first and stays open through Close others and Close all, and the panes keep their files.
    #[test]
    fn pinned_tabs_stay() {
        let mut files = OpenFiles::default();
        files.open("d", "a.rs", true);
        files.open("d", "b.rs", true);
        files.open("d", "c.rs", true);
        files.act(TabAction::TogglePin(2));
        assert_eq!(files.files[0].path, "c.rs");
        assert_eq!(files.tab, SessionTab::File(0));
        files.act(TabAction::CloseOthers(1));
        let left: Vec<&str> = files.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(left, vec!["c.rs", "a.rs"]);
        files.act(TabAction::CloseAll);
        assert_eq!(files.files.len(), 1);
        assert_eq!(files.open("d", "d.rs", false), Some(1));
    }

    /// Closing a file keeps the right pane on the file it showed.
    #[test]
    fn right_pane_keeps_its_file() {
        let mut files = OpenFiles::default();
        files.open("d", "a.rs", true);
        files.open("d", "b.rs", true);
        files.act(TabAction::MoveTo(SessionTab::File(1), Pane::Right));
        files.act(TabAction::Close(0));
        assert_eq!(files.split, Some(SessionTab::File(0)));
        files.act(TabAction::Resize(990));
        assert_eq!(files.left_share(), 1.0 - PANE_SHARE_MIN);
    }

    /// Splitting shows the tab in the right pane, where chosen tabs then
    /// show, and a tab dropped on a pane moves there.
    #[test]
    fn splitting_and_moving_tabs() {
        let mut files = OpenFiles::default();
        files.open("d", "a.rs", true);
        files.open("d", "b.rs", true);
        files.act(TabAction::Split);
        assert_eq!(files.right_tab(), Some(SessionTab::File(1)));
        files.act(TabAction::Select(SessionTab::Chat));
        assert_eq!(files.right_tab(), Some(SessionTab::Chat));
        assert_eq!(files.tab, SessionTab::File(1));
        files.act(TabAction::MoveTo(SessionTab::File(0), Pane::Left));
        assert_eq!(files.tab, SessionTab::File(0));
        assert_eq!(files.focus, Pane::Left);
        files.act(TabAction::Unsplit);
        assert_eq!(files.right_tab(), None);
    }

    /// Two texts compare as a diff naming the changed line, and the same
    /// texts compare as the same.
    #[test]
    fn comparing_texts() {
        let comparison = Comparison::between("Clipboard", "a\nb\n", "a\nc\n");
        assert!(comparison.diff.contains("-b") && comparison.diff.contains("+c"));
        assert!(Comparison::between("Saved", "a\n", "a\n").is_same());
    }

    /// Closing a modified file asks first; cancelling keeps it, and
    /// discarding closes it.
    #[test]
    fn closing_modified_files_asks() {
        let mut files = OpenFiles::default();
        files.open("d", "a.rs", true);
        files.files[0].take_text("one\n", false, None);
        files.files[0].document.0.borrow_mut().mark_unsaved();
        assert!(files.files[0].is_dirty());
        files.act(TabAction::Close(0));
        assert_eq!(files.asking, Some(TabAction::Close(0)));
        assert_eq!(files.files.len(), 1);
        files.act(TabAction::Unsaved(Unsaved::Cancel));
        assert_eq!((files.asking, files.files.len()), (None, 1));
        files.act(TabAction::CloseAll);
        files.act(TabAction::Unsaved(Unsaved::Discard));
        assert!(files.files.is_empty());
    }

    /// Stepping through the tabs goes round.
    #[test]
    fn stepping_goes_round() {
        let mut files = OpenFiles::default();
        files.open("d", "a.rs", true);
        files.act(TabAction::Next);
        assert_eq!(files.tab, SessionTab::Chat);
        files.act(TabAction::Previous);
        assert_eq!(files.tab, SessionTab::File(0));
    }
}
