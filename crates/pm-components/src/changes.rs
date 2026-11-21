//! The changes of a commit or of a branch, and a file set against one of the states it was saved in, as one scrollable view of every file: each folded away from its header, and, in the folder, edited in place.

use crate::areas::SearchEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use pm_editor::Edit;
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, Arrangement};

use crate::Tally;
use crate::button::Variant;
use crate::excerpts::{Block, ExcerptFile, Source, file_list, restore_button};
use crate::open_files::Contents;
use crate::parts::{glyph, line, spacer};
use crate::sample::{Change, DiffLine, Kind};
use crate::size::Size;
use crate::state::{Tab, WorkspaceState};
use crate::theme::Role;

/// How tall the strip above the files is, in logical pixels.
const STRIP: f32 = 40.0;

/// How many lines of the file are kept around a change when a file is set against an older state.
const CONTEXT: usize = 3;

/// The view of the changes being shown, being described: the state it is drawn from.
///
/// Build it with [`Changes::new`] and emit it with [`Changes::build`].
#[derive(Clone)]
pub struct Changes<'a> {
    /// The state the files are drawn from.
    state: &'a WorkspaceState,
}

impl<'a> Changes<'a> {
    /// The view of the excerpts `state` holds, of what they are of.
    pub fn new(state: &'a WorkspaceState) -> Self {
        Self { state }
    }

    /// Emits the strip above the files: how many there are and how far the changes went, then the button that restores the state a timeline sets the file against, or those that fold the files and take back the turn that made them.
    #[track_caller]
    fn strip(&self) {
        let Some(excerpts) = self.state.excerpts.as_ref() else {
            return;
        };
        let count = excerpts.files.len();
        let (added, removed) = excerpts
            .files
            .iter()
            .fold((0, 0), |(added, removed), file| {
                (added + file.tally.0, removed + file.tally.1)
            });
        let when = match &excerpts.source {
            Source::Timeline { when, .. } => Some(when.as_str()),
            _ => None,
        };
        let what = match &excerpts.source {
            Source::Revision { label, .. } => label.clone(),
            _ => format!("{count} file{} changed", if count == 1 { "" } else { "s" }),
        };
        let undoes =
            matches!(&excerpts.source, Source::Revision { turn: Some(_), .. }) && count > 0;
        rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(STRIP)
                .border_bottom(1.0, theme::color(Role::Border))
                .padding_each(0.0, 0.0, 0.0, 1.0)
                .padding_xy(theme::sp(4), 0.0),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Center,
            move || {
                if let Some(when) = when {
                    glyph(phosphor::CLOCK_COUNTER_CLOCKWISE, 3.5, Role::Muted);
                    line(
                        format!("Saved {when}, against the file as it is now"),
                        12,
                        1.4,
                        Role::Muted,
                    );
                } else {
                    line(what, 12, 1.4, Role::Muted);
                }
                Tally::new(added, removed).size(12).build();
                spacer();
                if when.is_some() {
                    restore_button();
                    return;
                }
                if undoes {
                    button::small_button(
                        "Undo turn",
                        Target::Search(SearchEvent::ExcerptsUndoAll),
                        Variant::Ghost,
                    )
                    .with_icon(phosphor::ARROW_COUNTER_CLOCKWISE)
                    .tooltip("Take everything this turn changed back out of the files")
                    .build();
                }
                button::action(
                    "Fold all",
                    Target::Search(SearchEvent::ExcerptsFoldAll),
                    Variant::Ghost,
                );
            },
        );
    }

    /// Emits the view: the strip, then every file with its lines.
    #[track_caller]
    pub fn build(self) {
        rustle::column_arranged(
            Modifier::new().weight(1.0),
            Arrangement::Start,
            Alignment::Start,
            || {
                self.strip();
                file_list(self.state, Size::default());
            },
        );
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
            Kind::Context | Kind::Add => {
                flush(&mut current, &mut taken, next);
                if let Some(block) = current.as_mut() {
                    if entry.kind == Kind::Add {
                        block.hits.push(next);
                    }
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

/// The `@@` line that heads the hunk `group`, counting its lines from one in
/// the old text and the new; none for an empty group.
pub(crate) fn hunk_header(group: &[similar::DiffOp]) -> Option<DiffLine> {
    let (first, last) = (group.first()?, group.last()?);
    let old = first.old_range().start..last.old_range().end;
    let new = first.new_range().start..last.new_range().end;
    let start = |range: &std::ops::Range<usize>| {
        if range.is_empty() {
            range.start
        } else {
            range.start + 1
        }
    };
    Some(DiffLine {
        kind: Kind::Hunk,
        text: format!(
            "@@ -{},{} +{},{} @@",
            start(&old),
            old.len(),
            start(&new),
            new.len()
        ),
    })
}

/// The lines of the diff that turns `old` into `new`, with `CONTEXT` lines kept around each change, each hunk under its header.
pub(crate) fn lines_between(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    for group in diff.grouped_ops(CONTEXT) {
        let Some(header) = hunk_header(&group) else {
            continue;
        };
        lines.push(header);
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
    /// Opens the files `changes`, which a commit or a branch changed, or the agent during the reader's turn `turn`, as one view named `label`, each shown from its diff and none of them edited.
    pub fn revision_opened(
        mut self,
        session: usize,
        label: String,
        turn: Option<u64>,
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
        self.excerpts_shown(session, Source::Revision { label, turn }, files);
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
