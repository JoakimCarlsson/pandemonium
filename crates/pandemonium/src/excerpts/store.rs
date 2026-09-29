//! Which lines of which files the pane shows, and where its cursor is.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use pm_core::Change;
use pm_gfx::Point;
use pm_text::{Finder, Position, Query};

use crate::editor::{FileId, OpenFile};

/// How many unchanged lines an excerpt shows either side of a change.
const CONTEXT: usize = 3;

/// How many rows the pane keeps between the cursor and its top or bottom.
const MARGIN: usize = 3;

/// One worktree's excerpts, shared between the window and the pane drawing
/// them, as an open file is.
pub type OpenExcerpts = Rc<RefCell<Excerpts>>;

/// One changed file the pane shows excerpts of.
pub struct Excerpted {
    /// The open document the excerpts are windows onto.
    pub file: FileId,
    /// That document.
    pub document: OpenFile,
    /// Where the file sits in its worktree.
    pub name: String,
    /// What the last commit holds for it, when it holds anything.
    source: Source,
    /// Where the document differs from that, and at which version it was
    /// worked out.
    changes: (i32, Vec<Change>),
    /// Cached match ranges and the buffer version they came from.
    matches: (i32, Vec<Range<Position>>),
}

/// The reason a file appears in an excerpts pane.
enum Source {
    /// Lines changed relative to the last commit.
    Changes(Option<String>),
    /// Lines matched by a project search.
    Matches(Query),
}

impl Excerpted {
    /// The document of `file`, called `name`, compared against `committed`.
    pub fn new(file: FileId, document: OpenFile, name: String, committed: Option<String>) -> Self {
        Self {
            file,
            document,
            name,
            source: Source::Changes(committed),
            changes: (-1, Vec::new()),
            matches: (-1, Vec::new()),
        }
    }

    /// The document of `file`, called `name`, showing matches for `query`.
    pub fn matched(
        file: FileId,
        document: OpenFile,
        name: String,
        query: Query,
        ranges: Vec<Range<Position>>,
    ) -> Self {
        let version = document.borrow().buffer().version();
        Self {
            file,
            document,
            name,
            source: Source::Matches(query),
            changes: (-1, Vec::new()),
            matches: (version, ranges),
        }
    }

    /// The match ranges in this file, refreshed after each buffer edit.
    pub fn matches(&mut self) -> &[Range<Position>] {
        let Source::Matches(query) = &self.source else {
            return &[];
        };
        let document = self.document.borrow();
        let buffer = document.buffer();
        if self.matches.0 != buffer.version() {
            let mut found = Vec::new();
            if let Ok(finder) = Finder::new(query) {
                for line in 0..buffer.line_count() {
                    found.extend(
                        finder
                            .line(&buffer.line_text(line))
                            .into_iter()
                            .map(|range| {
                                Position::new(line, range.start)..Position::new(line, range.end)
                            }),
                    );
                }
            }
            self.matches = (buffer.version(), found);
        }
        &self.matches.1
    }

    /// Whether this file is shown for search matches rather than changes.
    pub fn is_match_source(&self) -> bool {
        matches!(self.source, Source::Matches(_))
    }

    /// Where the document differs from the last commit, worked out again
    /// only when it has been edited since the last time it was asked.
    pub fn changes(&mut self) -> &[Change] {
        let Source::Changes(committed) = &self.source else {
            return &[];
        };
        let document = self.document.borrow();
        let version = document.buffer().version();
        if self.changes.0 != version {
            let before = committed.as_deref().unwrap_or_default();
            self.changes = (
                version,
                pm_core::changes(before, &document.buffer().contents()),
            );
        }
        &self.changes.1
    }

    /// The runs of lines the pane shows of this file, in order and apart.
    ///
    /// Each change brings the lines it covers and [`CONTEXT`] either side;
    /// two whose context meets are one excerpt, because a line of context
    /// between two changes read twice is read once too many.
    pub fn ranges(&mut self) -> Vec<Range<usize>> {
        let count = self.document.borrow().buffer().line_count();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let (lines, context) = match self.source {
            Source::Changes(_) => (
                self.changes()
                    .iter()
                    .map(|change| change.lines.clone())
                    .collect::<Vec<_>>(),
                CONTEXT,
            ),
            Source::Matches(_) => (
                self.matches()
                    .iter()
                    .map(|found| found.start.line..found.end.line + 1)
                    .collect(),
                1,
            ),
        };
        for run in lines {
            let start = run.start.saturating_sub(context);
            let end = (run.end.max(run.start + 1) + context).min(count);
            match ranges.last_mut() {
                Some(last) if start <= last.end => last.end = last.end.max(end),
                _ => ranges.push(start..end.max(start)),
            }
        }
        ranges
    }

    /// How many lines the file adds and takes out against the last commit.
    pub fn counts(&mut self) -> (usize, usize) {
        if matches!(self.source, Source::Matches(_)) {
            return (self.matches().len(), 0);
        }
        self.changes()
            .iter()
            .fold((0, 0), |(added, removed), change| {
                (
                    added + change.lines.len(),
                    removed + change.removed.lines().count(),
                )
            })
    }
}

/// One row of the pane, in the order they are drawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Row {
    /// The heading of the file in this place of the list.
    Header(usize),
    /// A line of that file, by its number in the document.
    Line(usize, usize),
    /// A line the last commit held that the file no longer does.
    Removed(usize, String),
    /// The lines of that file left out between two of its excerpts.
    Gap(usize),
}

/// Every changed file of one worktree, as the excerpts a pane shows.
#[derive(Default)]
pub struct Excerpts {
    /// The changed files, in the order the review lists them.
    files: Vec<Excerpted>,
    /// The file holding the cursor, which is the one keystrokes go to.
    active: Option<FileId>,
    /// The first row the pane shows.
    scroll: usize,
    /// How many rows the pane last had room for.
    shown: usize,
    /// The cursor the pane last brought into view, so a wheel turned since
    /// is not undone by the next frame.
    followed: Option<(FileId, i32, Position)>,
    /// Where the cursor was when it was last inside an excerpt.
    settled: Option<(FileId, Position)>,
    /// Just under where the pane last drew the cursor, for what opens
    /// beside it — completions, a signature — to be placed against.
    caret: Option<Point>,
}

impl Excerpts {
    /// The changed files, in order.
    pub fn files(&self) -> &[Excerpted] {
        &self.files
    }

    /// The changed files, to work their changes out.
    pub fn files_mut(&mut self) -> &mut [Excerpted] {
        &mut self.files
    }

    /// Takes in the changed files as they now stand.
    ///
    /// The cursor stays in the file it was in while that file is still
    /// changed, and goes to the first file otherwise.
    pub fn set_files(&mut self, files: Vec<Excerpted>) {
        self.files = files;
        let kept = self
            .active
            .filter(|active| self.files.iter().any(|excerpted| excerpted.file == *active));
        self.active = kept.or_else(|| self.files.first().map(|excerpted| excerpted.file));
        self.followed = None;
    }

    /// Appends one file of streamed search results.
    pub fn push_file(&mut self, file: Excerpted) {
        if self.active.is_none() {
            self.active = Some(file.file);
        }
        self.files.push(file);
        self.followed = None;
    }

    /// The file holding the cursor.
    pub fn active(&self) -> Option<FileId> {
        self.active
    }

    /// Where in the list the file holding the cursor is.
    pub fn active_index(&self) -> Option<usize> {
        let active = self.active?;
        self.files
            .iter()
            .position(|excerpted| excerpted.file == active)
    }

    /// Puts the cursor in `file`, which keystrokes then go to.
    pub fn activate(&mut self, file: FileId) {
        if self.files.iter().any(|excerpted| excerpted.file == file) {
            self.active = Some(file);
        }
    }

    /// Every row of the pane, file by file and excerpt by excerpt.
    pub fn rows(&mut self) -> Vec<Row> {
        let mut rows = Vec::new();
        for index in 0..self.files.len() {
            let excerpted = &mut self.files[index];
            if excerpted.is_match_source() && excerpted.matches().is_empty() {
                continue;
            }
            rows.push(Row::Header(index));
            let ranges = excerpted.ranges();
            let changes = excerpted.changes().to_vec();
            let count = excerpted.document.borrow().buffer().line_count();
            for (at, range) in ranges.iter().enumerate() {
                if at > 0 {
                    rows.push(Row::Gap(index));
                }
                for line in range.clone() {
                    push_removed(&mut rows, index, &changes, line);
                    rows.push(Row::Line(index, line));
                }
                if range.end >= count {
                    push_removed(&mut rows, index, &changes, range.end);
                }
            }
        }
        if !rows.iter().any(|row| match row {
            Row::Header(index) => Some(self.files[*index].file) == self.active,
            _ => false,
        }) {
            self.active = rows.iter().find_map(|row| match row {
                Row::Header(index) => Some(self.files[*index].file),
                _ => None,
            });
        }
        rows
    }

    /// Just under where the pane last drew the cursor, if it drew it.
    pub fn caret(&self) -> Option<Point> {
        self.caret
    }

    /// Takes down where the pane drew the cursor this frame.
    pub fn set_caret(&mut self, caret: Option<Point>) {
        self.caret = caret;
    }

    /// The first row the pane shows.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Scrolls `rows` down, or up when negative, as far as there are rows.
    pub fn scroll_by(&mut self, rows: isize) {
        let total = self.rows().len();
        let last = total.saturating_sub(self.shown.max(1) / 2);
        self.scroll = self.scroll.saturating_add_signed(rows).min(last);
    }

    /// Takes down how many rows the pane had room for, and brings the cursor
    /// into view if it has moved since the pane last did.
    pub fn follow(&mut self, rows: &[Row], shown: usize) {
        self.shown = shown.max(1);
        let Some(active) = self.active else {
            return;
        };
        let Some(index) = self.active_index() else {
            return;
        };
        let (version, head) = {
            let document = self.files[index].document.borrow();
            (
                document.buffer().version(),
                document.buffer().selection().head,
            )
        };
        if self.followed == Some((active, version, head)) {
            return;
        }
        self.followed = Some((active, version, head));
        let Some(row) = rows
            .iter()
            .position(|row| *row == Row::Line(index, head.line))
        else {
            return;
        };
        let margin = MARGIN.min(self.shown.saturating_sub(1) / 2);
        if row < self.scroll + margin {
            self.scroll = row.saturating_sub(margin);
        } else if row + margin >= self.scroll + self.shown {
            self.scroll = (row + margin + 1).saturating_sub(self.shown);
        }
    }

    /// Where the cursor should go when it has left every excerpt of its
    /// file: the next excerpt down when it moved down, the one above when it
    /// moved up, whichever file that is in.
    ///
    /// A cursor inside an excerpt stays where it is and is written down as
    /// settled there; one that has nowhere to go stays where it is too.
    pub fn resettle(&mut self) -> Option<(FileId, Position)> {
        let index = self.active_index()?;
        let file = self.files[index].file;
        let head = self.files[index]
            .document
            .borrow()
            .buffer()
            .selection()
            .head;
        let ranges = self.files[index].ranges();
        if ranges.iter().any(|range| range.contains(&head.line)) {
            self.settled = Some((file, head));
            return None;
        }

        let down = match self.settled {
            Some((settled, at)) if settled == file => head.line >= at.line,
            _ => true,
        };
        let stops = self.stops();
        let here = (index, head.line);
        let target = match down {
            true => stops.iter().find(|(at, range)| (*at, range.start) > here),
            false => stops
                .iter()
                .rev()
                .find(|(at, range)| (*at, range.end.saturating_sub(1)) < here),
        }
        .or_else(|| stops.iter().rev().find(|(at, _)| *at == index))
        .or_else(|| stops.first())?;

        let (at, range) = target.clone();
        let line = match down {
            true => range.start,
            false => range.end.saturating_sub(1),
        };
        let target = (self.files[at].file, Position::new(line, head.column));
        self.active = Some(target.0);
        self.settled = Some(target);
        Some(target)
    }

    /// Every excerpt of every file, in the order the pane draws them.
    fn stops(&mut self) -> Vec<(usize, Range<usize>)> {
        (0..self.files.len())
            .flat_map(|index| {
                self.files[index]
                    .ranges()
                    .into_iter()
                    .map(move |range| (index, range))
            })
            .collect()
    }
}

/// Adds the rows of every line a change took out just before `line`.
fn push_removed(rows: &mut Vec<Row>, index: usize, changes: &[Change], line: usize) {
    for change in changes.iter().filter(|change| change.lines.start == line) {
        rows.extend(
            change
                .removed
                .lines()
                .map(|removed| Row::Removed(index, removed.to_owned())),
        );
    }
}
