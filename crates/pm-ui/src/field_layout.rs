//! Wrapped field navigation without shaping or painting offscreen lines.

use std::sync::Arc;

use pm_input::FocusId;
use pm_text::Shaper;
use pm_typography::TypographyStyle;

use crate::Line;

/// Byte ranges for every wrapped line, shared by painting and lazy keyboard navigation.
#[derive(Clone, Debug)]
pub struct FieldLayout {
    /// The input these lines belong to.
    pub id: FocusId,
    /// The displayed text, including any composition or masking.
    pub content: Arc<str>,
    /// The wrapped lines in display order.
    pub lines: Arc<[Line]>,
    /// The typography used to shape a requested line.
    pub typography: TypographyStyle,
}

impl FieldLayout {
    /// Finds the wrapped line containing a displayed byte offset.
    fn line_at(&self, caret: usize) -> usize {
        self.lines
            .partition_point(|line| line.start <= caret)
            .saturating_sub(1)
    }

    /// Moves by wrapped lines while shaping only the current and destination lines.
    pub(crate) fn vertical(&self, caret: usize, by: isize, shaper: &mut Shaper) -> Option<usize> {
        let at = self.line_at(caret);
        let here = self.lines.get(at)?;
        let there = self.lines.get(at.checked_add_signed(by)?)?;
        let run = shaper.shape(&self.content[here.start..here.end], &self.typography);
        let x = run.caret_x(caret - here.start);
        let run = shaper.shape(&self.content[there.start..there.end], &self.typography);
        Some(there.start + run.byte_at(x, there.end - there.start))
    }

    /// Returns the beginning or end of the wrapped line without shaping text.
    pub(crate) fn edge(&self, caret: usize, end: bool) -> Option<usize> {
        let line = self.lines.get(self.line_at(caret))?;
        Some(if end { line.end } else { line.start })
    }
}
