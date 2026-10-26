//! Working on whole lines: commenting them out and back in, indenting and outdenting them, moving them up and down, duplicating and deleting them, and going to one by its number. Each is one change that undoes at once.

use std::ops::Range;

use pm_text::Edit;

use crate::document::Document;

/// One stretch replaced in a rewrite of some lines: where it starts, how many bytes it takes away and what it puts there.
type Splice = (usize, usize, String);

/// Where the line holding the byte `offset` of `text` starts.
fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

/// Where the line holding the byte `offset` of `text` ends, past its line break when it has one.
fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |at| offset + at + 1)
}

/// Where `offset` ends up after `splices`, sorted by where they start and not overlapping, are made: moved along by what was put in and taken out before it, and to the end of what replaced a stretch it was inside.
fn mapped(offset: usize, splices: &[Splice]) -> usize {
    let mut shift: isize = 0;
    for (at, removed, inserted) in splices {
        if offset >= at + removed {
            shift += inserted.len() as isize - *removed as isize;
        } else if offset > *at {
            return (*at as isize + shift) as usize + inserted.len();
        }
    }
    (offset as isize + shift).max(0) as usize
}

impl Document {
    /// The lines the selection or the caret is on, as a byte range from the start of the first to past the line break of the last; a selection that ends at the start of a line leaves that line out.
    fn picked_lines(&self) -> Range<usize> {
        let text = self.field.text();
        let caret = self.field.caret();
        let range = self.field.selection().unwrap_or(caret..caret);
        let start = line_start(text, range.start);
        let end = if range.end > range.start && range.end == line_start(text, range.end) {
            range.end
        } else {
            line_end(text, range.end)
        };
        start..end
    }

    /// The lines of `block`, each as the byte range of its text without its line break.
    fn lines_in(&self, block: Range<usize>) -> Vec<Range<usize>> {
        let text = self.field.text();
        let mut lines = Vec::new();
        let mut at = block.start;
        while at < block.end || (at == block.end && lines.is_empty()) {
            let end = text[at..].find('\n').map_or(text.len(), |found| at + found);
            lines.push(at..end);
            if end >= block.end || end >= text.len() {
                break;
            }
            at = end + 1;
        }
        lines
    }

    /// The edit that makes `splices`, sorted by where they start and not overlapping, as one change, the selection carried along with the text.
    pub(crate) fn spliced(&self, splices: Vec<Splice>) -> Option<Edit> {
        let first = splices.first()?.0;
        let last = splices.iter().map(|(at, removed, _)| at + removed).max()?;
        let text = self.field.text();
        let mut replacement = String::new();
        let mut at = first;
        for (start, removed, inserted) in &splices {
            replacement.push_str(&text[at..*start]);
            replacement.push_str(inserted);
            at = start + removed;
        }
        replacement.push_str(&text[at..last]);
        let caret = self.field.caret();
        let anchor = self.field.selection().map_or(caret, |range| {
            if range.start == caret {
                range.end
            } else {
                range.start
            }
        });
        Some(Edit::Replace(
            first..last,
            replacement,
            (mapped(anchor, &splices), mapped(caret, &splices)),
        ))
    }

    /// The edit that comments out the lines the selection is on with the line comment of the scope the first of them is in, or of the language, or takes the comment away when every line that holds anything has one; where there are no line comments it wraps them in the scope's block comment, and does nothing where there are neither.
    pub fn toggle_comment_edit(&self) -> Option<Edit> {
        let text = self.field.text();
        let lines: Vec<(Range<usize>, usize)> = self
            .lines_in(self.picked_lines())
            .into_iter()
            .filter(|line| !text[line.clone()].trim().is_empty())
            .map(|line| {
                let indent = text[line.clone()].len() - text[line.clone()].trim_start().len();
                (line, indent)
            })
            .collect();
        let first = lines.first().map(|(line, indent)| line.start + indent)?;
        let Some(prefix) = self.line_comment_at(first) else {
            return self.toggle_block_comment_edit(&lines, first);
        };
        let commented = lines
            .iter()
            .all(|(line, indent)| text[line.start + indent..line.end].starts_with(&prefix));
        let splices = if commented {
            lines
                .iter()
                .map(|(line, indent)| {
                    let at = line.start + indent;
                    let after = &text[at + prefix.len()..line.end];
                    let space = usize::from(after.starts_with(' '));
                    (at, prefix.len() + space, String::new())
                })
                .collect()
        } else {
            let column = lines.iter().map(|(_, indent)| *indent).min().unwrap_or(0);
            lines
                .iter()
                .map(|(line, _)| (line.start + column, 0, format!("{prefix} ")))
                .collect()
        };
        self.spliced(splices)
    }

    /// The edit that indents the lines the selection is on one level, or takes one level off them when `outdent`.
    pub fn indent_lines_edit(&self, outdent: bool) -> Option<Edit> {
        let text = self.field.text();
        let unit = self.editing.unit(1);
        let splices: Vec<Splice> = self
            .lines_in(self.picked_lines())
            .into_iter()
            .filter_map(|line| {
                let held = &text[line.clone()];
                if !outdent {
                    return (!held.trim().is_empty()).then(|| (line.start, 0, unit.clone()));
                }
                let removed = if held.starts_with('\t') {
                    1
                } else {
                    held.chars()
                        .take(self.editing.indent)
                        .take_while(|character| *character == ' ')
                        .count()
                };
                (removed > 0).then_some((line.start, removed, String::new()))
            })
            .collect();
        self.spliced(splices)
    }

    /// The edit that moves the lines the selection is on one line up, or down when `down`, past the line beside them; nothing at the top or bottom of the text.
    pub fn move_lines_edit(&self, down: bool) -> Option<Edit> {
        let text = self.field.text();
        let block = self.picked_lines();
        let (neighbour, start) = if down {
            if block.end >= text.len() {
                return None;
            }
            (block.end..line_end(text, block.end), block.start)
        } else {
            if block.start == 0 {
                return None;
            }
            let above = line_start(text, block.start - 1);
            (above..block.start, above)
        };
        let moving = &text[block.clone()];
        let passed = &text[neighbour.clone()];
        let (first, second) = if down {
            (passed, moving)
        } else {
            (moving, passed)
        };
        let joined = if first.ends_with('\n') {
            format!("{first}{second}")
        } else {
            format!("{first}\n{}", second.strip_suffix('\n').unwrap_or(second))
        };
        let moved_to = if down {
            start + passed.len() + usize::from(!passed.ends_with('\n'))
        } else {
            start
        };
        let shift = moved_to as isize - block.start as isize;
        let caret = self.field.caret();
        let anchor = self.field.selection().map_or(caret, |range| {
            if range.start == caret {
                range.end
            } else {
                range.start
            }
        });
        let place = |offset: usize| (offset as isize + shift).max(0) as usize;
        Some(Edit::Replace(
            start..block.end.max(neighbour.end),
            joined,
            (place(anchor), place(caret)),
        ))
    }

    /// The edit that writes the lines the selection is on again right after them, the selection going with the copy below when `down` and staying on the lines above otherwise.
    pub fn duplicate_lines_edit(&self, down: bool) -> Option<Edit> {
        let text = self.field.text();
        let block = self.picked_lines();
        let held = &text[block.clone()];
        let first = if held.ends_with('\n') {
            held.to_owned()
        } else {
            format!("{held}\n")
        };
        let copy = held.strip_suffix('\n').unwrap_or(held);
        let joined = if held.ends_with('\n') {
            format!("{first}{held}")
        } else {
            format!("{first}{copy}")
        };
        let shift = if down { first.len() } else { 0 };
        let caret = self.field.caret();
        let anchor = self.field.selection().map_or(caret, |range| {
            if range.start == caret {
                range.end
            } else {
                range.start
            }
        });
        Some(Edit::Replace(
            block,
            joined,
            (anchor + shift, caret + shift),
        ))
    }

    /// The edit that deletes the lines the selection is on, the caret left at the start of the line after them.
    pub fn delete_lines_edit(&self) -> Option<Edit> {
        let text = self.field.text();
        let block = self.picked_lines();
        let range = if !text[block.clone()].ends_with('\n') && block.start > 0 {
            block.start - 1..block.end
        } else {
            block
        };
        let caret = line_start(text, range.start);
        Some(Edit::Replace(range, String::new(), (caret, caret)))
    }

    /// The edit that puts the caret at the start of line `number`, counted from 1, kept inside the text.
    pub fn go_to_line_edit(&self, number: usize) -> Edit {
        let line = number.saturating_sub(1).min(self.lines().saturating_sub(1));
        let start = self.line(line).start;
        let text = self.field.text();
        let indent = text[start..]
            .chars()
            .take_while(|character| *character == ' ' || *character == '\t')
            .count();
        Edit::Place(start + indent, false)
    }

    /// The edit that takes the caret to the start of the next stretch the language server finds wrong after it, or of the one before it when `back`, going round the end of the file.
    pub fn next_diagnostic_edit(&self, back: bool) -> Option<Edit> {
        let caret = self.field.caret();
        let mut starts: Vec<usize> = self
            .diagnostics
            .iter()
            .map(|diagnostic| self.range_of(diagnostic.span).start)
            .collect();
        starts.sort_unstable();
        starts.dedup();
        let found = if back {
            starts
                .iter()
                .rev()
                .find(|start| **start < caret)
                .or(starts.last())
        } else {
            starts
                .iter()
                .find(|start| **start > caret)
                .or(starts.first())
        };
        found.map(|start| Edit::Place(*start, false))
    }

    /// The edit that selects the line of the caret, past its line break, or adds the line after a selection of whole lines to it.
    pub fn select_line_edit(&self) -> Edit {
        let text = self.field.text();
        let caret = self.field.caret();
        let range = self.field.selection().unwrap_or(caret..caret);
        Edit::Select(line_start(text, range.start), line_end(text, range.end))
    }

    /// The edit that joins the line of the caret with the line after, or the lines of the selection with one another: each line break and the indentation after it become one space.
    pub fn join_lines_edit(&self) -> Option<Edit> {
        let text = self.field.text();
        let caret = self.field.caret();
        let range = self.field.selection().unwrap_or(caret..caret);
        let first = line_start(text, range.start);
        let reach = if range.end > range.start && range.end == line_start(text, range.end) {
            range.end - 1
        } else {
            range.end
        };
        let mut last = line_start(text, reach);
        if last == first {
            last = first + text[first..].find('\n')? + 1;
        }
        let end = text[last..].find('\n').map_or(text.len(), |at| last + at);
        let mut lines = text[first..end].split('\n');
        let head = lines.next().unwrap_or_default().trim_end();
        let mut joined = head.to_owned();
        for part in lines.map(str::trim).filter(|part| !part.is_empty()) {
            if !joined.trim().is_empty()
                && !joined.ends_with(['(', '['])
                && !part.starts_with([')', ']', '}', ',', ';', '.'])
            {
                joined.push(' ');
            }
            joined.push_str(part);
        }
        let at = first + head.len();
        Some(Edit::Replace(first..end, joined, (at, at)))
    }

    /// The edit that takes the caret to the bracket that pairs with the one it touches.
    pub fn matching_bracket_edit(&self) -> Option<Edit> {
        let caret = self.field.caret();
        let (open, close) = self
            .syntax
            .as_ref()?
            .bracket_pair(self.field.text(), caret)?;
        let target = if open.start == caret || open.end == caret {
            close.start
        } else {
            open.start
        };
        Some(Edit::Place(target, false))
    }

    /// The edit that starts a new line below the line of the caret, or above it when `above`, indented like it, and puts the caret on it.
    pub fn open_line_edit(&self, above: bool) -> Edit {
        let text = self.field.text();
        let caret = self.field.caret();
        let start = line_start(text, caret);
        let end = text[caret..].find('\n').map_or(text.len(), |at| caret + at);
        let indent: String = text[start..end]
            .chars()
            .take_while(|character| *character == ' ' || *character == '\t')
            .collect();
        let opens = text[start..end].trim_end().ends_with(['{', '(', '[', ':']);
        if above {
            let at = start + indent.len();
            Edit::Replace(start..start, format!("{indent}\n"), (at, at))
        } else {
            let indent = if opens {
                format!("{indent}{}", self.editing.unit(1))
            } else {
                indent
            };
            let at = end + 1 + indent.len();
            Edit::Replace(end..end, format!("\n{indent}"), (at, at))
        }
    }

    /// The edit that takes the caret to where the text of its line starts, or to the very start of the line when it is there already; the selection grows when `extend`.
    pub fn home_edit(&self, extend: bool) -> Edit {
        let text = self.field.text();
        let caret = self.field.caret();
        let start = line_start(text, caret);
        let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
        let first =
            start + text[start..end].len() - text[start..end].trim_start_matches([' ', '\t']).len();
        let target = if caret == first { start } else { first };
        Edit::Place(target, extend)
    }

    /// The edit that tidies the text before it is saved: spaces and tabs at the ends of lines taken off when `trim`, and a line break put at the end when `final_newline` and there is none; nothing when there is nothing to tidy.
    pub fn tidy_edit(&self, trim: bool, final_newline: bool) -> Option<Edit> {
        let text = self.field.text();
        let mut tidied: String = if trim {
            text.split('\n')
                .map(|line| line.trim_end_matches([' ', '\t']))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            text.to_owned()
        };
        if final_newline && !tidied.is_empty() && !tidied.ends_with('\n') {
            tidied.push('\n');
        }
        if tidied == text {
            return None;
        }
        let caret = self.field.caret();
        let line = self.line_of(caret);
        let column = caret - self.line(line).start;
        let start: usize = tidied
            .split('\n')
            .take(line)
            .map(|held| held.len() + 1)
            .sum();
        let length = tidied[start.min(tidied.len())..]
            .split('\n')
            .next()
            .map_or(0, str::len);
        let at = (start + column.min(length)).min(tidied.len());
        Some(Edit::Replace(0..text.len(), tidied, (at, at)))
    }
}
