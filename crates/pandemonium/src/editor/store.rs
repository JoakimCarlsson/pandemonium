//! The files the window has open, whichever pane is showing them.
//!
//! A file is opened once per worktree: the same path in two worktrees is two
//! documents, because it is two worktrees. Which pane shows which of them is
//! the pane tree's business — this is only where the documents live, and the
//! one seam a file is opened, edited, saved and closed through, so the
//! language server hears about every change exactly once.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use pm_core::{Blame, Change, ProjectId, Scope};
use pm_gfx::Point;
use pm_text::{Buffer, Client, Indent, Position, Server, Servers};

use crate::editor::layout::TextLayout;
use crate::editor::search::Search;

/// How many lines of context the view keeps above and below the cursor.
pub const SCROLL_MARGIN: usize = 2;

/// How many columns of context the view keeps left and right of the cursor.
const SCROLL_MARGIN_X: usize = 4;

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
///
/// What the tab is drawn like beyond this — which icon it wears, whether the
/// pane keeps it through a change of worktree — is the pane's, because the
/// same file is one document and as many tabs as there are panes showing it.
pub struct FileEntry {
    /// What the tab calls it: the file's own name.
    pub name: String,
    /// Whether it has changes that are not on disk.
    pub dirty: bool,
    /// Whether it is only being previewed, and will give its tab up.
    pub preview: bool,
}

/// How the reader writes a file: how it indents one that does not say,
/// and what it tidies as it is saved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Habits {
    /// How a file that does not say is indented, and how wide a tab is.
    pub indent: Indent,
    /// Whether the space at the ends of lines goes when a file is saved.
    pub trim_whitespace: bool,
    /// Whether a saved file always ends in a line break.
    pub final_newline: bool,
}

/// One open file: its buffer, where the pane is looking, and who serves it.
pub struct Document {
    /// The text and everything the editor knows about it.
    buffer: Buffer,
    /// The first line the pane shows.
    scroll: usize,
    /// How far that line is scrolled up past the top of the pane, in logical
    /// pixels, never as much as a whole line.
    offset: f32,
    /// Where the cursor was when the view last followed it, so the view is
    /// brought back to the cursor when it moves and left alone when only the
    /// view does, or when the text changes under a reader scrolled away.
    followed: Option<Position>,
    /// The first column the pane shows, the text being scrolled left by it.
    column: usize,
    /// Where the pane last drew the text.
    layout: TextLayout,
    /// What is being looked for in the file, and where it was found.
    search: Search,
    /// Whether the file is only being looked at, not kept open.
    ///
    /// A previewed file holds the one preview tab of the pane it was opened
    /// in and gives it up to the next file previewed there. Editing it, or
    /// asking for it a second time, is what keeps it.
    preview: bool,
    /// The language servers this file is open in.
    servers: Vec<Arc<Client>>,
    /// What the index holds for this file, when git knows about it.
    baseline: Option<String>,
    /// Where the file differs from that, and at which version it was worked out.
    changes: (i32, Vec<Change>),
    /// Who last changed each line, once it has been asked for.
    blame: Vec<Blame>,
    /// Whether the blame column is being drawn.
    blame_shown: bool,
    /// The version the server was last asked what to write into the lines.
    hinted: Option<i32>,
    /// The version the server was last asked what the names in the file are.
    named: Option<i32>,
    /// The version the server was last asked for the notes above declarations.
    lensed: Option<i32>,
    /// The version and cursor the server was last asked where the symbol is used.
    used: Option<(i32, Position)>,
    /// The runs of lines that are folded away, in the order they appear.
    folded: Vec<std::ops::Range<usize>>,
    /// The mode modal editing has the file in, and the keys gathered towards
    /// a command.
    modal: pm_vim::State,
}

impl Document {
    /// Opens `buffer`, telling every one of `servers` that it is open.
    fn new(
        buffer: Buffer,
        preview: bool,
        servers: Vec<Arc<Client>>,
        baseline: Option<String>,
    ) -> Self {
        if let Some(language) = buffer.language() {
            for server in &servers {
                server.did_open(
                    buffer.path(),
                    language.language_id(),
                    buffer.version(),
                    &buffer.contents(),
                );
            }
        }

        Self {
            baseline,
            changes: (-1, Vec::new()),
            blame: Vec::new(),
            blame_shown: false,
            hinted: None,
            named: None,
            lensed: None,
            used: None,
            folded: Vec::new(),
            buffer,
            scroll: 0,
            offset: 0.0,
            followed: None,
            column: 0,
            layout: TextLayout::default(),
            search: Search::default(),
            preview,
            servers,
            modal: pm_vim::State::default(),
        }
    }

    /// A document holding nothing, called `name`, that is not on disk.
    ///
    /// A commit message is edited in the same editor a file is — the same
    /// cursor, the same selection, the same undo — so it is the same kind of
    /// document, with no file behind it and no server to tell about it.
    pub fn scratch(name: &str) -> Self {
        Self::new(Buffer::holding(name, ""), false, Vec::new(), None)
    }

    /// The text and everything the editor knows about it.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// The buffer, to read a highlight or a line out of while painting.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Where the file differs from what the index holds.
    ///
    /// The comparison is made against the version it was last made at, so a
    /// frame that has not been typed into since the last one costs nothing.
    /// A file git has never heard of has nothing to differ from, and is
    /// marked nowhere rather than marked new from end to end.
    pub fn changes(&mut self) -> &[Change] {
        let Some(baseline) = self.baseline.clone() else {
            return &[];
        };
        let version = self.buffer.version();
        if self.changes.0 != version {
            self.changes = (
                version,
                pm_core::changes(&baseline, &self.buffer.contents()),
            );
        }
        &self.changes.1
    }

    /// Whether git knows anything about this file at all.
    pub fn is_tracked(&self) -> bool {
        self.baseline.is_some()
    }

    /// The runs of lines folded away, in the order they appear.
    pub fn folds(&self) -> &[std::ops::Range<usize>] {
        &self.folded
    }

    /// Whether `line` is hidden inside a fold.
    pub fn is_folded(&self, line: usize) -> bool {
        self.folded.iter().any(|fold| fold.contains(&line))
    }

    /// Whether the fold under `line` is closed.
    pub fn is_folded_at(&self, line: usize) -> bool {
        self.folded.iter().any(|fold| fold.start == line + 1)
    }

    /// Folds what `line` holds, or unfolds it when it is already folded.
    pub fn toggle_fold(&mut self, line: usize) {
        if let Some(index) = self.folded.iter().position(|fold| fold.start == line + 1) {
            self.folded.remove(index);
            return;
        }
        let Some(fold) = self.buffer.fold_at(line) else {
            return;
        };
        self.folded.push(fold);
        self.folded.sort_by_key(|fold| fold.start);
    }

    /// Folds every line that holds something.
    pub fn fold_all(&mut self) {
        self.folded = self.buffer.folds();
        self.folded.sort_by_key(|fold| fold.start);
    }

    /// Unfolds everything.
    pub fn unfold_all(&mut self) {
        self.folded.clear();
    }

    /// Unfolds whatever is hiding `line`, so the cursor can be seen.
    pub fn reveal(&mut self, line: usize) {
        self.folded.retain(|fold| !fold.contains(&line));
    }

    /// The line `rows` rows below `line`, folded lines skipped.
    ///
    /// Everything that counts lines on the screen rather than in the file
    /// goes through here, so a buffer with nothing folded counts exactly as
    /// it always did.
    pub fn line_after(&self, line: usize, rows: isize) -> usize {
        let last = self.buffer.line_count().saturating_sub(1);
        let step = if rows >= 0 { 1isize } else { -1 };
        let mut at = line;
        let mut left = rows.abs();

        while left > 0 {
            let Some(next) = at.checked_add_signed(step).filter(|next| *next <= last) else {
                break;
            };
            at = next;
            if !self.is_folded(at) {
                left -= 1;
            }
        }
        at
    }

    /// How many rows below `from` the line `line` is drawn, if it is drawn.
    pub fn row_of(&self, from: usize, line: usize) -> Option<usize> {
        if line < from || self.is_folded(line) {
            return None;
        }
        Some((from..line).filter(|line| !self.is_folded(*line)).count())
    }

    /// The line drawn `row` rows below `from`.
    pub fn line_at_row(&self, from: usize, row: usize) -> usize {
        self.line_after(from, row as isize)
    }

    /// The lines drawn from `from` down, as many as `rows` has room for.
    pub fn drawn_lines(&self, from: usize, rows: usize) -> Vec<usize> {
        let last = self.buffer.line_count();
        (from..last)
            .filter(|line| !self.is_folded(*line))
            .take(rows)
            .collect()
    }

    /// Whether the server should be asked again what to write into the lines.
    ///
    /// Asking is the window's to do and answering the server's; the document
    /// only says whether what it is showing is still what was asked about.
    pub fn wants_hints(&mut self) -> bool {
        let version = self.buffer.version();
        if self.servers.is_empty() || self.hinted == Some(version) {
            return false;
        }
        self.hinted = Some(version);
        true
    }

    /// Takes every hint out of the file, and forgets it asked for them.
    fn forget_hints(&mut self) {
        self.buffer.set_hints(Vec::new());
        self.hinted = None;
    }

    /// Whether the server should be asked again what the names in the file are.
    pub fn wants_semantics(&mut self) -> bool {
        let version = self.buffer.version();
        if self.servers.is_empty() || self.named == Some(version) {
            return false;
        }
        self.named = Some(version);
        true
    }

    /// Whether the server should be asked again for the notes above declarations.
    pub fn wants_lenses(&mut self) -> bool {
        let version = self.buffer.version();
        if self.servers.is_empty() || self.lensed == Some(version) {
            return false;
        }
        self.lensed = Some(version);
        true
    }

    /// Takes every note above a declaration out, and forgets it asked for them.
    fn forget_lenses(&mut self) {
        self.buffer.set_lenses(Vec::new());
        self.lensed = None;
    }

    /// Whether the server should be asked again where the symbol at the cursor
    /// is used.
    ///
    /// Nothing is asked while text is selected: the selection lights up the
    /// other places it appears itself, and two answers to one question would
    /// be drawn over each other.
    pub fn wants_uses(&mut self) -> bool {
        let selection = self.buffer.selection();
        let asked = (self.buffer.version(), selection.head);
        if self.servers.is_empty() || !selection.is_empty() || self.used == Some(asked) {
            return false;
        }
        self.used = Some(asked);
        true
    }

    /// Who last changed each line, as far as it has been asked for.
    pub fn blame(&self) -> &[Blame] {
        &self.blame
    }

    /// Whether the blame column is being drawn.
    pub fn is_blamed(&self) -> bool {
        self.blame_shown
    }

    /// Draws the blame column, or stops drawing it.
    pub fn show_blame(&mut self, shown: bool) {
        self.blame_shown = shown;
    }

    /// Takes in who last changed each line.
    pub fn set_blame(&mut self, blame: Vec<Blame>) {
        self.blame = blame;
    }

    /// Reads again what the index holds for this file.
    pub fn reread_baseline(&mut self, root: &Path) {
        self.baseline = pm_core::baseline(root, self.buffer.path());
        self.changes = (-1, Vec::new());
    }

    /// What is being looked for in the file, and where it was found.
    pub fn search(&self) -> &Search {
        &self.search
    }

    /// The language servers this file is open in.
    pub fn servers(&self) -> Vec<Arc<Client>> {
        self.servers.clone()
    }

    /// Whether any language server is open on this file.
    pub fn is_served(&self) -> bool {
        !self.servers.is_empty()
    }

    /// Puts the search through `change`, against the text as it stands.
    pub fn search_with(&mut self, change: impl FnOnce(&mut Search, &Buffer)) {
        change(&mut self.search, &self.buffer);
    }

    /// The first line the pane shows.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// How far the first line is scrolled up past the top of the pane.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// The first column the pane shows.
    pub fn column(&self) -> usize {
        self.column
    }

    /// Where the pane last drew the text.
    pub fn layout(&self) -> TextLayout {
        self.layout
    }

    /// Takes down where the pane drew the text this frame.
    pub fn set_layout(&mut self, layout: TextLayout) {
        self.layout = layout;
    }

    /// The place in the file `point` fell on, as the pane last drew it.
    pub fn position_at(&self, point: Point) -> Position {
        let line = self.line_at_row(self.scroll, self.layout.row_at(point));
        let column = self.layout.column_at(point);
        self.buffer
            .clamped(self.buffer.position_at_display(line, column))
    }

    /// The character in the file `point` is over, when it is over one.
    ///
    /// Nothing is over the blank to the right of a line or under the last
    /// one: a question asked about the place a caret would be clamped to is
    /// a question about a name the reader is not pointing at.
    pub fn position_under(&self, point: Point) -> Option<Position> {
        let row = self.layout.row_at(point);
        let line = self.line_at_row(self.scroll, row);
        if line >= self.buffer.line_count() || self.row_of(self.scroll, line) != Some(row) {
            return None;
        }
        let at = self
            .buffer
            .position_at_display(line, self.layout.column_under(point));
        (at.column < self.buffer.line_len(line)).then_some(at)
    }

    /// Where on screen `position` was drawn, as the pane last drew it.
    pub fn point_of(&self, position: Position) -> Point {
        let row = self
            .row_of(self.scroll, position.line)
            .unwrap_or_else(|| position.line.saturating_sub(self.scroll));
        Point::new(
            self.layout.x_of(self.buffer.display_column(position)),
            self.layout.top_at(row),
        )
    }

    /// Shows the file from `line` down, as far as there is file to show.
    pub fn scroll_to(&mut self, line: usize) {
        let last = self.buffer.line_count().saturating_sub(1);
        self.scroll = line.min(last);
        self.offset = 0.0;
    }

    /// Scrolls so that the cursor's line has `rows` rows above it, as near
    /// as the margin kept around the cursor allows.
    pub fn scroll_cursor_to(&mut self, rows: usize) {
        let shown = self.rows();
        let margin = SCROLL_MARGIN.min(shown.saturating_sub(1) / 2);
        let rows = rows.clamp(margin, shown.saturating_sub(margin + 1).max(margin));
        let head = self.buffer.selection().head.line;
        self.scroll_to(self.line_after(head, -(rows as isize)));
    }

    /// Scrolls `lines` down, or up when `lines` is negative.
    pub fn scroll_by(&mut self, lines: isize) {
        self.scroll_to(self.line_after(self.scroll, lines));
    }

    /// Scrolls `pixels` logical pixels down, or up when `pixels` is negative,
    /// in lines as tall as the pane last drew them.
    ///
    /// The view comes to rest part of the way through a line, the way a
    /// trackpad moves it; only the first line and the last one hold it to a
    /// whole line, because there is nothing to show beyond either.
    pub fn scroll_by_pixels(&mut self, pixels: f32) {
        let line = self.layout.cell.height.max(1.0);
        let reach = self.offset + pixels;
        let rows = (reach / line).floor();
        self.scroll_to(self.line_after(self.scroll, rows as isize));
        let last = self.buffer.line_count().saturating_sub(1);
        let pinned = (self.scroll == 0 && reach < 0.0) || self.scroll >= last;
        self.offset = match pinned {
            true => 0.0,
            false => reach - rows * line,
        };
    }

    /// Scrolls `columns` right, or left when `columns` is negative.
    pub fn scroll_columns(&mut self, columns: isize) {
        self.scroll_to_column(self.column.saturating_add_signed(columns));
    }

    /// Shows the lines from `column` across.
    pub fn scroll_to_column(&mut self, column: usize) {
        self.column = column;
    }

    /// Goes to the match after the one being looked at, wrapping around.
    pub fn search_next(&mut self) -> Option<std::ops::Range<Position>> {
        self.search.next()
    }

    /// Goes to the match before the one being looked at, wrapping around.
    pub fn search_previous(&mut self) -> Option<std::ops::Range<Position>> {
        self.search.previous()
    }

    /// How many lines the pane last had room for.
    pub fn rows(&self) -> usize {
        self.layout.rows()
    }

    /// Brings the cursor back into view, having drawn `rows` by `columns`.
    ///
    /// Only the pane knows how tall a line came out, so this is where the
    /// view is told from: a keypress moves the cursor without knowing
    /// whether the place it moved to is on screen, and the next frame brings
    /// it back.
    ///
    /// Only a cursor that moved is followed: a view the wheel moved away
    /// from the cursor stays where the wheel left it.
    pub fn follow_cursor(&mut self, rows: usize, columns: usize) {
        let head = self.buffer.selection().head;
        if self.followed == Some(head) {
            return;
        }
        self.followed = Some(head);
        self.reveal(head.line);

        let margin = SCROLL_MARGIN.min(rows.saturating_sub(1) / 2);
        let first = self.line_after(head.line, -(margin as isize));
        let last = self.line_after(head.line, margin as isize);
        if first < self.scroll || (first == self.scroll && self.offset > 0.0) {
            self.scroll_to(first);
        } else if rows > 0 && self.row_of(self.scroll, last).is_none_or(|row| row >= rows) {
            self.scroll_to(self.line_after(last, 1 - rows as isize));
        }

        let drawn = self.buffer.display_column(head);
        let margin = SCROLL_MARGIN_X.min(columns.saturating_sub(1) / 2);
        let left = drawn.saturating_sub(margin);
        let right = drawn + margin;
        if left < self.column {
            self.column = left;
        } else if columns > 0 && right >= self.column + columns {
            self.column = right + 1 - columns;
        }
    }

    /// Puts the cursor and the view back where a launch left them.
    ///
    /// Where the pane was looking is as much a part of an open file as the
    /// text is: a window that comes back with every file scrolled to the top
    /// has not come back.
    pub fn restore(&mut self, line: usize, column: usize, scroll: usize) {
        self.buffer.place(Position::new(line, column), false);
        self.scroll_to(scroll);
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
        self.edit_modal(|buffer, _| edit(buffer));
    }

    /// The mode modal editing has the file in.
    pub fn modal(&self) -> &pm_vim::State {
        &self.modal
    }

    /// Applies `edit` to the buffer and the modal state beside it, as
    /// [`Self::edit`] does to the buffer alone, answering what `edit` did.
    pub fn edit_modal<R>(&mut self, edit: impl FnOnce(&mut Buffer, &mut pm_vim::State) -> R) -> R {
        let version = self.buffer.version();
        let result = edit(&mut self.buffer, &mut self.modal);
        if version == self.buffer.version() {
            return result;
        }
        self.preview = false;
        self.changed();
        result
    }

    /// Brings the search and the servers up to the text as it now stands.
    fn changed(&mut self) {
        if self.search.is_open() {
            self.search.refresh(&self.buffer);
        }
        let contents = self.buffer.contents();
        for server in &self.servers {
            server.did_change(self.buffer.path(), self.buffer.version(), &contents);
        }
    }

    /// Tidies the file the way `habits` say, writes it to disk and tells
    /// the server it was written.
    ///
    /// Only a file with changes is tidied: saving everything must not
    /// rewrite a file nobody touched because it was untidy when it opened.
    pub fn save(&mut self, habits: Habits) {
        if self.buffer.is_dirty() {
            if habits.trim_whitespace {
                self.edit(Buffer::trim_trailing_whitespace);
            }
            if habits.final_newline {
                self.edit(Buffer::ensure_final_newline);
            }
        }
        for server in &self.servers {
            server.will_save(self.buffer.path());
        }
        if self.buffer.save().is_err() {
            return;
        }
        let contents = self.buffer.contents();
        for server in &self.servers {
            server.did_save(self.buffer.path(), &contents);
        }
    }

    /// Reads this document from disk again, when it has nothing unsaved.
    ///
    /// Whatever wrote the file — a branch change, an agent, a formatter run
    /// from a shell — the servers hear the new text the way they hear an
    /// edit, and the comparison against the index is made again. A document
    /// with unsaved changes keeps them: the reader's edits are not something
    /// a write they did not see may throw away. Answers whether it changed.
    fn reread(&mut self, root: &Path) -> bool {
        if self.buffer.is_dirty() {
            return false;
        }
        let version = self.buffer.version();
        let changed = self.buffer.reread().unwrap_or(false);
        if version != self.buffer.version() {
            self.changed();
        }
        self.baseline = pm_core::baseline(root, self.buffer.path());
        self.changes = (-1, Vec::new());
        if changed {
            let lines = self.buffer.line_count();
            self.folded.retain(|fold| fold.end <= lines);
            self.blame.clear();
            self.blame_shown = false;
        }
        changed
    }

    /// Takes in what the servers have last said about this file.
    ///
    /// What they say is added together: a type checker and a linter both
    /// have squiggles to draw, and neither one's replace the other's.
    fn refresh(&mut self) {
        if self.servers.is_empty() {
            return;
        }
        let faults = self
            .servers
            .iter()
            .flat_map(|server| server.diagnostics(self.buffer.path()))
            .collect();
        self.buffer.set_diagnostics(faults);
    }
}

impl Drop for Document {
    /// Tells every server the file is no longer open.
    fn drop(&mut self) {
        for server in &self.servers {
            server.did_close(self.buffer.path());
        }
    }
}

/// One open file: the worktree it belongs to and the document itself.
struct Entry {
    /// The worktree the file was opened from.
    scope: Scope,
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
    /// How the reader writes the files.
    habits: Habits,
}

impl Files {
    /// Wakes the window through `notify` when a server has something to say.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.servers.set_notify(notify);
    }

    /// Runs `overrides` for the languages they name, in place of the usual.
    pub fn set_language_servers(&mut self, overrides: &BTreeMap<String, Vec<Server>>) {
        let named = overrides
            .iter()
            .map(|(language, servers)| (&*language.clone().leak(), servers.clone()))
            .collect();
        self.servers.set_overrides(named);
    }

    /// Writes files the way `habits` say from now on, the open ones
    /// included.
    pub fn set_habits(&mut self, habits: Habits) {
        if habits.indent != self.habits.indent {
            for entry in self.open.values() {
                entry
                    .document
                    .borrow_mut()
                    .buffer_mut()
                    .set_habit(habits.indent);
            }
        }
        self.habits = habits;
    }

    /// Takes every hint out of the open files, and forgets they were asked
    /// for, so they are asked for again once hints are wanted.
    pub fn forget_hints(&mut self) {
        for entry in self.open.values() {
            entry.document.borrow_mut().forget_hints();
        }
    }

    /// Takes every note above a declaration out of the open files, and
    /// forgets they were asked for, so they are asked for again once wanted.
    pub fn forget_lenses(&mut self) {
        for entry in self.open.values() {
            entry.document.borrow_mut().forget_lenses();
        }
    }

    /// Every language server running over the worktree at `root`.
    pub fn servers_over(&self, root: &Path) -> Vec<Arc<Client>> {
        self.servers.over(root)
    }

    /// Opens `path` in `scope`, or hands back the file if it is open already.
    ///
    /// A file that cannot be read does not open and does not complain: the
    /// tree lists what is on disk, and a directory entry that turns out not
    /// to be a readable file is the tree's business, not the store's.
    pub fn open(
        &mut self,
        scope: Scope,
        root: &Path,
        path: &Path,
        preview: bool,
    ) -> Option<FileId> {
        if let Some(id) = self.find(scope, path) {
            if !preview {
                self.keep(id);
            }
            return Some(id);
        }

        let mut buffer = Buffer::open(path).ok()?;
        buffer.set_habit(self.habits.indent);
        let servers = buffer
            .language()
            .map(|language| self.servers.open(root, language))
            .unwrap_or_default();

        let id = self.next;
        self.next = FileId(id.0 + 1);
        self.open.insert(
            id,
            Entry {
                scope,
                document: Rc::new(RefCell::new(Document::new(
                    buffer,
                    preview,
                    servers,
                    pm_core::baseline(root, path),
                ))),
            },
        );
        Some(id)
    }

    /// The file `path` is open as in `scope`, if it is open at all.
    fn find(&self, scope: Scope, path: &Path) -> Option<FileId> {
        self.open
            .iter()
            .find(|(_, entry)| {
                entry.scope == scope && entry.document.borrow().buffer().path() == path
            })
            .map(|(id, _)| *id)
    }

    /// The file `path` is open as in `scope`, whether or not it is open.
    pub fn opened(&self, scope: Scope, path: &Path) -> Option<FileId> {
        self.find(scope, path)
    }

    /// The document `id` names, if it is still open.
    pub fn get(&self, id: FileId) -> Option<OpenFile> {
        self.open.get(&id).map(|entry| entry.document.clone())
    }

    /// The worktree the file `id` names was opened from.
    pub fn scope_of(&self, id: FileId) -> Option<Scope> {
        self.open.get(&id).map(|entry| entry.scope)
    }

    /// The file `id` names as a bar of tabs presents it.
    pub fn entry(&self, id: FileId) -> Option<FileEntry> {
        let document = self.open.get(&id)?.document.borrow();
        Some(FileEntry {
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

    /// Writes the file `id` names to disk, in the worktree at `root`.
    ///
    /// Saving is when the index is read again: what a file is compared
    /// against only changes when git is given something to change it with,
    /// and writing the file is the moment that becomes possible.
    pub fn save(&mut self, id: FileId, root: &Path) {
        if let Some(entry) = self.open.get(&id) {
            let mut document = entry.document.borrow_mut();
            document.save(self.habits);
            document.reread_baseline(root);
        }
    }

    /// Writes every open file to disk, each against its own worktree.
    pub fn save_all(&mut self, root: &dyn Fn(Scope) -> Option<PathBuf>) {
        for entry in self.open.values() {
            let mut document = entry.document.borrow_mut();
            document.save(self.habits);
            if let Some(root) = root(entry.scope) {
                document.reread_baseline(&root);
            }
        }
    }

    /// Whether `project` has an open document whose edits are not on disk.
    pub fn project_is_dirty(&self, project: ProjectId) -> bool {
        self.open.values().any(|entry| {
            entry.scope.project() == project && entry.document.borrow().buffer().is_dirty()
        })
    }

    /// Reads every clean open document of `scope` from its changed worktree.
    pub fn reload_project(&mut self, scope: Scope, root: &Path) {
        self.reread(scope, root, |_| true);
    }

    /// Reads the clean open documents of `scope` at `paths` from disk again,
    /// answering whether any of them changed.
    pub fn reread_paths(&mut self, scope: Scope, root: &Path, paths: &BTreeSet<PathBuf>) -> bool {
        self.reread(scope, root, |path| paths.contains(path))
    }

    /// Reads the clean open documents of `scope` that `wanted` picks from
    /// disk again, answering whether any of them changed.
    fn reread(&mut self, scope: Scope, root: &Path, wanted: impl Fn(&Path) -> bool) -> bool {
        let mut changed = false;
        for entry in self.open.values() {
            if entry.scope != scope {
                continue;
            }
            let mut document = entry.document.borrow_mut();
            if wanted(document.buffer().path()) {
                changed |= document.reread(root);
            }
        }
        changed
    }

    /// Tells the servers over `root` what changed on disk under it.
    pub fn watched(&self, root: &Path, changes: &[(PathBuf, pm_text::Watched)]) {
        self.servers.watched(root, changes);
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
        self.open
            .retain(|_, entry| entry.scope.project() != project);
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
