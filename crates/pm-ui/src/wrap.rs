//! Breaking text into lines that fit a width, measured as it will be drawn.

use pm_style::{Style, Utility};
use pm_text::Shaper;

/// One line of wrapped text, as byte offsets into the text it was cut from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Line {
    /// Where the line starts.
    pub start: usize,
    /// Where it ends, after any spaces it keeps at the break but before a line break.
    pub end: usize,
    /// Where the next line starts: the same place when the line was cut for
    /// want of room, past the line break when the text had one.
    pub next: usize,
}

/// The lines `text` breaks into in `width` logical pixels when it is set in
/// the typography `styles` describe.
///
/// A line break in the text always breaks a line, and so does a word that
/// would not fit; a word wider than the width is cut where it must be. The
/// lines tile the text: none is empty unless the text has a blank line, and
/// at least one is always returned.
pub fn wrap(
    shaper: &mut Shaper,
    text: &str,
    styles: impl IntoIterator<Item = Utility>,
    width: f32,
) -> Vec<Line> {
    let mut style = Style::default();
    style.extend(styles);
    let typography = style.base().typography;
    let mut fits = |piece: &str| shaper.measure(piece.trim_end(), &typography).width <= width;
    let mut lines = Vec::new();
    let mut paragraph = 0;
    for source in text.split('\n') {
        let end = paragraph + source.len();
        cut(source, paragraph, &mut fits, &mut lines);
        if let Some(last) = lines.last_mut() {
            last.next = end + 1;
        }
        paragraph = end + 1;
    }
    lines
}

/// Cuts the paragraph `source`, which starts at byte `base` of the text, into lines that `fits`.
fn cut(source: &str, base: usize, fits: &mut impl FnMut(&str) -> bool, lines: &mut Vec<Line>) {
    let mut start = 0;
    let mut cursor = 0;
    for word_end in word_ends(source) {
        if fits(&source[start..word_end]) {
            cursor = word_end;
            continue;
        }
        if cursor > start {
            lines.push(Line {
                start: base + start,
                end: base + cursor,
                next: base + cursor,
            });
            start = cursor;
        }
        while !fits(&source[start..word_end]) {
            let mut split = start;
            for (at, character) in source[start..word_end].char_indices() {
                let after = start + at + character.len_utf8();
                if split > start && !fits(&source[start..after]) {
                    break;
                }
                split = after;
            }
            if split >= word_end {
                break;
            }
            lines.push(Line {
                start: base + start,
                end: base + split,
                next: base + split,
            });
            start = split;
        }
        cursor = word_end;
    }
    lines.push(Line {
        start: base + start,
        end: base + source.len(),
        next: base + source.len(),
    });
}

/// The offsets at which each word of `source` ends, taking the spaces after it along.
fn word_ends(source: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut spaced = false;
    for (at, character) in source.char_indices() {
        if character == ' ' {
            spaced = true;
        } else if spaced {
            ends.push(at);
            spaced = false;
        }
    }
    ends.push(source.len());
    ends
}
