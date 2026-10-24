//! The changes of a commit or of a branch, and a file set against one of the states it was saved in, as one scrollable view of every file: each folded away from its header, and, in the folder, edited in place.

use pm_icon::IconName;
use pm_style::{
    Role, border_b, border_color, flex, flex_1, flex_col, flex_none, gap, h_px, items_center,
    min_h, min_w_px, px,
};
use pm_text::Edit;
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::excerpts::{Block, EXCERPTS_FOLD_ALL, ExcerptFile, Source, file_list, restore_button};
use crate::open_files::Contents;
use crate::parts::{glyph, line, spacer};
use crate::sample::{Change, DiffLine, Kind};
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::{Button, Tally};

/// How tall the strip above the files is, in logical pixels.
const STRIP: f32 = 40.0;

/// How many lines of the file are kept around a change when a file is set against an older state.
const CONTEXT: usize = 3;

/// What the view is of, which says what the strip above the files offers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    /// One file against a state it was saved in, with the button that restores that state.
    Timeline,
    /// The files a commit or a branch changed, as their diffs say.
    #[default]
    Revision,
}

/// The view of the changes being shown, being described: what it is of, how big it is and how tall the room for it is.
///
/// Build it with [`Changes::new`], change it with the methods that follow, and
/// turn it into a node with [`Changes::build`].
#[derive(Clone, Copy)]
pub struct Changes<'a> {
    /// The state the files are drawn from.
    state: &'a WorkspaceState,
    /// How tall the room for the view is, in logical pixels.
    height: f32,
    /// What it is of.
    mode: Mode,
    /// How big it is.
    size: Size,
}

impl<'a> Changes<'a> {
    /// The view of the excerpts `state` holds, in `height` logical pixels of room, of what they are of.
    pub fn new(state: &'a WorkspaceState, height: f32) -> Self {
        let mode = match state.excerpts.as_ref().map(|excerpts| &excerpts.source) {
            Some(Source::Timeline { .. }) => Mode::Timeline,
            _ => Mode::Revision,
        };
        Self {
            state,
            height,
            mode,
            size: Size::default(),
        }
    }

    /// The view as `mode` says.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// The view `size` big.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The strip above the files: how many there are and how far the changes went, then what the mode offers.
    fn strip(&self) -> Node {
        let Some(excerpts) = self.state.excerpts.as_ref() else {
            return node();
        };
        let count = excerpts.files.len();
        let (added, removed) = excerpts
            .files
            .iter()
            .fold((0, 0), |(added, removed), file| {
                (added + file.tally.0, removed + file.tally.1)
            });
        let mut strip = node().styles([
            flex(),
            items_center(),
            gap(3),
            h_px(STRIP),
            px(4),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ]);
        let when = match &excerpts.source {
            Source::Timeline { when, .. } => when.as_str(),
            _ => "",
        };
        let what = match &excerpts.source {
            Source::Revision { label } => label.clone(),
            _ => format!("{count} file{} changed", if count == 1 { "" } else { "s" }),
        };
        strip = match self.mode {
            Mode::Timeline => strip
                .child(glyph(IconName::History, 3.5, Role::Muted))
                .child(line(
                    format!("Saved {when}, against the file as it is now"),
                    12,
                    1.4,
                    Role::Muted,
                ))
                .child(Tally::new(added, removed).size(12).build())
                .child(spacer())
                .child(restore_button()),
            Mode::Revision => strip
                .child(line(what, 12, 1.4, Role::Muted))
                .child(Tally::new(added, removed).size(12).build())
                .child(spacer())
                .child(
                    Button::new("Fold all", EXCERPTS_FOLD_ALL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build(),
                ),
        };
        strip
    }

    /// The view: the strip, then every file with its lines.
    pub fn build(self) -> Node {
        node()
            .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
            .child(self.strip())
            .child(file_list(
                self.state,
                (self.height - STRIP).max(0.0),
                self.size,
            ))
    }
}

/// The first line the change `header` puts, counted from 0, and how many lines of the new file it covers; `header` is a hunk header such as `@@ -3,4 +5,6 @@`.
fn new_side(header: &str) -> (usize, usize) {
    let part = header
        .split_whitespace()
        .find(|part| part.starts_with('+'))
        .unwrap_or("+0,0");
    let mut numbers = part[1..].split(',');
    let start: usize = numbers.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    let count: usize = numbers.next().and_then(|n| n.parse().ok()).unwrap_or(1);
    if count == 0 {
        (start, 0)
    } else {
        (start.saturating_sub(1), count)
    }
}

/// The runs of lines of the new file that the hunks of `diff` touch: each hunk with the lines it adds marked and the lines it takes out kept to show above the line that follows them.
pub(crate) fn blocks_of_diff(diff: &[DiffLine]) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut current: Option<Block> = None;
    let mut taken: Vec<String> = Vec::new();
    let mut next = 0;
    let flush = |current: &mut Option<Block>, taken: &mut Vec<String>, at: usize| {
        if taken.is_empty() {
            return;
        }
        if let Some(block) = current.as_mut() {
            block.removed.push((at, std::mem::take(taken)));
        }
    };
    for entry in diff {
        match entry.kind {
            Kind::Hunk => {
                flush(&mut current, &mut taken, next);
                blocks.extend(current.take());
                let (first, _) = new_side(&entry.text);
                next = first;
                current = Some(Block {
                    first,
                    last: first,
                    hits: Vec::new(),
                    removed: Vec::new(),
                });
            }
            Kind::Context => {
                flush(&mut current, &mut taken, next);
                if let Some(block) = current.as_mut() {
                    block.last = next;
                }
                next += 1;
            }
            Kind::Add => {
                flush(&mut current, &mut taken, next);
                if let Some(block) = current.as_mut() {
                    block.hits.push(next);
                    block.last = next;
                }
                next += 1;
            }
            Kind::Remove => taken.push(entry.text.clone()),
        }
    }
    flush(&mut current, &mut taken, next);
    blocks.extend(current);
    blocks
}

/// The file the change `change` is of, listed among the excerpts of a commit: shown from its diff alone and not to be edited.
fn revision_file(change: &Change) -> ExcerptFile {
    ExcerptFile {
        path: change.path.clone(),
        editable: false,
        blocks: Vec::new(),
        closed: false,
        mark: Some(change.mark()),
        tally: (change.added(), change.removed()),
        diff: change.diff.clone(),
    }
}

/// The lines of the diff that turns `old` into `new`, with `CONTEXT` lines kept around each change, each hunk under its header.
pub(crate) fn lines_between(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    for group in diff.grouped_ops(CONTEXT) {
        let (Some(first), Some(last)) = (group.first(), group.last()) else {
            continue;
        };
        let (old_range, new_range) = (
            first.old_range().start..last.old_range().end,
            first.new_range().start..last.new_range().end,
        );
        let start = |range: &std::ops::Range<usize>| {
            if range.is_empty() {
                range.start
            } else {
                range.start + 1
            }
        };
        lines.push(DiffLine {
            kind: Kind::Hunk,
            text: format!(
                "@@ -{},{} +{},{} @@",
                start(&old_range),
                old_range.len(),
                start(&new_range),
                new_range.len()
            ),
        });
        for operation in &group {
            for change in diff.iter_changes(operation) {
                let kind = match change.tag() {
                    similar::ChangeTag::Equal => Kind::Context,
                    similar::ChangeTag::Delete => Kind::Remove,
                    similar::ChangeTag::Insert => Kind::Add,
                };
                lines.push(DiffLine {
                    kind,
                    text: change.value().trim_end_matches(['\n', '\r']).to_owned(),
                });
            }
        }
    }
    lines
}

impl WorkspaceState {
    /// Opens the files `changes`, which a commit or a branch changed, as one view named `label`, each shown from its diff and none of them edited.
    pub fn revision_opened(
        mut self,
        session: usize,
        label: String,
        changes: Vec<pm_git::Change>,
    ) -> Self {
        if self.review_index() != Some(session) {
            return self;
        }
        let files: Vec<ExcerptFile> = changes
            .iter()
            .map(|change| revision_file(&Change::new(&change.path, &change.diff)))
            .map(|mut file| {
                file.mark = changes
                    .iter()
                    .find(|change| change.path == file.path)
                    .and_then(|change| change.index)
                    .map(letter_mark);
                file
            })
            .collect();
        self.excerpts_shown(session, Source::Revision { label }, files);
        self
    }

    /// Opens the file at `path` of the session at `session` against `text`, what it held in a state it was saved in `when`, as one view whose lines are edited in place; nothing opens while the file is not read yet.
    pub fn timeline_opened(
        mut self,
        session: usize,
        path: &str,
        when: String,
        text: String,
    ) -> Self {
        if self.review_index() != Some(session) {
            return self;
        }
        let Some(current) = self
            .viewer_at(path)
            .and_then(|index| self.git.viewers.get(index))
            .and_then(|viewer| match &viewer.contents {
                Contents::Text(document) => Some(document.field.text().to_owned()),
                _ => None,
            })
        else {
            return self;
        };
        let diff = lines_between(&text.replace("\r\n", "\n"), &current.replace("\r\n", "\n"));
        let blocks = blocks_of_diff(&diff);
        let tally = (
            diff.iter().filter(|line| line.kind == Kind::Add).count(),
            diff.iter().filter(|line| line.kind == Kind::Remove).count(),
        );
        let file = ExcerptFile {
            path: path.to_owned(),
            blocks,
            closed: false,
            mark: None,
            tally,
            editable: true,
            diff: Vec::new(),
        };
        self.excerpts_shown(session, Source::Timeline { when, text }, vec![file]);
        self
    }

    /// Puts the file of the timeline shown back as it was in the state it is set against, saves it, and shows it in a tab of its own.
    pub(crate) fn timeline_restored(mut self) -> Self {
        let Some((path, text)) = self.excerpts.as_ref().and_then(|excerpts| {
            let Source::Timeline { text, .. } = &excerpts.source else {
                return None;
            };
            Some((excerpts.files.first()?.path.clone(), text.clone()))
        }) else {
            return self;
        };
        let Some(index) = self.viewer_at(&path) else {
            return self;
        };
        self.git.shown = Some(index);
        let length = match self.shown_document() {
            Some(document) => document.field.text().len(),
            None => return self,
        };
        self.file_edited(Edit::Replace(0..length, text, (0, 0)));
        self.save_file();
        self = self.excerpts_closed();
        if let Some(index) = self.viewer_at(&path) {
            self.git.shown = Some(index);
            self.tab = Tab::File;
        }
        self
    }
}

/// The letter a change is marked with in the list of changes, for the letter git writes it with.
fn letter_mark(letter: char) -> &'static str {
    match letter {
        'A' => "A",
        'D' => "D",
        'R' | 'C' => "R",
        _ => "M",
    }
}
