//! The results of a search as one document: the lines around every hit, from every file, one under another, each edited in place in the file it comes from.

use std::ops::Range;

use pm_input::FocusId;
use pm_protocol::Location;
use pm_style::{
    Role, bg, border_b, border_color, cursor_pointer, cursor_text, flex, flex_1, flex_col,
    flex_none, font_medium, gap, h_px, items_center, min_h, min_w_px, overflow_hidden, px,
    truncate,
};
use pm_ui::{Mark, Node, Paint, node};

use crate::file_icon::file_icon;
use crate::open_files::{Contents, EDITOR_FIELD, Viewer};
use crate::parts::{glyph, line};
use crate::state::{Tab, WorkspaceState};
use crate::viewer::{Beside, breakpoint_cell, document_row, file_line, line_marks};

/// The tab of the excerpts in a pane's strip.
pub const TAB_EXCERPTS: FocusId = FocusId(7_410_000);

/// Closes the excerpts.
pub const EXCERPTS_CLOSE: FocusId = FocusId(7_410_001);

/// The scrolling box of the excerpts.
pub const EXCERPTS_SCROLL: FocusId = FocusId(7_410_002);

/// The first of the headers that open a file, each following it by the position of the file in the excerpts.
pub const EXCERPTS_FILE_BASE: u64 = 7_411_000;

/// The first of the boxes holding the lines of a file that is not the one being edited, each following it by the position of the file among the open files; a press on one makes that file the one edited.
pub const EXCERPT_OWNER_BASE: u64 = 7_412_000;

/// One past the last box holding lines of a file that is not the one being edited.
const EXCERPT_OWNER_END: u64 = 7_413_000;

/// How many lines are shown on each side of a hit.
const CONTEXT: usize = 2;

/// How tall the header over the excerpts of a file is, in logical pixels.
const HEADER: f32 = 32.0;

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
    /// The lines that hold a hit.
    pub hits: Vec<usize>,
}

/// The excerpts of one file.
#[derive(Clone, Debug, PartialEq)]
pub struct ExcerptFile {
    /// The file, from the root of the project.
    pub path: String,
    /// The runs of lines, in the order of the file.
    pub blocks: Vec<Block>,
}

/// The excerpts being shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Excerpts {
    /// What was searched for, which names the tab and marks the hits.
    pub text: String,
    /// Whether upper and lower case had to match.
    pub case: bool,
    /// Whether the text was a regular expression.
    pub regex: bool,
    /// The files, in the order the search listed them.
    pub files: Vec<ExcerptFile>,
    /// How far down it is scrolled, in logical pixels.
    pub scroll: f32,
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
        || (EXCERPTS_FILE_BASE..EXCERPT_OWNER_END).contains(&id.0)
}

impl WorkspaceState {
    /// The position among the open files of the file `path`, read from the folder.
    fn viewer_at(&self, path: &str) -> Option<usize> {
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
            .map(|group| ExcerptFile {
                path: group.path.clone(),
                blocks: blocks_of(
                    &group
                        .hits
                        .iter()
                        .map(|hit| hit.span.start.line as usize)
                        .collect::<Vec<_>>(),
                ),
            })
            .collect();
        if files.is_empty() {
            return self;
        }
        if self.excerpts.is_some() {
            self = self.excerpts_closed();
        }
        for file in &files {
            self.load_hidden_viewer(session, &file.path);
        }
        self.git.shown = files.first().and_then(|file| self.viewer_at(&file.path));
        self.excerpts = Some(Excerpts {
            text,
            case,
            regex,
            files,
            scroll: 0.0,
        });
        self.tab = Tab::Excerpts;
        self
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

    /// Carries out what pressing `id` of the excerpts does.
    pub(crate) fn excerpts_pressed(mut self, id: FocusId) -> Self {
        if id == TAB_EXCERPTS {
            self.tab = Tab::Excerpts;
            return self;
        }
        if id == EXCERPTS_CLOSE {
            return self.excerpts_closed();
        }
        if let Some(index) = self.excerpt_owner_of(id) {
            return self.excerpt_file_shown(index);
        }
        let place = (id.0 - EXCERPTS_FILE_BASE) as usize;
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
            } else {
                block.last = shifted(block.last).max(block.first);
                for hit in block.hits.iter_mut().filter(|hit| **hit > at) {
                    *hit = shifted(*hit);
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
        _ => block.last + 1,
    };
    block.first.min(end)..end
}

/// The header over the excerpts of one file: its icon and name, where it is, whether it has changes not saved and how many hits it holds.
fn header(position: usize, file: &ExcerptFile, viewer: Option<&Viewer>) -> Node {
    let name = file.path.rsplit('/').next().unwrap_or(&file.path);
    let folder = file.path.strip_suffix(name).unwrap_or_default();
    let hits: usize = file.blocks.iter().map(|block| block.hits.len()).sum();
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h_px(HEADER),
            flex_none(),
            px(3),
            bg(Role::Surface),
            border_b(),
            border_color(Role::Border),
            cursor_pointer(),
        ])
        .child(file_icon(name, 4.0))
        .child(line(name, 13, 1.4, Role::Text).styles([font_medium(), flex_none()]))
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(folder, 12, 1.4, Role::Subtle).styles([truncate()])),
        );
    if viewer.is_some_and(Viewer::is_dirty) {
        row = row.child(glyph(pm_icon::IconName::Pencil, 3.0, Role::Muted));
    }
    row.child(line(hits.to_string(), 11, 1.4, Role::Muted).styles([flex_none()]))
        .focusable(FocusId(EXCERPTS_FILE_BASE + position as u64))
}

/// What the view of the excerpts needs to draw the lines of one file.
struct Drawing<'a> {
    /// The state the lines are drawn from.
    state: &'a WorkspaceState,
    /// How the editor draws.
    settings: &'a pm_editor::EditorSettings,
    /// The matcher that marks the hits within their lines.
    matcher: pm_editor::Query,
}

impl Drawing<'_> {
    /// The lines of `block` of the file open at `index`, in a box that a press makes the file edited, with the space under them.
    fn block(&self, index: usize, block: &Block) -> Node {
        let state = self.state;
        let Some(Contents::Text(document)) = state.git.viewers.get(index).map(|v| &v.contents)
        else {
            return node().styles([h_px(GAP + state.editor.row_height()), flex_none()]);
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
        let painted = document.highlights(run.clone());
        let lines = run.zip(painted).map(|(row, spans)| {
            let mut marks = line_marks(document, row, spans, &pair, &found, self.settings);
            let range = document.line(row);
            if block.hits.contains(&row)
                && let Some(matched) = self.matcher.first_in(&document.field.text()[range.clone()])
            {
                marks.push(Mark {
                    range: matched,
                    color: Role::SurfaceSelected.into(),
                    paint: Paint::Fill,
                });
            }
            let segments = vec![Range {
                start: 0,
                end: range.len(),
            }];
            let cell = breakpoint_cell(index, row, &gutter, document_row(self.settings, &segments));
            file_line(
                document,
                row,
                marks,
                segments,
                Beside {
                    blamed: None,
                    breakpoint: cell,
                    paused: gutter.paused == Some(row),
                },
                self.settings,
                owner,
            )
        });
        node()
            .styles([flex(), flex_col(), flex_none()])
            .child(
                node()
                    .styles([flex(), flex_col(), flex_none(), cursor_text()])
                    .focusable(owner)
                    .children(lines),
            )
            .child(node().styles([h_px(GAP), flex_none()]))
    }
}

/// One thing drawn in the excerpts, with its height.
enum Entry {
    /// The header of the file at this position in the excerpts.
    Header(usize),
    /// The block at the second position of the file at the first.
    Block(usize, usize),
}

/// The excerpts in a view `width` by `height` logical pixels: a header over each file, and under it the runs of lines around the hits, only those near the view being built.
pub fn excerpts_view(state: &WorkspaceState, height: f32) -> Node {
    let Some(excerpts) = state.excerpts.as_ref() else {
        return node();
    };
    let row = state.editor.row_height();
    let mut entries: Vec<(Entry, f32)> = Vec::new();
    for (place, file) in excerpts.files.iter().enumerate() {
        let viewer = state
            .viewer_at(&file.path)
            .and_then(|i| state.git.viewers.get(i));
        entries.push((Entry::Header(place), HEADER));
        for (step, block) in file.blocks.iter().enumerate() {
            let lines = shown_lines(viewer, block).len().max(1);
            entries.push((Entry::Block(place, step), lines as f32 * row + GAP));
        }
    }
    let settings = pm_editor::EditorSettings {
        git_gutter: state.shows_gutter(),
        ..state.editor.clone()
    };
    let drawing = Drawing {
        state,
        settings: &settings,
        matcher: pm_editor::Query {
            text: excerpts.text.clone(),
            case: excerpts.case,
            word: false,
            regex: excerpts.regex,
        },
    };
    let (low, high) = (
        excerpts.scroll - OVERSCAN,
        excerpts.scroll + height + OVERSCAN,
    );
    let (mut before, mut after, mut top) = (0.0_f32, 0.0_f32, 0.0_f32);
    let mut built = Vec::new();
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
        built.push(match entry {
            Entry::Header(place) => {
                let file = &excerpts.files[*place];
                let viewer = state
                    .viewer_at(&file.path)
                    .and_then(|i| state.git.viewers.get(i));
                header(*place, file, viewer)
            }
            Entry::Block(place, step) => {
                let file = &excerpts.files[*place];
                match state.viewer_at(&file.path) {
                    Some(index) => drawing.block(index, &file.blocks[*step]),
                    None => node().styles([h_px(*size), flex_none()]),
                }
            }
        });
    }
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_w_px(0),
            min_h(0),
            overflow_hidden(),
            bg(Role::Background),
        ])
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(EXCERPTS_SCROLL, excerpts.scroll)
                .child(node().styles([h_px(before), flex_none()]))
                .children(built)
                .child(node().styles([h_px(after), flex_none()])),
        )
}
