//! Rewriting the text of the selections: letters written in another case, lines put in order, reversed and made unique, two characters swapped and a caret put at the end of each line a selection covers.

use std::collections::HashSet;
use std::ops::Range;

use pm_text::Edit;

use crate::document::Document;

/// How the letters of a stretch of text are written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Case {
    /// Every letter a capital.
    Upper,
    /// Every letter small.
    Lower,
    /// Each word starting with a capital and going on small.
    Title,
    /// Capitals small and small letters capitals.
    Swap,
    /// Words small and joined by underscores.
    Snake,
    /// Words small and joined by hyphens.
    Kebab,
    /// Words joined, each but the first starting with a capital.
    Camel,
    /// Words joined, each starting with a capital.
    Pascal,
    /// Words capital and joined by underscores.
    Constant,
}

/// How the selected lines are rearranged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineOrder {
    /// From the smallest to the largest.
    Ascending,
    /// From the largest to the smallest.
    Descending,
    /// From the smallest to the largest, capitals counting as small letters.
    IgnoringCase,
    /// Last first.
    Reversed,
    /// Each line that comes again left out, the first of each kept.
    Unique,
}

/// Whether `character` is part of a word.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// `word` with its first letter a capital and the rest small.
fn capitalized(word: &str) -> String {
    let mut characters = word.chars();
    characters.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(characters.flat_map(char::to_lowercase))
            .collect()
    })
}

/// The words `text` is made of: split where something that is not a letter or digit comes, before a capital that follows a small letter or a digit, and before the last capital of a run of capitals that a small letter follows.
fn words_of(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (at, &character) in characters.iter().enumerate() {
        if !character.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        let before = at
            .checked_sub(1)
            .map(|index| characters[index])
            .filter(|held| held.is_alphanumeric());
        if let Some(before) = before
            && character.is_uppercase()
        {
            let lowercase_follows = characters
                .get(at + 1)
                .is_some_and(|next| next.is_lowercase());
            if before.is_lowercase()
                || before.is_numeric()
                || (before.is_uppercase() && lowercase_follows)
            {
                words.push(std::mem::take(&mut current));
            }
        }
        current.push(character);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// `line` with the first letter of each word a capital and the rest small.
fn titled(line: &str) -> String {
    let mut titled = String::with_capacity(line.len());
    let mut starts = true;
    for character in line.chars() {
        if character.is_alphanumeric() {
            if starts {
                titled.extend(character.to_uppercase());
            } else {
                titled.extend(character.to_lowercase());
            }
            starts = false;
        } else {
            if character != '\'' && character != '’' {
                starts = true;
            }
            titled.push(character);
        }
    }
    titled
}

impl Case {
    /// `text` written in this case, line by line.
    pub fn apply(self, text: &str) -> String {
        text.split('\n')
            .map(|line| self.apply_to_line(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `line` written in this case, the spaces and tabs at its ends left as they are.
    fn apply_to_line(self, line: &str) -> String {
        match self {
            Self::Upper => return line.to_uppercase(),
            Self::Lower => return line.to_lowercase(),
            Self::Title => return titled(line),
            Self::Swap => {
                return line
                    .chars()
                    .map(|character| {
                        if character.is_uppercase() {
                            character.to_lowercase().to_string()
                        } else {
                            character.to_uppercase().to_string()
                        }
                    })
                    .collect();
            }
            _ => {}
        }
        let start = line.len() - line.trim_start().len();
        let end = line.trim_end().len();
        let words = if start < end {
            words_of(&line[start..end])
        } else {
            Vec::new()
        };
        if words.is_empty() {
            return line.to_owned();
        }
        let joined = match self {
            Self::Snake => words
                .iter()
                .map(|word| word.to_lowercase())
                .collect::<Vec<_>>()
                .join("_"),
            Self::Kebab => words
                .iter()
                .map(|word| word.to_lowercase())
                .collect::<Vec<_>>()
                .join("-"),
            Self::Constant => words
                .iter()
                .map(|word| word.to_uppercase())
                .collect::<Vec<_>>()
                .join("_"),
            Self::Pascal => words.iter().map(|word| capitalized(word)).collect(),
            _ => words
                .iter()
                .enumerate()
                .map(|(at, word)| {
                    if at == 0 {
                        word.to_lowercase()
                    } else {
                        capitalized(word)
                    }
                })
                .collect(),
        };
        format!("{}{joined}{}", &line[..start], &line[end..])
    }
}

/// Where `offset` is after `splices`, sorted and not overlapping, are made: after what replaced a stretch it was at the end of, at the start of one it was at the start of, moved along by what was put in and taken out before it, and as far into what replaced a stretch it was inside as there is room.
fn carried(offset: usize, splices: &[(Range<usize>, String)]) -> usize {
    let mut shift: isize = 0;
    for (range, text) in splices {
        if offset >= range.end {
            shift += text.len() as isize - range.len() as isize;
        } else if offset > range.start {
            let inside = (offset - range.start).min(text.len());
            return (range.start as isize + shift) as usize + inside;
        } else {
            break;
        }
    }
    (offset as isize + shift).max(0) as usize
}

impl Document {
    /// The edits that make `splices`, sorted by where they start and not overlapping, as one change, the selections carried along with the text or, when `select`, put on what the splices wrote, without the line break that ends it; nothing when they change nothing.
    fn rewritten(&self, mut splices: Vec<(Range<usize>, String)>, select: bool) -> Vec<Edit> {
        let text = self.field.text();
        splices.retain(|(range, new)| text[range.clone()] != *new);
        let (Some(first), Some(last)) = (
            splices.first().map(|(range, _)| range.start),
            splices.iter().map(|(range, _)| range.end).max(),
        ) else {
            return Vec::new();
        };
        let mut rebuilt = String::new();
        let mut at = first;
        for (range, new) in &splices {
            rebuilt.push_str(&text[at..range.start]);
            rebuilt.push_str(new);
            at = range.end;
        }
        rebuilt.push_str(&text[at..last]);
        let selections: Vec<(usize, usize)> = if select {
            let mut shift: isize = 0;
            splices
                .iter()
                .map(|(range, new)| {
                    let start = (range.start as isize + shift) as usize;
                    shift += new.len() as isize - range.len() as isize;
                    (start, start + new.strip_suffix('\n').unwrap_or(new).len())
                })
                .collect()
        } else {
            self.field
                .selections()
                .into_iter()
                .map(|(anchor, caret)| (carried(anchor, &splices), carried(caret, &splices)))
                .collect()
        };
        let mut edits = Vec::with_capacity(selections.len());
        for (at, (anchor, caret)) in selections.into_iter().enumerate() {
            if at == 0 {
                edits.push(Edit::Replace(first..last, rebuilt.clone(), (anchor, caret)));
            } else {
                edits.push(Edit::AddSelection(anchor, caret));
            }
        }
        edits
    }

    /// The word around the byte `offset`, when it touches one.
    fn word_around(&self, offset: usize) -> Option<Range<usize>> {
        let text = self.field.text();
        let start = text[..offset]
            .char_indices()
            .rev()
            .take_while(|(_, character)| is_word(*character))
            .last()
            .map_or(offset, |(at, _)| at);
        let end = text[offset..]
            .char_indices()
            .find(|(_, character)| !is_word(*character))
            .map_or(text.len(), |(at, _)| offset + at);
        (start < end).then_some(start..end)
    }

    /// The edits that write every selection in `case`, or the word at a caret that selects nothing.
    pub fn case_edits(&self, case: Case) -> Vec<Edit> {
        let text = self.field.text();
        let mut splices: Vec<(Range<usize>, String)> = Vec::new();
        for (anchor, caret) in self.field.selections() {
            let range = if anchor == caret {
                match self.word_around(caret) {
                    Some(word) => word,
                    None => continue,
                }
            } else {
                anchor.min(caret)..anchor.max(caret)
            };
            if splices
                .last()
                .is_some_and(|(held, _)| range.start < held.end)
            {
                continue;
            }
            let written = case.apply(&text[range.clone()]);
            splices.push((range, written));
        }
        self.rewritten(splices, false)
    }

    /// The edits that rearrange the lines every selection covers as `order` says, every line of the file when only a caret is in it; the lines stay selected when some were.
    pub fn order_lines_edits(&self, order: LineOrder) -> Vec<Edit> {
        let text = self.field.text();
        let selections = self.field.selections();
        let only_caret = selections.iter().all(|(anchor, caret)| anchor == caret);
        let mut blocks: Vec<Range<usize>> = Vec::new();
        if only_caret {
            blocks.push(0..text.len());
        } else {
            for (anchor, caret) in selections {
                let block = self.lines_around(anchor.min(caret)..anchor.max(caret));
                match blocks.last_mut() {
                    Some(held) if block.start <= held.end => held.end = held.end.max(block.end),
                    _ => blocks.push(block),
                }
            }
        }
        let splices = blocks
            .into_iter()
            .filter(|block| !block.is_empty())
            .map(|block| {
                let held = &text[block.clone()];
                let (body, ending) = held
                    .strip_suffix('\n')
                    .map_or((held, ""), |body| (body, "\n"));
                let mut lines: Vec<&str> = body.split('\n').collect();
                match order {
                    LineOrder::Ascending => lines.sort(),
                    LineOrder::Descending => lines.sort_by(|a, b| b.cmp(a)),
                    LineOrder::IgnoringCase => lines.sort_by_key(|line| line.to_lowercase()),
                    LineOrder::Reversed => lines.reverse(),
                    LineOrder::Unique => {
                        let mut seen = HashSet::new();
                        lines.retain(|line| seen.insert(*line));
                    }
                }
                (block, format!("{}{ending}", lines.join("\n")))
            })
            .collect();
        self.rewritten(splices, !only_caret)
    }

    /// The edit that swaps the two characters around the caret, or the last two of a line when it is at its end, the caret going after them; nothing while something is selected.
    pub fn transpose_edit(&self) -> Option<Edit> {
        if self.field.selection().is_some() {
            return None;
        }
        let text = self.field.text();
        let caret = self.field.caret();
        let line = self.line(self.line_of(caret));
        let before = text[line.start..caret].chars().next_back();
        let after = text[caret..line.end].chars().next();
        let (start, end) = match (before, after) {
            (Some(before), Some(after)) => (caret - before.len_utf8(), caret + after.len_utf8()),
            (Some(_), None) => {
                let mut earlier = text[line.start..caret].char_indices().rev();
                earlier.next()?;
                (line.start + earlier.next()?.0, caret)
            }
            _ => return None,
        };
        let mut pair = text[start..end].chars();
        let (first, second) = (pair.next()?, pair.next()?);
        Some(Edit::Replace(
            start..end,
            format!("{second}{first}"),
            (end, end),
        ))
    }

    /// The edits that put a caret at the end of each line every selection covers, at the end of the selection on its last line.
    pub fn split_into_lines_edits(&self) -> Vec<Edit> {
        let mut points: Vec<usize> = Vec::new();
        for (anchor, caret) in self.field.selections() {
            let (start, end) = (anchor.min(caret), anchor.max(caret));
            let first = self.line_of(start);
            let mut last = self.line_of(end);
            if end > start && last > first && end == self.line(last).start {
                last -= 1;
            }
            points.extend((first..=last).map(|line| self.line(line).end.min(end)));
        }
        points.sort_unstable();
        points.dedup();
        let mut edits = Vec::with_capacity(points.len());
        for (at, point) in points.into_iter().enumerate() {
            edits.push(if at == 0 {
                Edit::Select(point, point)
            } else {
                Edit::AddSelection(point, point)
            });
        }
        edits
    }
}
