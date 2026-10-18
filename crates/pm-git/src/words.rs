//! The words that changed inside the lines of a diff: where, in each line taken out or put in, the text differs from the line it was paired with.

use similar::{ChangeTag, TextDiff};

/// The stretches of one line of a diff that are not in the line it was paired with.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Words {
    /// The line, counted from 0 among the lines of the diff.
    pub line: usize,
    /// The stretches that changed, each as the byte where it starts and the byte where it ends in the line without its marker.
    pub ranges: Vec<(usize, usize)>,
}

/// Stretches of a line, each as the byte where it starts and the byte where it ends.
type Stretches = Vec<(usize, usize)>;

/// How long a line may be for its words to be compared, in bytes.
const LONGEST_LINE: usize = 2000;

/// The kind of character a token is made of, so that a run of one kind is one token: letters and digits, blanks, or any other single character.
fn kind_of(character: char) -> u8 {
    if character.is_alphanumeric() || character == '_' {
        0
    } else if character.is_whitespace() {
        1
    } else {
        2
    }
}

/// `text` cut into identifiers, runs of blanks and single punctuation marks.
fn tokens(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = 0;
    let mut previous: Option<u8> = None;
    for (at, character) in text.char_indices() {
        let kind = kind_of(character);
        if at > start && (previous != Some(kind) || kind == 2) {
            found.push(&text[start..at]);
            start = at;
        }
        previous = Some(kind);
    }
    if start < text.len() {
        found.push(&text[start..]);
    }
    found
}

/// The stretches of `old` and of `new` that differ, as byte ranges of each.
fn changed_words(old: &str, new: &str) -> (Stretches, Stretches) {
    let diff = TextDiff::from_slices(&tokens(old), &tokens(new));
    let (mut removed, mut added) = (Vec::new(), Vec::new());
    let (mut old_at, mut new_at) = (0, 0);
    for change in diff.iter_all_changes() {
        let length = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                old_at += length;
                new_at += length;
            }
            ChangeTag::Delete => {
                extend(&mut removed, old_at, old_at + length);
                old_at += length;
            }
            ChangeTag::Insert => {
                extend(&mut added, new_at, new_at + length);
                new_at += length;
            }
        }
    }
    (removed, added)
}

/// Adds the stretch from `start` to `end` to `ranges`, joined to the last one when they touch.
fn extend(ranges: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    match ranges.last_mut() {
        Some(last) if last.1 == start => last.1 = end,
        _ => ranges.push((start, end)),
    }
}

/// Whether a stretch changed covers the whole of a line of `length` bytes, which a highlight of the whole line says nothing more than the line's own tint does.
fn covers(ranges: &[(usize, usize)], length: usize) -> bool {
    matches!(ranges, [(0, end)] if *end == length)
}

/// Pairs the removed lines `removed` with the added lines `added` one by one and puts the words that changed in each pair into `found`.
fn pair(lines: &[&str], removed: &mut Vec<usize>, added: &mut Vec<usize>, found: &mut Vec<Words>) {
    let pairs = std::mem::take(removed)
        .into_iter()
        .zip(std::mem::take(added));
    for (old_line, new_line) in pairs {
        let (old, new) = (&lines[old_line][1..], &lines[new_line][1..]);
        if old.len() > LONGEST_LINE || new.len() > LONGEST_LINE {
            continue;
        }
        let (old_ranges, new_ranges) = changed_words(old, new);
        for (line, ranges, text) in [(old_line, old_ranges, old), (new_line, new_ranges, new)] {
            if !ranges.is_empty() && !covers(&ranges, text.len()) {
                found.push(Words { line, ranges });
            }
        }
    }
}

/// The words that changed in the lines of `diff`, a unified diff from its first hunk on: each removed line is compared with the added line in the same place of the run of changes it is in, and lines with no partner have none.
pub fn words(diff: &str) -> Vec<Words> {
    let lines: Vec<&str> = diff.lines().collect();
    let (mut removed, mut added, mut found) = (Vec::new(), Vec::new(), Vec::new());
    for (at, line) in lines.iter().enumerate() {
        match line.as_bytes().first() {
            Some(b'-') => {
                if !added.is_empty() {
                    pair(&lines, &mut removed, &mut added, &mut found);
                }
                removed.push(at);
            }
            Some(b'+') => added.push(at),
            _ => pair(&lines, &mut removed, &mut added, &mut found),
        }
    }
    pair(&lines, &mut removed, &mut added, &mut found);
    found.sort_by_key(|words| words.line);
    found
}
