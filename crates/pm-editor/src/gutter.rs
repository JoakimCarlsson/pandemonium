//! How the text differs from the last commit, line by line, for the marks beside the line numbers: lines added, lines changed, and where lines were taken out.

use std::time::{Duration, Instant};

use pm_text::Edit;
use similar::{Algorithm, DiffOp, capture_diff_deadline};

use crate::document::Document;

/// How long working out the differences may take before it settles for a rougher answer.
const PATIENCE: Duration = Duration::from_millis(30);

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

impl Document {
    /// Takes in what the file held in the last commit, `base`, or that it is not in one, and works out how the text differs from it.
    pub fn set_base(&mut self, base: Option<String>) {
        self.base = base.map(|text| text.replace("\r\n", "\n"));
        self.compare_with_base();
    }

    /// Works out again how each line differs from what the file held in the last commit.
    pub(crate) fn compare_with_base(&mut self) {
        let Some(base) = &self.base else {
            self.line_changes = Vec::new();
            return;
        };
        let old: Vec<&str> = base.split('\n').collect();
        let new: Vec<&str> = self.field.text().split('\n').collect();
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
        self.line_changes = changes;
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
        self.line_changes.get(index).copied().flatten()
    }
}
