//! Which lines a fold would hide, read off the shape of the text.
//!
//! What can be folded is worked out from indentation rather than from the
//! syntax tree: a line that something below it is indented under holds that
//! something, in every language anybody indents. A grammar would say it more
//! precisely for the languages the editor has one for and say nothing at all
//! for the rest.

use std::ops::Range;

use crate::buffer::Buffer;

impl Buffer {
    /// The lines a fold at `line` would hide, if anything is under it.
    ///
    /// The header itself stays: folding is hiding what a line holds, not
    /// hiding the line that says what it holds.
    pub fn fold_at(&self, line: usize) -> Option<Range<usize>> {
        let outer = self.indentation(line)?;
        let mut last = line;

        for below in line + 1..self.line_count() {
            match self.indentation(below) {
                Some(inner) if inner > outer => last = below,
                Some(_) => break,
                None => {}
            }
        }

        (last > line).then_some(line + 1..last + 1)
    }

    /// Whether anything is indented under `line`.
    pub fn is_foldable(&self, line: usize) -> bool {
        self.fold_at(line).is_some()
    }

    /// Every fold the file has, outermost first.
    ///
    /// This is what folding the whole file comes to: one fold per line that
    /// holds something, which the folds already inside it then join.
    pub fn folds(&self) -> Vec<Range<usize>> {
        (0..self.line_count())
            .filter_map(|line| self.fold_at(line))
            .collect()
    }

    /// The fold holding `line`, the innermost one first.
    pub fn fold_around(&self, line: usize) -> Option<Range<usize>> {
        (0..=line)
            .rev()
            .filter_map(|above| self.fold_at(above))
            .find(|fold| fold.contains(&line))
    }

    /// The lines whose folds hold `line`, outermost first.
    ///
    /// This is what a line is inside — the function, the block, the
    /// implementation — read the same way a fold is, so a pane can keep
    /// them in sight while the body of them scrolls past.
    pub fn enclosing(&self, line: usize) -> Vec<usize> {
        let mut holders = Vec::new();
        let mut indent = self.indentation(line).unwrap_or(usize::MAX);

        for above in (0..line).rev() {
            let Some(outer) = self.indentation(above) else {
                continue;
            };
            if outer >= indent {
                continue;
            }
            if self.fold_at(above).is_some_and(|fold| fold.contains(&line)) {
                holders.push(above);
                indent = outer;
            }
            if indent == 0 {
                break;
            }
        }
        holders.reverse();
        holders
    }

    /// How far `line` is indented, or nothing when it holds no text.
    fn indentation(&self, line: usize) -> Option<usize> {
        let mut indent = 0;
        for ch in self.line_chars(line) {
            match ch {
                ' ' => indent += 1,
                '\t' => indent += self.tab_width(),
                _ => return Some(indent),
            }
        }
        None
    }
}
