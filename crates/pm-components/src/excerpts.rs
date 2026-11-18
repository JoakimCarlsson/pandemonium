//! Lines of files shown as one document, one under another, each edited in place in the file it comes from: the lines around the hits of a search, the lines a set of changes touched, or a file set against one of the states it was saved in.

use crate::kit::sp;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use std::ops::Range;

use crate::kit::{
    FocusId, IconName, Interactive, Radius, Role, Scroll, Side, Styled, Tone, Tooltip, Type, label,
    label_with, scroll_area,
};
use crate::marked::{Mark, Paint};
use pm_protocol::Location;
use rustle::{CursorIcon, Modifier};

use crate::file_icon::file_icon;
use crate::open_files::{Contents, EDITOR_FIELD, Viewer};
use crate::parts::glyph;
use crate::sample::DiffLine;
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::viewer::{
    BREAKPOINT_CELL, Beside, breakpoint_cell, document_row, file_line, line_marks,
};
use crate::{Button, Tally};

/// The tab of the excerpts in a pane's strip.
pub const TAB_EXCERPTS: FocusId = FocusId(7_410_000);

/// Closes the excerpts.
pub const EXCERPTS_CLOSE: FocusId = FocusId(7_410_001);

/// The scrolling box of the excerpts.
pub const EXCERPTS_SCROLL: FocusId = FocusId(7_410_002);

/// Puts the file shown back as it was in the state it is set against.
pub const EXCERPTS_RESTORE: FocusId = FocusId(7_410_003);

/// Folds every file of the excerpts away, or opens them all when they are all folded.
pub const EXCERPTS_FOLD_ALL: FocusId = FocusId(7_410_004);

/// The first of the headers that open a file, each following it by the position of the file in the excerpts.
pub const EXCERPTS_FILE_BASE: u64 = 7_411_000;

/// The first of the boxes holding the lines of a file that is not the one being edited, each following it by the position of the file among the open files; a press on one makes that file the one edited.
pub const EXCERPT_OWNER_BASE: u64 = 7_412_000;

/// One past the last box holding lines of a file that is not the one being edited.
const EXCERPT_OWNER_END: u64 = 7_413_000;

/// Takes everything the turn shown changed back out of the files.
pub const EXCERPTS_UNDO_ALL: FocusId = FocusId(7_410_005);

/// The first of the buttons taking what the turn shown changed back out of one file, each following it by the position of the file in the excerpts.
pub const EXCERPT_UNDO_BASE: u64 = 7_413_000;

/// One past the last button taking a turn's changes back out of one file.
const EXCERPT_UNDO_END: u64 = 7_414_000;

/// How many lines are shown on each side of a hit.
const CONTEXT: usize = 2;

/// How tall the space under each excerpt is, in logical pixels.
const GAP: f32 = 12.0;

/// How far beyond the view, above and below, excerpts are still built, in logical pixels.
const OVERSCAN: f32 = 200.0;

/// A run of lines of a file shown together, and the lines in it that are hits.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The first line shown, counted from 0.
    pub first: usize,
    /// The last line shown.
    pub last: usize,
    /// The lines that hold a hit, or that a change added, in the order of the file.
    pub hits: Vec<usize>,
    /// The lines a change took out, each run shown above the line of the file that follows it, which is counted from 0.
    pub removed: Vec<(usize, Vec<String>)>,
}

/// How many lines a block stands in for while its file is not read yet.
const WAITING_LINES: usize = 40;

impl Block {
    /// A block of every line of a file, whatever its length.
    pub(crate) fn whole() -> Self {
        Self {
            first: 0,
            last: usize::MAX - 1,
            hits: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// How many lines the change took out that this block shows.
    fn removed_lines(&self) -> usize {
        self.removed.iter().map(|(_, lines)| lines.len()).sum()
    }
}

/// The excerpts of one file.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcerptFile {
    /// The file, from the root of the project.
    pub path: String,
    /// The runs of lines, in the order of the file.
    pub blocks: Vec<Block>,
    /// Whether the file is folded away, leaving its header.
    pub closed: bool,
    /// The letter the file's change is marked with, when it is a change.
    pub mark: Option<&'static str>,
    /// How many lines the change put in and took out, when it is a change.
    pub tally: (usize, usize),
    /// Whether the file is in the folder to be edited, which a file a change deleted is not.
    pub editable: bool,
    /// The lines of the file's diff when it is shown from the diff alone, which a commit's files are.
    pub diff: Vec<DiffLine>,
}

impl ExcerptFile {
    /// A file shown by the runs of lines `blocks`, with nothing else known about it.
    pub fn new(path: String, blocks: Vec<Block>) -> Self {
        Self {
            path,
            blocks,
            closed: false,
            mark: None,
            tally: (0, 0),
            editable: true,
            diff: Vec::new(),
        }
    }
}

/// What the excerpts show.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// The lines around the hits of a search, the hits marked within them.
    Search {
        /// What was searched for, which names the tab and marks the hits.
        text: String,
        /// Whether upper and lower case had to match.
        case: bool,
        /// Whether the text was a regular expression.
        regex: bool,
    },
    /// A file set against one of the states it was saved in.
    Timeline {
        /// How long ago the state was saved, as it is said.
        when: String,
        /// What the file held in that state.
        text: String,
    },
    /// The files a commit or a branch changed, or the agent during one turn, shown from their diffs alone.
    Revision {
        /// What the view is called.
        label: String,
        /// The turn whose changes these are, by how many of the reader's turns came before it, when they are a turn's: its changes can be taken back out of the files.
        turn: Option<u64>,
    },
    /// A file a merge, a rebase or a cherry-pick stopped on, with the sides to take from.
    Conflict(Box<crate::conflict::Resolver>),
}

/// The excerpts being shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Excerpts {
    /// What they show.
    pub source: Source,
    /// The files, in the order they are listed.
    pub files: Vec<ExcerptFile>,
    /// How far down it is scrolled, in logical pixels.
    pub scroll: f32,
}

impl Excerpts {
    /// What the tab of the excerpts says.
    pub fn title(&self) -> String {
        match &self.source {
            Source::Search { text, .. } => format!("Search: {text}"),
            Source::Timeline { .. } => {
                let name = self.files.first().map_or("", |file| name_of(&file.path));
                format!("Timeline: {name}")
            }
            Source::Revision { label, .. } => label.clone(),
            Source::Conflict(resolver) => format!("Resolve: {}", name_of(&resolver.path)),
        }
    }

    /// The glyph of the tab of the excerpts.
    pub fn icon(&self) -> IconName {
        match &self.source {
            Source::Search { .. } => IconName::Search,
            Source::Revision { .. } => IconName::GitCompare,
            Source::Timeline { .. } => IconName::History,
            Source::Conflict(_) => IconName::GitFork,
        }
    }

    /// Whether the headers of the files fold them away rather than going to the file.
    pub fn folds(&self) -> bool {
        !matches!(self.source, Source::Search { .. })
    }
}

/// The name of the file at `path`.
fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The runs of lines around the hits at `lines`, in order, runs that touch joined into one.
fn blocks_of(lines: &[usize]) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for &hit in lines {
        let first = hit.saturating_sub(CONTEXT);
        let last = hit + CONTEXT;
        match blocks.last_mut() {
            Some(block) if first <= block.last + 1 => {
                block.last = block.last.max(last);
                block.hits.push(hit);
            }
            _ => blocks.push(Block {
                first,
                last,
                hits: vec![hit],
                removed: Vec::new(),
            }),
        }
    }
    blocks
}

/// Whether `id` is the box of lines of a file that is not the one being edited.
pub fn is_excerpt_owner(id: FocusId) -> bool {
    (EXCERPT_OWNER_BASE..EXCERPT_OWNER_END).contains(&id.0)
}

/// Whether `id` is something of the excerpts the state handles when it is pressed.
pub fn is_excerpt_part(id: FocusId) -> bool {
    id == TAB_EXCERPTS
        || id == EXCERPTS_CLOSE
        || id == EXCERPTS_RESTORE
        || id == EXCERPTS_FOLD_ALL
        || id == EXCERPTS_UNDO_ALL
        || (EXCERPTS_FILE_BASE..EXCERPT_UNDO_END).contains(&id.0)
        || crate::conflict::is_conflict_part(id)
}

impl WorkspaceState {
    /// The position among the open files of the file `path`, read from the folder.
    pub(crate) fn viewer_at(&self, path: &str) -> Option<usize> {
        self.git
            .viewers
            .iter()
            .position(|viewer| viewer.path == path && !viewer.head)
    }

    /// Opens the hits of the search of the review panel as excerpts, reading each file that is not open, and shows them.
    pub fn excerpts_opened(mut self) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let search = &self.project_search;
        let (text, case, regex) = (search.query.text().to_owned(), search.case, search.regex);
        let files: Vec<ExcerptFile> = search
            .groups()
            .iter()
            .map(|group| {
                ExcerptFile::new(
                    group.path.clone(),
                    blocks_of(
                        &group
                            .hits
                            .iter()
                            .map(|hit| hit.span.start.line as usize)
                            .collect::<Vec<_>>(),
                    ),
                )
            })
            .collect();
        if files.is_empty() {
            return self;
        }
        self.excerpts_shown(session, Source::Search { text, case, regex }, files);
        self
    }

    /// Shows `files` as excerpts of `source`, closing the excerpts shown before, and reads each file that is not open and that is in the folder to be edited.
    pub(crate) fn excerpts_shown(
        &mut self,
        session: usize,
        source: Source,
        files: Vec<ExcerptFile>,
    ) {
        if self.excerpts.is_some() {
            *self = std::mem::replace(self, Self::new(&[])).excerpts_closed();
        }
        for file in files.iter().filter(|file| file.editable) {
            self.load_hidden_viewer(session, &file.path);
        }
        self.git.shown = files
            .iter()
            .filter(|file| file.editable)
            .find_map(|file| self.viewer_at(&file.path));
        self.excerpts = Some(Excerpts {
            source,
            files,
            scroll: 0.0,
        });
        self.tab = Tab::Excerpts;
    }

    /// Closes the excerpts: the files read only for them are closed, and those with changes that were not saved become tabs of their own.
    pub fn excerpts_closed(mut self) -> Self {
        if self.excerpts.take().is_none() {
            return self;
        }
        while let Some(index) = self
            .git
            .viewers
            .iter()
            .position(|viewer| viewer.hidden && !viewer.is_dirty())
        {
            self.discard_file(index);
        }
        for viewer in &mut self.git.viewers {
            viewer.hidden = false;
        }
        if self.tab == Tab::Excerpts {
            self.tab = Tab::Chat;
        }
        self
    }

    /// Whether the excerpts are open.
    pub fn excerpts_open(&self) -> bool {
        self.excerpts.is_some()
    }

    /// The state after the file at `index` among the open files was made the one edited, which is what a press on its lines does.
    pub fn excerpt_file_shown(mut self, index: usize) -> Self {
        if index < self.git.viewers.len() {
            self.git.shown = Some(index);
        }
        self
    }

    /// The position among the open files of the file whose box of lines `id` is.
    pub fn excerpt_owner_of(&self, id: FocusId) -> Option<usize> {
        is_excerpt_owner(id)
            .then(|| (id.0 - EXCERPT_OWNER_BASE) as usize)
            .filter(|index| *index < self.git.viewers.len())
    }

    /// Folds the file at `place` among the excerpts away, or opens it when it is folded.
    fn excerpt_file_folded(&mut self, place: usize) {
        if let Some(file) = self
            .excerpts
            .as_mut()
            .and_then(|excerpts| excerpts.files.get_mut(place))
        {
            file.closed = !file.closed;
        }
    }

    /// Folds every file of the excerpts away, or opens them all when every one is folded.
    fn excerpts_folded(&mut self) {
        if let Some(excerpts) = self.excerpts.as_mut() {
            let close = excerpts.files.iter().any(|file| !file.closed);
            for file in &mut excerpts.files {
                file.closed = close;
            }
        }
    }

    /// Carries out what pressing `id` of the excerpts does.
    pub(crate) fn excerpts_pressed(mut self, id: FocusId) -> Self {
        if crate::conflict::is_conflict_part(id) {
            return self.conflict_pressed(id);
        }
        if id == TAB_EXCERPTS {
            self.tab = Tab::Excerpts;
            return self;
        }
        if id == EXCERPTS_CLOSE {
            return self.excerpts_closed();
        }
        if id == EXCERPTS_RESTORE {
            return self.timeline_restored();
        }
        if id == EXCERPTS_FOLD_ALL {
            self.excerpts_folded();
            return self;
        }
        if id == EXCERPTS_UNDO_ALL {
            self.turn_undone(None);
            return self;
        }
        if (EXCERPT_UNDO_BASE..EXCERPT_UNDO_END).contains(&id.0) {
            self.turn_undone(Some((id.0 - EXCERPT_UNDO_BASE) as usize));
            return self;
        }
        if let Some(index) = self.excerpt_owner_of(id) {
            return self.excerpt_file_shown(index);
        }
        if !(EXCERPTS_FILE_BASE..EXCERPT_OWNER_BASE).contains(&id.0) {
            return self;
        }
        let place = (id.0 - EXCERPTS_FILE_BASE) as usize;
        if self.excerpts.as_ref().is_some_and(Excerpts::folds) {
            self.excerpt_file_folded(place);
            return self;
        }
        let target = self.excerpts.as_ref().and_then(|excerpts| {
            let file = excerpts.files.get(place)?;
            let first = file.blocks.first()?.hits.first().copied().unwrap_or(0);
            Some((file.path.clone(), first))
        });
        match (self.review_index(), target) {
            (Some(session), Some((path, line))) => {
                let position = pm_protocol::Position::new(line as u32, 0);
                self.go_to_location(
                    session,
                    Location {
                        path,
                        span: pm_protocol::Span::new(position, position),
                        inside: true,
                        ..Location::default()
                    },
                )
            }
            _ => self,
        }
    }

    /// Asks for what the turn whose changes are shown changed to be taken back out of the file at `place` of the excerpts, or out of every file when there is none, and closes the view, which no longer says what the files hold.
    fn turn_undone(&mut self, place: Option<usize>) {
        let (Some(session), Some(excerpts)) = (self.review_index(), self.excerpts.as_ref()) else {
            return;
        };
        let Source::Revision {
            turn: Some(turn), ..
        } = excerpts.source
        else {
            return;
        };
        let paths = match place {
            Some(place) => match excerpts.files.get(place) {
                Some(file) => vec![file.path.clone()],
                None => return,
            },
            None => Vec::new(),
        };
        self.outbox.push(crate::state::Outgoing::UndoTurn {
            session,
            turn,
            paths,
        });
        *self = std::mem::replace(self, Self::new(&[])).excerpts_closed();
    }

    /// Moves the excerpts of the file `path` to follow an edit that changed its number of lines by `delta` at line `at`, counted before the edit.
    pub(crate) fn excerpts_followed(&mut self, path: &str, at: usize, delta: isize) {
        let Some(file) = self
            .excerpts
            .as_mut()
            .and_then(|excerpts| excerpts.files.iter_mut().find(|file| file.path == path))
        else {
            return;
        };
        let shifted = |line: usize| (line as isize + delta).max(0) as usize;
        for block in &mut file.blocks {
            if block.last < at {
                continue;
            }
            if block.first > at {
                block.first = shifted(block.first);
                block.last = shifted(block.last);
                for hit in &mut block.hits {
                    *hit = shifted(*hit);
                }
                for (anchor, _) in &mut block.removed {
                    *anchor = shifted(*anchor);
                }
            } else {
                block.last = shifted(block.last).max(block.first);
                for hit in block.hits.iter_mut().filter(|hit| **hit > at) {
                    *hit = shifted(*hit);
                }
                for (anchor, _) in block.removed.iter_mut().filter(|(anchor, _)| *anchor > at) {
                    *anchor = shifted(*anchor);
                }
            }
        }
    }

    /// The state after the excerpts were scrolled by `delta` logical pixels, with `room` to scroll.
    pub fn scrolled_excerpts(mut self, delta: f32, room: f32) -> Self {
        if let Some(excerpts) = self.excerpts.as_mut() {
            excerpts.scroll = (excerpts.scroll + delta).clamp(0.0, room);
        }
        self
    }
}

/// How many lines of `viewer` the block shows: the lines it holds, up to the end of the file once that is read.
fn shown_lines(viewer: Option<&Viewer>, block: &Block) -> Range<usize> {
    let end = match viewer.map(|viewer| &viewer.contents) {
        Some(Contents::Text(document)) => (block.last + 1).min(document.lines()),
        _ => (block.last + 1).min(block.first + WAITING_LINES),
    };
    block.first.min(end)..end
}

/// How many rows the block shows, the lines of the file and the lines taken out of it.
fn block_rows(viewer: Option<&Viewer>, block: &Block) -> usize {
    shown_lines(viewer, block).len() + block.removed_lines()
}

/// Emits the header over the excerpts of one file: its icon and name, where it is, whether it has changes not saved and what it holds, with, for a change, how far it went.
#[track_caller]
fn header(
    position: usize,
    file: &ExcerptFile,
    viewer: Option<&Viewer>,
    excerpts: &Excerpts,
    size: Size,
) {
    let name = name_of(&file.path);
    let folder = file.path.strip_suffix(name).unwrap_or_default();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(header_height(size))
            .fill(Role::Surface)
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .pad_x(3),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new()
                    .weight(1.0)
                    .clip_to(Radius::None)
                    .cursor(CursorIcon::Pointer)
                    .target(FocusId(EXCERPTS_FILE_BASE + position as u64)),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    if excerpts.folds() {
                        glyph(
                            if file.closed {
                                IconName::ChevronRight
                            } else {
                                IconName::ChevronDown
                            },
                            3.5,
                            Role::Muted,
                        );
                    }
                    file_icon(name, 4.0);
                    label(
                        name,
                        Type::new(13)
                            .ratio(1.4)
                            .tone(Role::Text)
                            .truncate()
                            .medium(),
                    );
                    label_with(
                        Modifier::new().weight(1.0).clip_to(Radius::None),
                        folder,
                        Type::new(12).ratio(1.4).tone(Role::Subtle).truncate(),
                    );
                },
            );
            if viewer.is_some_and(Viewer::is_dirty) {
                glyph(IconName::Pencil, 3.0, Role::Muted);
            }
            match &excerpts.source {
                Source::Search { .. } => {
                    let hits: usize = file.blocks.iter().map(|block| block.hits.len()).sum();
                    label(
                        hits.to_string(),
                        Type::new(11).ratio(1.4).tone(Role::Muted).truncate(),
                    );
                }
                _ => {
                    if let Some(mark) = file.mark {
                        label(
                            mark,
                            Type::new(11).ratio(1.4).tone(mark_tone(mark)).truncate(),
                        );
                    }
                    Tally::new(file.tally.0, file.tally.1).size(11).build();
                    if matches!(excerpts.source, Source::Revision { turn: Some(_), .. }) {
                        Button::icon(IconName::Undo, FocusId(EXCERPT_UNDO_BASE + position as u64))
                            .size(Size::Sm)
                            .modifier(
                                Modifier::new()
                                    .tooltip("Take what this turn changed back out of this file"),
                            )
                            .build();
                    }
                }
            }
        },
    );
}

/// The colour of the letter a change is marked with.
fn mark_tone(mark: &str) -> Role {
    match mark {
        "A" | "U" => Role::Success,
        "D" | "C" => Role::Danger,
        _ => Role::Warning,
    }
}

/// Emits the button of the header of a timeline that puts the file back as it was in the state.
#[track_caller]
pub(crate) fn restore_button() {
    Button::new("Restore this version", EXCERPTS_RESTORE)
        .with_icon(IconName::History)
        .size(Size::Sm)
        .build();
}

/// What the view of the excerpts needs to draw the lines of one file.
struct Drawing<'a> {
    /// The state the lines are drawn from.
    state: &'a WorkspaceState,
    /// How the editor draws.
    settings: &'a pm_editor::EditorSettings,
    /// The matcher that marks the hits within their lines, for the excerpts of a search.
    matcher: Option<pm_editor::Query>,
}

/// One row of a block: a line of the file, or a line a change took out.
enum Row<'a> {
    /// The line of the file at this position.
    Line(usize),
    /// A line that was taken out.
    Removed(&'a str),
}

/// Emits a line a change took out, drawn under the gutter of the lines of the file so that the text stays in its column, tinted as taken out and not to be edited.
#[track_caller]
fn removed_row(content: &str, settings: &pm_editor::EditorSettings) {
    let height = settings.row_height();
    let numbers = if settings.line_numbers {
        Modifier::new().w(13)
    } else {
        Modifier::new().w(2)
    };
    let kind = Type::code(settings.font_size).ratio(1.4).tone(Role::Muted);
    let shown = crate::diff_syntax::shown(content);
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(height)
            .fill(Tone::from(Role::Danger).alpha(0.12))
            .clip_to(Radius::None),
        Arrangement::Start,
        Alignment::Center,
        move || {
            rustle::spacer(Modifier::new().width(BREAKPOINT_CELL + 3.0));
            rustle::spacer(numbers.pad_side(Side::Right, 3));
            label_with(
                Modifier::new().weight(1.0).clip_to(Radius::None).pad_x(2),
                shown,
                kind,
            );
        },
    );
}

/// Emits a box as tall as `rows` rows of `row` logical pixels, standing in for those left out.
#[track_caller]
fn blank(rows: usize, row: f32) {
    rustle::spacer(Modifier::new().height(rows as f32 * row));
}

impl Drawing<'_> {
    /// Emits the rows of `block` of the file open at `index`, the ones in `window` built from a block that starts `top` logical pixels down, in a box that a press makes the file edited, with the space under them.
    #[track_caller]
    fn block(&self, index: usize, block: &Block, top: f32, window: (f32, f32)) {
        let state = self.state;
        let row_height = state.editor.row_height();
        let Some(Contents::Text(document)) = state.git.viewers.get(index).map(|v| &v.contents)
        else {
            rustle::spacer(Modifier::new().height(GAP + row_height));
            return;
        };
        let live = state.git.shown == Some(index);
        let owner = if live {
            EDITOR_FIELD
        } else {
            FocusId(EXCERPT_OWNER_BASE + index as u64)
        };
        let found = if live {
            state.find_marks()
        } else {
            (Vec::new(), None)
        };
        let pair: Vec<Range<usize>> = document
            .syntax
            .as_ref()
            .filter(|_| live)
            .and_then(|syntax| syntax.bracket_pair(document.field.text(), document.field.caret()))
            .map(|(open, close)| vec![open, close])
            .unwrap_or_default();
        let gutter = state.gutter_of(index);
        let run = shown_lines(state.git.viewers.get(index), block);
        let mut rows: Vec<Row> = Vec::with_capacity(run.len() + block.removed_lines());
        for line in run.clone() {
            for (_, taken) in block.removed.iter().filter(|(anchor, _)| *anchor == line) {
                rows.extend(taken.iter().map(|text| Row::Removed(text)));
            }
            rows.push(Row::Line(line));
        }
        for (_, taken) in block
            .removed
            .iter()
            .filter(|(anchor, _)| *anchor >= run.end)
        {
            rows.extend(taken.iter().map(|text| Row::Removed(text)));
        }
        let first = (((window.0 - top) / row_height).floor().max(0.0) as usize).min(rows.len());
        let last = (((window.1 - top) / row_height).ceil().max(0.0) as usize).min(rows.len());
        let visible = &rows[first..last.max(first)];
        let (low, high) = visible
            .iter()
            .filter_map(|row| match row {
                Row::Line(line) => Some(*line),
                Row::Removed(_) => None,
            })
            .fold((usize::MAX, 0), |(low, high), line| {
                (low.min(line), high.max(line + 1))
            });
        let painted = if low < high {
            document.highlights(low..high)
        } else {
            Vec::new()
        };
        let draw = |row: &Row| match row {
            Row::Removed(text) => removed_row(text, self.settings),
            Row::Line(line) => {
                let spans = painted.get(line - low).cloned().unwrap_or_default();
                let mut marks = line_marks(document, *line, spans, &pair, &found, self.settings);
                let range = document.line(*line);
                let hit = block.hits.binary_search(line).is_ok();
                if hit
                    && let Some(matched) = self
                        .matcher
                        .as_ref()
                        .and_then(|matcher| matcher.first_in(&document.field.text()[range.clone()]))
                {
                    marks.push(Mark::new(matched, Role::SurfaceSelected, Paint::Fill));
                }
                let segments = vec![Range {
                    start: 0,
                    end: range.len(),
                }];
                let height = document_row(self.settings, &segments);
                let cell = || breakpoint_cell(index, *line, &gutter, height);
                let beside = Beside {
                    blamed: None,
                    breakpoint: cell,
                    bulb: false,
                    paused: gutter.paused == Some(*line),
                };
                let shown = || {
                    file_line(
                        document,
                        *line,
                        marks,
                        segments,
                        beside,
                        self.settings,
                        owner,
                    );
                };
                if hit && self.matcher.is_none() {
                    column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .fill(Tone::from(Role::Success).alpha(0.12)),
                        Arrangement::Start,
                        Alignment::Start,
                        shown,
                    );
                } else {
                    shown();
                }
            }
        };
        column_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::Start,
            Alignment::Start,
            || {
                column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .cursor(CursorIcon::Text)
                        .target(owner),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        blank(first, row_height);
                        for (at, shown) in visible.iter().enumerate() {
                            rustle::key(first + at, || draw(shown));
                        }
                        blank(rows.len() - last.max(first), row_height);
                    },
                );
                rustle::spacer(Modifier::new().height(GAP));
            },
        );
    }
}

/// One thing drawn in the excerpts, with its height.
enum Entry {
    /// The header of the file at this position in the excerpts.
    Header(usize),
    /// The block at the second position of the file at the first.
    Block(usize, usize),
}

/// Emits the lines of a file that is shown from its diff alone, as the rows of a diff, under its header.
#[track_caller]
fn diff_rows(file: &ExcerptFile) {
    let colours = crate::diff_syntax::DiffColours::of(
        &file.diff,
        crate::file_icon::file_language(&file.path).as_deref(),
    );
    for (at, diff) in file.diff.iter().enumerate() {
        crate::diff::diff_line(diff.kind, &diff.text, colours.line(at));
    }
}

/// How tall the rows of a diff are, in logical pixels.
const DIFF_ROW: f32 = 20.0;

/// Emits the excerpts in a view `height` logical pixels tall: a header over each file, and under it the runs of lines, only those near the view being built.
#[track_caller]
pub(crate) fn file_list(state: &WorkspaceState, height: f32, size: Size) {
    let Some(excerpts) = state.excerpts.as_ref() else {
        return;
    };
    let row = state.editor.row_height();
    let header_pixels = header_height(size);
    let mut entries: Vec<(Entry, f32)> = Vec::new();
    for (place, file) in excerpts.files.iter().enumerate() {
        let viewer = state
            .viewer_at(&file.path)
            .and_then(|i| state.git.viewers.get(i));
        entries.push((Entry::Header(place), header_pixels));
        if file.closed {
            continue;
        }
        if !file.diff.is_empty() {
            entries.push((
                Entry::Block(place, usize::MAX),
                file.diff.len() as f32 * DIFF_ROW + GAP,
            ));
            continue;
        }
        let viewer = viewer.filter(|_| file.editable);
        for (step, block) in file.blocks.iter().enumerate() {
            let rows = if file.editable {
                block_rows(viewer, block)
            } else {
                block.removed_lines()
            }
            .max(1);
            entries.push((Entry::Block(place, step), rows as f32 * row + GAP));
        }
    }
    let settings = pm_editor::EditorSettings {
        git_gutter: state.shows_gutter(),
        ..state.editor.clone()
    };
    let drawing = Drawing {
        state,
        settings: &settings,
        matcher: match &excerpts.source {
            Source::Search { text, case, regex } => Some(pm_editor::Query {
                text: text.clone(),
                case: *case,
                word: false,
                regex: *regex,
            }),
            _ => None,
        },
    };
    let (low, high) = (
        excerpts.scroll - OVERSCAN,
        excerpts.scroll + height + OVERSCAN,
    );
    let (mut before, mut after, mut top) = (0.0_f32, 0.0_f32, 0.0_f32);
    let mut built: Vec<(&Entry, f32, f32)> = Vec::new();
    for (entry, size) in &entries {
        let (start, end) = (top, top + size);
        top = end;
        if end < low {
            before += size;
            continue;
        }
        if start > high {
            after += size;
            continue;
        }
        built.push((entry, start, *size));
    }
    let draw = |entry: &Entry, start: f32, size: f32| match entry {
        Entry::Header(place) => {
            let file = &excerpts.files[*place];
            let viewer = state
                .viewer_at(&file.path)
                .and_then(|i| state.git.viewers.get(i));
            header(*place, file, viewer, excerpts, size_of(header_pixels));
        }
        Entry::Block(place, usize::MAX) => {
            let file = &excerpts.files[*place];
            column_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    diff_rows(file);
                    rustle::spacer(Modifier::new().height(GAP));
                },
            );
        }
        Entry::Block(place, step) => {
            let file = &excerpts.files[*place];
            match state.viewer_at(&file.path).filter(|_| file.editable) {
                Some(index) => drawing.block(index, &file.blocks[*step], start, (low, high)),
                None if !file.editable => gone_block(&file.blocks[*step], &settings),
                None => rustle::spacer(Modifier::new().height(size)),
            }
        }
    };
    column_arranged(
        Modifier::new()
            .weight(1.0)
            .clip_to(Radius::None)
            .fill(Role::Background),
        Arrangement::Start,
        Alignment::Start,
        || {
            scroll_area(
                Modifier::new().fill_max_width().weight(1.0),
                Scroll::from_start(EXCERPTS_SCROLL, excerpts.scroll),
                || {
                    column_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            rustle::spacer(Modifier::new().height(before));
                            for (entry, start, size) in built {
                                let identity = match entry {
                                    Entry::Header(place) => (*place, None),
                                    Entry::Block(place, step) => (*place, Some(*step)),
                                };
                                rustle::key(identity, || draw(entry, start, size));
                            }
                            rustle::spacer(Modifier::new().height(after));
                        },
                    );
                },
            );
        },
    );
}

/// How tall a header over a file is at `size`, in logical pixels.
pub(crate) const fn header_height(size: Size) -> f32 {
    match size {
        Size::Xs => 24.0,
        Size::Sm => 28.0,
        Size::Md => 32.0,
        Size::Lg => 36.0,
    }
}

/// The header size that is `height` logical pixels tall.
fn size_of(height: f32) -> Size {
    if height <= header_height(Size::Xs) {
        Size::Xs
    } else if height <= header_height(Size::Sm) {
        Size::Sm
    } else if height <= header_height(Size::Md) {
        Size::Md
    } else {
        Size::Lg
    }
}

/// Emits the lines a change took out of a file that is gone, drawn as taken out, with the space under them.
#[track_caller]
fn gone_block(block: &Block, settings: &pm_editor::EditorSettings) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for text in block.removed.iter().flat_map(|(_, lines)| lines.iter()) {
                removed_row(text, settings);
            }
            rustle::spacer(Modifier::new().height(GAP));
        },
    );
}

/// Emits the rows of `block` of the file open at `index` in the window, the ones in `window` built from a block that starts `top` logical pixels down, as the excerpts draw them but for the hits of a search.
#[track_caller]
pub(crate) fn plain_block(
    state: &WorkspaceState,
    index: usize,
    block: &Block,
    top: f32,
    window: (f32, f32),
) {
    let settings = pm_editor::EditorSettings {
        git_gutter: state.shows_gutter(),
        ..state.editor.clone()
    };
    Drawing {
        state,
        settings: &settings,
        matcher: None,
    }
    .block(index, block, top, window);
}

/// Emits the excerpts of a search in a view `height` logical pixels tall.
#[track_caller]
pub fn excerpts_view(state: &WorkspaceState, height: f32) {
    file_list(state, height, Size::default());
}
