//! Moving between the problems a language server found in a document.

use pm_text::Edit;

use crate::document::Document;

impl Document {
    /// The edit that puts the caret at the start of the next problem after
    /// it, going round to the first, or of the one before it when not
    /// `forward`, going round to the last; nothing when there are none.
    pub fn next_problem_edit(&self, forward: bool) -> Option<Edit> {
        let caret = self.field.caret();
        let mut starts: Vec<usize> = self
            .diagnostics
            .iter()
            .map(|found| self.range_of(found.span).start)
            .collect();
        starts.sort_unstable();
        starts.dedup();
        let target = if forward {
            starts
                .iter()
                .find(|start| **start > caret)
                .or(starts.first())
        } else {
            starts
                .iter()
                .rev()
                .find(|start| **start < caret)
                .or(starts.last())
        };
        target.map(|start| Edit::Place(*start, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_buffer::{Position, Span};

    /// A problem on `line` from its start, as a language server reports it.
    fn problem(line: u32) -> pm_lsp::Diagnostic {
        let at = Position { line, character: 0 };
        pm_lsp::Diagnostic {
            span: Span::new(at, at),
            severity: pm_lsp::Severity::Error,
            message: "wrong".to_owned(),
            source: None,
        }
    }

    /// Stepping through the problems goes forward and back, round the ends.
    #[test]
    fn steps_through_problems() {
        let mut document = Document::new("a\nb\nc\n", false, false);
        document.diagnostics = vec![problem(2), problem(1)];
        assert_eq!(
            document.next_problem_edit(true),
            Some(Edit::Place(2, false))
        );
        document.edit(Edit::Place(4, false));
        assert_eq!(
            document.next_problem_edit(true),
            Some(Edit::Place(2, false))
        );
        assert_eq!(
            document.next_problem_edit(false),
            Some(Edit::Place(2, false))
        );
        assert!(
            Document::new("a", false, false)
                .next_problem_edit(true)
                .is_none()
        );
    }
}
