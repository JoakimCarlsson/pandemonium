//! Sticky scroll: the lines that open the blocks the first line of the view is inside of, which stay at the top of the view while the block is scrolled through.

use crate::document::Document;

impl Document {
    /// The lines that open the blocks around the line drawn in row `row`, outermost first, at most `most` of them, nearest the line when there are more; each is above the line, and the rows they take at the top of the view cover rows that change which blocks those are, so the lines are worked out again for the row under them for as long as that gives more of them.
    pub fn sticky_lines(&self, row: usize, columns: Option<usize>, most: usize) -> Vec<usize> {
        let Some(syntax) = &self.syntax else {
            return Vec::new();
        };
        if most == 0 {
            return Vec::new();
        }
        let text = self.field.text();
        let mut held: Vec<usize> = Vec::new();
        for _ in 0..=most {
            let (lines, _) = self.wrapped_lines(row + held.len(), 1, columns);
            let Some(under) = lines.first().copied() else {
                return Vec::new();
            };
            let line = self.line(under);
            let leading = text[line.clone()].len() - text[line.clone()].trim_start().len();
            let mut opening: Vec<usize> = syntax
                .enclosing(text, (line.start + leading).min(text.len()))
                .iter()
                .map(|item| self.line_of(item.name_start))
                .filter(|header| *header < under)
                .collect();
            opening.dedup();
            let start = opening.len().saturating_sub(most);
            let opening = opening.split_off(start);
            if opening.len() <= held.len() {
                break;
            }
            held = opening;
        }
        held
    }
}
