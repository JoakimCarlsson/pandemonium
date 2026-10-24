//! Breaking text into lines that fit a width, measured as it will be drawn.

use pm_style::{Style, Utility};
use pm_text::Shaper;

mod cache;

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
    let mut lines = Vec::new();
    let mut paragraph = 0;
    for source in text.split('\n') {
        let end = paragraph + source.len();
        lines.extend(
            cache::paragraph(shaper, source, typography, width)
                .iter()
                .map(|line| Line {
                    start: paragraph + line.start,
                    end: paragraph + line.end,
                    next: paragraph + line.next,
                }),
        );
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
        if word_end - start <= 256 && fits(&source[start..word_end]) {
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
        loop {
            let split = start + fitting_prefix(&source[start..word_end], fits);
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

/// Finds a fitting prefix with bounded probes, so long unbroken tokens do not shape every suffix.
fn fitting_prefix(source: &str, fits: &mut impl FnMut(&str) -> bool) -> usize {
    let mut boundaries = Vec::with_capacity(64);
    let mut good = 0;
    let mut next = 1;
    for (at, character) in source.char_indices() {
        boundaries.push(at + character.len_utf8());
        if boundaries.len() == next || at + character.len_utf8() == source.len() {
            if fits(&source[..*boundaries.last().expect("character boundary")]) {
                good = boundaries.len();
                next *= 2;
            } else {
                break;
            }
        }
    }
    if good == boundaries.len() {
        return source.len();
    }
    let mut bad = boundaries.len();
    while good + 1 < bad {
        let middle = (good + bad) / 2;
        if fits(&source[..boundaries[middle - 1]]) {
            good = middle;
        } else {
            bad = middle;
        }
    }
    boundaries[good.max(1) - 1]
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

/// Wrapping regression tests cover paragraph reuse and bounded long-token measurement.
#[cfg(test)]
mod tests {
    use super::*;

    /// Long Unicode tokens are tiled without oversized suffix probes or splitting a character.
    #[test]
    fn long_tokens_use_bounded_probes() {
        for width in [0, 1, 7, 24] {
            let source = "café🦀".repeat(2000);
            let mut largest = 0;
            let mut fits = |text: &str| {
                largest = largest.max(text.chars().count());
                text.chars().count() <= width
            };
            let mut lines = Vec::new();
            cut(&source, 0, &mut fits, &mut lines);
            assert_eq!(
                lines
                    .iter()
                    .map(|line| &source[line.start..line.end])
                    .collect::<String>(),
                source
            );
            assert!(
                lines
                    .iter()
                    .all(|line| source[line.start..line.end].chars().count() <= width.max(1))
            );
            assert!(largest <= width.max(1) * 2);
        }
    }

    /// Newlines, blank paragraphs and trailing newlines retain the original byte ranges.
    #[test]
    fn cached_paragraphs_keep_offsets_and_invalidate_fonts() {
        let mut shaper = Shaper::new();
        let text = "é 🦀\n\nend\n";
        let lines = wrap(&mut shaper, text, [pm_style::text_px(14)], 1000.0);
        assert_eq!(
            lines,
            vec![
                Line {
                    start: 0,
                    end: 7,
                    next: 8
                },
                Line {
                    start: 8,
                    end: 8,
                    next: 9
                },
                Line {
                    start: 9,
                    end: 12,
                    next: 13
                },
                Line {
                    start: 13,
                    end: 13,
                    next: 14
                }
            ]
        );
        let typography = pm_style::Style::new([pm_style::text_px(14)])
            .base()
            .typography;
        let first = cache::paragraph(&mut shaper, "unchanged", typography, 100.0);
        assert!(std::sync::Arc::ptr_eq(
            &first,
            &cache::paragraph(&mut shaper, "unchanged", typography, 100.0)
        ));
        assert!(!std::sync::Arc::ptr_eq(
            &first,
            &cache::paragraph(&mut shaper, "unchanged", typography, 20.0)
        ));
        shaper.set_stacks(&[], &[]);
        assert!(!std::sync::Arc::ptr_eq(
            &first,
            &cache::paragraph(&mut shaper, "unchanged", typography, 100.0)
        ));
    }
}
