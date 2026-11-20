//! Lines of files shown as one document, one under another, each edited in place in the file it comes from: the lines around the hits of a search, the lines a set of changes touched, or a file set against one of the states it was saved in.

use crate::areas::EditorEvent;
use crate::areas::SearchEvent;
use crate::button::Variant;
use crate::kit::{self, Frame};

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::{Icon, phosphor};

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crate::events::Interactive;
use crate::marked::{Mark, Paint};
use crate::theme::Role;
use pm_protocol::Location;
use rustle::{CursorIcon, Modifier};

use crate::file_icon::file_icon;
use crate::open_files::{Contents, Viewer};
use crate::parts::glyph;
use crate::sample::DiffLine;
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::viewer::{
    BREAKPOINT_CELL, Beside, breakpoint_cell, document_row, file_line, line_marks,
};
use crate::{Button, Tally};

/// How many lines are shown on each side of a hit.
const CONTEXT: usize = 2;

/// How tall the space under each excerpt is, in logical pixels.
const GAP: f32 = 12.0;

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
    /// The list of the headers and lines, which says how far down it is scrolled.
    pub scroll: rustle::LazyListState,
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
    pub fn icon(&self) -> Icon {
        match &self.source {
            Source::Search { .. } => phosphor::MAGNIFYING_GLASS,
            Source::Revision { .. } => phosphor::GIT_DIFF,
            Source::Timeline { .. } => phosphor::CLOCK_COUNTER_CLOCKWISE,
            Source::Conflict(_) => phosphor::GIT_FORK,
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

/// Whether `id` is something of the excerpts the state handles when it is pressed.
pub fn is_excerpt_part(id: Target) -> bool {
    matches!(
        id,
        Target::Search(
            SearchEvent::TabExcerpts
                | SearchEvent::ExcerptsClose
                | SearchEvent::ExcerptsRestore
                | SearchEvent::ExcerptsFoldAll
                | SearchEvent::ExcerptsUndoAll
                | SearchEvent::ExcerptsFile(_)
                | SearchEvent::ExcerptOwner(_)
                | SearchEvent::ExcerptUndo(_)
        )
    ) || crate::conflict::is_conflict_part(id)
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
            scroll: rustle::LazyListState::default(),
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

    /// The state after the file at `index` among the open files was made the one edited, which is what a press on its lines does.
    pub fn excerpt_file_shown(mut self, index: usize) -> Self {
        if index < self.git.viewers.len() {
            self.git.shown = Some(index);
        }
        self
    }

    /// The position among the open files of the file whose box of lines `id` is.
    pub fn excerpt_owner_of(&self, id: Target) -> Option<usize> {
        match id {
            Target::Search(SearchEvent::ExcerptOwner(index)) if index < self.git.viewers.len() => {
                Some(index)
            }
            _ => None,
        }
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
    pub(crate) fn excerpts_pressed(mut self, id: Target) -> Self {
        if crate::conflict::is_conflict_part(id) {
            return self.conflict_pressed(id);
        }
        let place = match id {
            Target::Search(SearchEvent::TabExcerpts) => {
                self.tab = Tab::Excerpts;
                return self;
            }
            Target::Search(SearchEvent::ExcerptsClose) => return self.excerpts_closed(),
            Target::Search(SearchEvent::ExcerptsRestore) => return self.timeline_restored(),
            Target::Search(SearchEvent::ExcerptsFoldAll) => {
                self.excerpts_folded();
                return self;
            }
            Target::Search(SearchEvent::ExcerptsUndoAll) => {
                self.turn_undone(None);
                return self;
            }
            Target::Search(SearchEvent::ExcerptUndo(turn)) => {
                self.turn_undone(Some(turn));
                return self;
            }
            Target::Search(SearchEvent::ExcerptsFile(place)) => place,
            _ => {
                return match self.excerpt_owner_of(id) {
                    Some(index) => self.excerpt_file_shown(index),
                    None => self,
                };
            }
        };
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
    kit::row(
        Modifier::new()
            .fill_max_width()
            .height(header_height(size))
            .background(theme::color(Role::Surface))
            .ruled(theme::Side::Bottom)
            .padding_xy(theme::sp(3), 0.0),
        2,
        || {
            kit::row(
                kit::pressed(
                    Modifier::new().weight(1.0).clip(),
                    Target::Search(SearchEvent::ExcerptsFile(position)),
                ),
                2,
                || {
                    if excerpts.folds() {
                        glyph(kit::chevron(!file.closed), 3.5, Role::Muted);
                    }
                    file_icon(name, 4.0);
                    kit::text(Modifier::new(), name, kit::strong(13, Role::Text));
                    kit::text(
                        Modifier::new().weight(1.0).clip(),
                        folder,
                        kit::line(12, Role::Subtle),
                    );
                },
            );
            if viewer.is_some_and(Viewer::is_dirty) {
                glyph(phosphor::PENCIL_SIMPLE, 3.0, Role::Muted);
            }
            match &excerpts.source {
                Source::Search { .. } => {
                    let hits: usize = file.blocks.iter().map(|block| block.hits.len()).sum();
                    kit::text(
                        Modifier::new(),
                        hits.to_string(),
                        kit::line(11, Role::Muted),
                    );
                }
                _ => {
                    if let Some(mark) = file.mark {
                        kit::text(Modifier::new(), mark, kit::line(11, mark_tone(mark)));
                    }
                    Tally::new(file.tally.0, file.tally.1).size(11).build();
                    if matches!(excerpts.source, Source::Revision { turn: Some(_), .. }) {
                        Button::icon(
                            phosphor::ARROW_COUNTER_CLOCKWISE,
                            Target::Search(SearchEvent::ExcerptUndo(position)),
                        )
                        .tooltip("Take what this turn changed back out of this file")
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
pub(crate) fn restore_button() {
    kit::small_button(
        "Restore this version",
        Target::Search(SearchEvent::ExcerptsRestore),
        Variant::Default,
    )
    .with_icon(phosphor::CLOCK_COUNTER_CLOCKWISE)
    .build();
}

/// What the lines of one open file are drawn with, worked out once for every line of it in view.
struct Context {
    /// The field whose caret and selection the lines hold.
    owner: Target,
    /// The places find found in the file, and the one chosen.
    found: (Vec<Range<usize>>, Option<usize>),
    /// The brackets paired at the caret.
    pair: Vec<Range<usize>>,
    /// The breakpoints and the paused line of the file.
    gutter: crate::debug::Gutter,
}

/// What the view of the excerpts needs to draw the lines of the files, with what it worked out for the lines already drawn.
pub(crate) struct Drawing<'a> {
    /// The state the lines are drawn from.
    state: &'a WorkspaceState,
    /// How the editor draws.
    settings: pm_editor::EditorSettings,
    /// The matcher that marks the hits within their lines, for the excerpts of a search.
    matcher: Option<pm_editor::Query>,
    /// What the lines of each open file are drawn with, by the file's index.
    contexts: RefCell<HashMap<usize, Rc<Context>>>,
    /// The rows of each block, by the place of its file and its own place in it.
    rows: RefCell<BlockRows<'a>>,
}

/// The rows of every block built so far, by the place of its file and its own place in it.
type BlockRows<'a> = HashMap<(usize, usize), Rc<Vec<Row<'a>>>>;

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
        Modifier::new().width(theme::sp(13))
    } else {
        Modifier::new().width(theme::sp(2))
    };
    let kind = theme::code_style(settings.font_size, 1.4).color(theme::color(Role::Muted));
    let shown = crate::diff_syntax::shown(content);
    kit::row(
        Modifier::new()
            .fill_max_width()
            .height(height)
            .background(theme::color(Role::Danger.alpha(0.12)))
            .clip(),
        0,
        move || {
            rustle::spacer(Modifier::new().width(BREAKPOINT_CELL + 3.0));
            rustle::spacer(numbers.padding_each(0.0, 0.0, theme::sp(3), 0.0));
            kit::text(
                Modifier::new()
                    .weight(1.0)
                    .clip()
                    .padding_xy(theme::sp(2), 0.0),
                shown,
                kind,
            );
        },
    );
}

/// The rows of `block` of the file `viewer` holds, in order: each line of the file it shows, under the lines a change took out before it, then those taken out after the last.
fn rows_of<'a>(viewer: Option<&Viewer>, block: &'a Block) -> Vec<Row<'a>> {
    let run = shown_lines(viewer, block);
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
    rows
}

impl<'a> Drawing<'a> {
    /// What draws the lines of the files of `state` as the editor does, the hits of a search marked by `matcher`.
    pub(crate) fn new(state: &'a WorkspaceState, matcher: Option<pm_editor::Query>) -> Self {
        Self {
            state,
            settings: pm_editor::EditorSettings {
                git_gutter: state.shows_gutter(),
                ..state.editor.clone()
            },
            matcher,
            contexts: RefCell::default(),
            rows: RefCell::default(),
        }
    }

    /// What the lines of the file open at `index` are drawn with.
    fn context(&self, index: usize) -> Rc<Context> {
        if let Some(held) = self.contexts.borrow().get(&index) {
            return held.clone();
        }
        let state = self.state;
        let live = state.git.shown == Some(index);
        let pair = match state.git.viewers.get(index).map(|viewer| &viewer.contents) {
            Some(Contents::Text(document)) if live => document
                .syntax
                .as_ref()
                .and_then(|syntax| {
                    syntax.bracket_pair(document.field.text(), document.field.caret())
                })
                .map(|(open, close)| vec![open, close])
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let made = Rc::new(Context {
            owner: if live {
                Target::Editor(EditorEvent::EditorField)
            } else {
                Target::Search(SearchEvent::ExcerptOwner(index))
            },
            found: if live {
                state.find_marks()
            } else {
                (Vec::new(), None)
            },
            pair,
            gutter: state.gutter_of(index),
        });
        self.contexts.borrow_mut().insert(index, made.clone());
        made
    }

    /// The rows of `block` of the file open at `index`, which `place` names among the blocks drawn.
    fn rows(&self, index: usize, place: (usize, usize), block: &'a Block) -> Rc<Vec<Row<'a>>> {
        if let Some(held) = self.rows.borrow().get(&place) {
            return held.clone();
        }
        let made = Rc::new(rows_of(self.state.git.viewers.get(index), block));
        self.rows.borrow_mut().insert(place, made.clone());
        made
    }

    /// How many rows `block` of the file open at `index` has.
    pub(crate) fn row_count(&self, index: usize, block: &Block) -> usize {
        block_rows(self.state.git.viewers.get(index), block)
    }

    /// Emits the row at `at` of `block` of the file open at `index`, which `place` names among the blocks drawn, in a box that a press makes the file edited; nothing when the file is not read.
    #[track_caller]
    pub(crate) fn row(&self, index: usize, place: (usize, usize), block: &'a Block, at: usize) {
        let state = self.state;
        let Some(Contents::Text(document)) = state.git.viewers.get(index).map(|v| &v.contents)
        else {
            return;
        };
        let rows = self.rows(index, place, block);
        let Some(row) = rows.get(at) else {
            return;
        };
        let context = self.context(index);
        let owner = context.owner;
        kit::column(
            Modifier::new()
                .fill_max_width()
                .cursor(CursorIcon::Text)
                .control(owner, send(Event::pressing(owner))),
            0,
            || match row {
                Row::Removed(text) => removed_row(text, &self.settings),
                Row::Line(line) => self.line(document, index, block, *line, &context),
            },
        );
    }

    /// Emits the line at `line` of `document`, the file open at `index`, which `block` shows, drawn with `context`.
    #[track_caller]
    fn line(
        &self,
        document: &pm_editor::Document,
        index: usize,
        block: &Block,
        line: usize,
        context: &Context,
    ) {
        let spans = document
            .highlights(line..line + 1)
            .pop()
            .unwrap_or_default();
        let mut marks = line_marks(
            document,
            line,
            spans,
            &context.pair,
            &context.found,
            &self.settings,
        );
        let range = document.line(line);
        let hit = block.hits.binary_search(&line).is_ok();
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
        let height = document_row(&self.settings, &segments);
        let gutter = &context.gutter;
        let beside = Beside {
            blamed: None,
            breakpoint: || breakpoint_cell(index, line, gutter, height),
            bulb: false,
            paused: gutter.paused == Some(line),
        };
        let shown = || {
            file_line(
                document,
                line,
                marks,
                segments,
                beside,
                &self.settings,
                (context.owner, &self.state.code_lines),
            );
        };
        if hit && self.matcher.is_none() {
            kit::column(
                Modifier::new()
                    .fill_max_width()
                    .background(theme::color(Role::Success.alpha(0.12))),
                0,
                shown,
            );
        } else {
            shown();
        }
    }
}

/// Emits the space under a block.
#[track_caller]
pub(crate) fn block_gap() {
    rustle::spacer(Modifier::new().height(GAP));
}

/// One item of the list of the excerpts.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Entry {
    /// The header of the file at this position in the excerpts.
    Header(usize),
    /// The rows of the diff of the file at this position, with the space under them.
    Diff(usize),
    /// The row at the third position of the block at the second of the file at the first.
    Row(usize, usize, usize),
    /// The space under the block at the second position of the file at the first.
    Gap(usize, usize),
    /// The space the block at the second position of the file at the first takes while its file is not read.
    Waiting(usize, usize),
    /// The lines a change took out of the block at the second position of the file at the first, which is gone.
    Gone(usize, usize),
}

/// Emits the lines of a file that is shown from its diff alone, as the rows of a diff, under its header.
fn diff_rows(file: &ExcerptFile) {
    let colours = crate::diff_syntax::DiffColours::of(
        &file.diff,
        crate::file_icon::file_language(&file.path).as_deref(),
    );
    for (at, diff) in file.diff.iter().enumerate() {
        crate::diff::diff_line(diff.kind, &diff.text, colours.line(at));
    }
}

/// The items of the list of `excerpts`: a header over each file, and under it, unless it is folded, the rows of its diff or of each of its blocks.
fn entries(state: &WorkspaceState, excerpts: &Excerpts, drawing: &Drawing) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (place, file) in excerpts.files.iter().enumerate() {
        entries.push(Entry::Header(place));
        if file.closed {
            continue;
        }
        if !file.diff.is_empty() {
            entries.push(Entry::Diff(place));
            continue;
        }
        let open = state.viewer_at(&file.path).filter(|_| file.editable);
        for (step, block) in file.blocks.iter().enumerate() {
            match open {
                _ if !file.editable => entries.push(Entry::Gone(place, step)),
                Some(index)
                    if matches!(
                        state.git.viewers.get(index).map(|viewer| &viewer.contents),
                        Some(Contents::Text(_))
                    ) =>
                {
                    let count = drawing.row_count(index, block);
                    entries.extend((0..count).map(|row| Entry::Row(place, step, row)));
                    entries.push(Entry::Gap(place, step));
                }
                _ => entries.push(Entry::Waiting(place, step)),
            }
        }
    }
    entries
}

/// Emits the excerpts as a lazy list: a header over each file, and under it the runs of lines, headers `size` big.
#[track_caller]
pub(crate) fn file_list(state: &WorkspaceState, size: Size) {
    let Some(excerpts) = state.excerpts.as_ref() else {
        return;
    };
    let row = state.editor.row_height();
    let drawing = Drawing::new(
        state,
        match &excerpts.source {
            Source::Search { text, case, regex } => Some(pm_editor::Query {
                text: text.clone(),
                case: *case,
                word: false,
                regex: *regex,
            }),
            _ => None,
        },
    );
    let entries = entries(state, excerpts, &drawing);
    let draw = |entry: Entry| match entry {
        Entry::Header(place) => {
            let file = &excerpts.files[place];
            let viewer = state
                .viewer_at(&file.path)
                .and_then(|i| state.git.viewers.get(i));
            header(place, file, viewer, excerpts, size);
        }
        Entry::Diff(place) => {
            let file = &excerpts.files[place];
            kit::column(Modifier::new().fill_max_width(), 0, || {
                diff_rows(file);
                block_gap();
            });
        }
        Entry::Row(place, step, at) => {
            let file = &excerpts.files[place];
            if let Some(index) = state.viewer_at(&file.path) {
                drawing.row(index, (place, step), &file.blocks[step], at);
            }
        }
        Entry::Gap(..) => block_gap(),
        Entry::Waiting(place, step) => {
            let file = &excerpts.files[place];
            let index = state.viewer_at(&file.path);
            let rows = match index {
                Some(_) => 1,
                None => block_rows(None, &file.blocks[step]).max(1),
            };
            rustle::spacer(Modifier::new().height(rows as f32 * row + GAP));
        }
        Entry::Gone(place, step) => {
            gone_block(&excerpts.files[place].blocks[step], &drawing.settings);
        }
    };
    kit::column(
        Modifier::new()
            .weight(1.0)
            .clip()
            .background(theme::color(Role::Background)),
        0,
        || {
            rustle::lazy_list_keyed(
                Modifier::new().fill_max_width().weight(1.0),
                &excerpts.scroll,
                entries.len(),
                |item| entries[item],
                |item| draw(entries[item]),
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

/// Emits the lines a change took out of a file that is gone, drawn as taken out, with the space under them.
fn gone_block(block: &Block, settings: &pm_editor::EditorSettings) {
    kit::column(Modifier::new().fill_max_width(), 0, || {
        for text in block.removed.iter().flat_map(|(_, lines)| lines.iter()) {
            removed_row(text, settings);
        }
        block_gap();
    });
}

/// Emits the excerpts of a search.
pub fn excerpts_view(state: &WorkspaceState) {
    file_list(state, Size::default());
}
