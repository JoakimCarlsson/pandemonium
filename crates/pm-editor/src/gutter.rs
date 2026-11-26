//! How the text differs from the last commit, line by line, for the marks beside the line numbers: lines added, lines changed, and where lines were taken out.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::Edit;
use pm_buffer::Worker;
use similar::{Algorithm, DiffOp, capture_diff_deadline};

use crate::document::{Document, LineEdit};

/// How long working out the differences may take before it settles for a rougher answer.
const PATIENCE: Duration = Duration::from_millis(30);

/// How many bytes the text and what the last commit held may come to for the differences to be worked out where the text is edited rather than on [`DIFFING`].
const NEAR: usize = 64 * 1024;

/// The thread that works out how longer texts differ from the last commit.
static DIFFING: Worker = Worker::new("diff");

/// How a line differs from the last commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineChange {
    /// The line is new.
    Added,
    /// The line replaced one or more lines.
    Modified,
    /// Lines were taken out just before this line.
    DeletedBefore,
}

/// How each line differs from the last commit, by line.
type ByLine = Arc<Vec<Option<LineChange>>>;

/// Working out the differences on [`DIFFING`], shared by the copies of the document it is for.
#[derive(Debug, Default)]
struct Diffing {
    /// The differences, once worked out.
    done: OnceLock<ByLine>,
    /// Whether a later edit made it of no use.
    cancelled: AtomicBool,
}

/// How each line differs from the last commit: as worked out last, and while the text is compared again away from here, the comparison under way, which takes over once it is done.
#[derive(Clone, Debug, Default)]
pub(crate) struct Changes {
    /// As worked out last, moved along the edits since.
    known: ByLine,
    /// The comparison under way, when there is one.
    diffing: Option<Arc<Diffing>>,
}

impl PartialEq for Changes {
    /// Two are alike when they hold the same differences, told at once when they share them.
    fn eq(&self, other: &Self) -> bool {
        let (mine, theirs) = (self.current(), other.current());
        Arc::ptr_eq(mine, theirs) || mine == theirs
    }
}

impl Changes {
    /// The differences known best: those the comparison under way found once it is done, or else those worked out last.
    fn current(&self) -> &ByLine {
        self.diffing
            .as_ref()
            .and_then(|diffing| diffing.done.get())
            .unwrap_or(&self.known)
    }

    /// The comparison under way, while it is under way.
    pub(crate) fn pending(&self) -> Option<Settling> {
        self.diffing
            .as_ref()
            .filter(|diffing| {
                diffing.done.get().is_none() && !diffing.cancelled.load(Ordering::Relaxed)
            })
            .map(|diffing| Settling(diffing.clone()))
    }
}

/// A comparison with the last commit under way away from here, to wait on.
#[derive(Clone, Debug)]
pub struct Settling(Arc<Diffing>);

impl Settling {
    /// Whether the comparison is done or was given up.
    pub fn is_settled(&self) -> bool {
        self.0.done.get().is_some() || self.0.cancelled.load(Ordering::Relaxed)
    }

    /// A number that tells this comparison apart from others under way.
    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

/// How each line of `new` differs from `old`, giving up on finer answers after [`PATIENCE`].
fn diff(old: &str, new: &str) -> Vec<Option<LineChange>> {
    let old: Vec<&str> = old.split('\n').collect();
    let new: Vec<&str> = new.split('\n').collect();
    let mut changes = vec![None; new.len() + 1];
    let operations = capture_diff_deadline(
        Algorithm::Myers,
        &old,
        0..old.len(),
        &new,
        0..new.len(),
        Some(Instant::now() + PATIENCE),
    );
    for operation in operations {
        match operation {
            DiffOp::Equal { .. } => {}
            DiffOp::Insert {
                new_index, new_len, ..
            } => {
                changes[new_index..new_index + new_len].fill(Some(LineChange::Added));
            }
            DiffOp::Replace {
                new_index, new_len, ..
            } => {
                changes[new_index..new_index + new_len].fill(Some(LineChange::Modified));
            }
            DiffOp::Delete { new_index, .. } => {
                if changes[new_index].is_none() {
                    changes[new_index] = Some(LineChange::DeletedBefore);
                }
            }
        }
    }
    changes.truncate(new.len());
    changes
}

impl Document {
    /// Takes in what the file held in the last commit, `base`, or that it is not in one, and works out how the text differs from it.
    pub fn set_base(&mut self, base: Option<String>) {
        self.base = base.map(|text| Arc::from(text.replace("\r\n", "\n")));
        self.compare_with_base(None);
    }

    /// Works out again how each line differs from what the file held in the last commit, after `edit` when the text was edited: where it is edited when the texts are short, and otherwise away from here, the lines known before moved along the edit and those it touched shown as changed meanwhile.
    pub(crate) fn compare_with_base(&mut self, edit: Option<&LineEdit>) {
        let previous = self.line_changes.current().clone();
        if let Some(diffing) = self.line_changes.diffing.take() {
            diffing.cancelled.store(true, Ordering::Relaxed);
        }
        let Some(base) = self.base.clone() else {
            self.line_changes = Changes::default();
            return;
        };
        let text = self.field.text();
        if base.len() + text.len() <= NEAR {
            self.line_changes = Changes {
                known: Arc::new(diff(&base, text)),
                diffing: None,
            };
            return;
        }
        let mut known = (*previous).clone();
        match edit {
            Some(edit) if edit.old_rows.start <= known.len() => {
                let end = edit.old_rows.end.min(known.len());
                known.splice(
                    edit.old_rows.start..end,
                    std::iter::repeat_n(Some(LineChange::Modified), edit.new_rows.len()),
                );
            }
            _ => {}
        }
        known.resize(self.lines(), None);
        let diffing = Arc::new(Diffing::default());
        let job = diffing.clone();
        let text = text.to_owned();
        DIFFING.run(move || {
            if !job.cancelled.load(Ordering::Relaxed) {
                let _ = job.done.set(Arc::new(diff(&base, &text)));
            }
        });
        self.line_changes = Changes {
            known: Arc::new(known),
            diffing: Some(diffing),
        };
    }

    /// The edit that takes the caret to the start of the first line of the next stretch of changed lines after the caret's line, or of the one before when `back`, going round the end of the file.
    pub fn next_change_edit(&self, back: bool) -> Option<Edit> {
        let changed = |row: usize| self.line_change(row).is_some();
        let starts: Vec<usize> = (0..self.lines())
            .filter(|row| changed(*row) && (*row == 0 || !changed(*row - 1)))
            .collect();
        let row = self.line_of(self.field.caret());
        let found = if back {
            starts
                .iter()
                .rev()
                .find(|start| **start < row)
                .or(starts.last())
        } else {
            starts.iter().find(|start| **start > row).or(starts.first())
        };
        found.map(|start| Edit::Place(self.line(*start).start, false))
    }

    /// How the line at `index` differs from the last commit, when it does.
    pub fn line_change(&self, index: usize) -> Option<LineChange> {
        self.line_changes.current().get(index).copied().flatten()
    }
}
