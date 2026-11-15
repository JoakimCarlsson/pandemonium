//! How a line of code is laid out in a monospace face: the column of each
//! byte with tabs expanded, the byte under a point, the colour of each
//! highlight and the runs of one colour a line is painted in.

use crate::app_theme::ExtendedColors;
use std::ops::Range;

use pm_treesitter::Kind;
use rustle::Color;

use rustle::ColorScheme;

/// How many columns a tab advances to the next multiple of.
pub(crate) const TAB_COLUMNS: usize = 4;

/// The display column at which byte `byte` of `line` starts, tabs expanded.
pub(crate) fn column_of(line: &str, byte: usize) -> usize {
    let mut column = 0;
    for (at, character) in line.char_indices() {
        if at >= byte {
            break;
        }
        column = advance(column, character);
    }
    column
}

/// The column after `character` written at `column`.
fn advance(column: usize, character: char) -> usize {
    if character == '\t' {
        column + TAB_COLUMNS - column % TAB_COLUMNS
    } else {
        column + 1
    }
}

/// The byte of `line` whose start is nearest display column `column`.
pub(crate) fn byte_at_column(line: &str, column: f32) -> usize {
    let mut at_column = 0;
    for (at, character) in line.char_indices() {
        let next = advance(at_column, character);
        if column < (at_column + next) as f32 / 2.0 {
            return at;
        }
        at_column = next;
    }
    line.len()
}

/// `text`, which starts at display column `column`, with each tab written
/// as the spaces it spans.
pub(crate) fn expand_tabs(text: &str, column: usize) -> String {
    let mut written = String::with_capacity(text.len());
    let mut at = column;
    for character in text.chars() {
        let next = advance(at, character);
        if character == '\t' {
            written.extend(std::iter::repeat_n(' ', next - at));
        } else {
            written.push(character);
        }
        at = next;
    }
    written
}

/// The colour code highlighted as `kind` is painted in.
pub(crate) fn kind_colour(kind: Kind, scheme: &ColorScheme, extended: &ExtendedColors) -> Color {
    match kind {
        Kind::Keyword => extended.syntax_keyword,
        Kind::Function => extended.syntax_function,
        Kind::Type => extended.syntax_type,
        Kind::String => extended.syntax_string,
        Kind::Constant => extended.syntax_constant,
        Kind::Comment => extended.syntax_comment,
        Kind::Property => extended.syntax_property,
        Kind::Operator | Kind::Punctuation => extended.syntax_punctuation,
        Kind::Variable => scheme.on_surface,
        Kind::Markup => extended.syntax_keyword,
        Kind::Link => scheme.primary,
    }
}

/// The runs of a line `length` bytes long that share a highlight, as the
/// stretches `spans` gives them, later ones over earlier ones; bytes no
/// span covers have none.
pub(crate) fn runs(
    length: usize,
    spans: &[(Range<usize>, Kind)],
) -> Vec<(Range<usize>, Option<Kind>)> {
    let mut kinds: Vec<Option<Kind>> = vec![None; length];
    for (range, kind) in spans {
        let end = range.end.min(length);
        for slot in kinds.iter_mut().take(end).skip(range.start) {
            *slot = Some(*kind);
        }
    }
    let mut runs = Vec::new();
    let mut start = 0;
    for index in 1..=length {
        if index == length || kinds[index] != kinds[start] {
            runs.push((start..index, kinds[start]));
            start = index;
        }
    }
    runs
}

/// The number of decimal digits of `value`, at least one.
pub(crate) fn digits(value: usize) -> usize {
    value.max(1).ilog10() as usize + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tabs advance to the next stop, both ways.
    #[test]
    fn tabs_expand_to_stops() {
        assert_eq!(column_of("\tab", 1), 4);
        assert_eq!(column_of("a\tb", 2), 4);
        assert_eq!(byte_at_column("\tab", 4.2), 1);
        assert_eq!(byte_at_column("\tab", 1.0), 0);
        assert_eq!(byte_at_column("ab", 9.0), 2);
        assert_eq!(expand_tabs("a\tb", 0), "a   b");
    }

    /// Spans split a line into runs, with gaps left plain.
    #[test]
    fn spans_make_runs() {
        let made = runs(6, &[(1..3, Kind::Keyword)]);
        assert_eq!(
            made,
            vec![(0..1, None), (1..3, Some(Kind::Keyword)), (3..6, None)]
        );
        assert!(runs(0, &[]).is_empty());
    }

    /// Digits are counted with one for zero.
    #[test]
    fn digits_counts() {
        assert_eq!(digits(0), 1);
        assert_eq!(digits(9), 1);
        assert_eq!(digits(10), 2);
        assert_eq!(digits(1234), 4);
    }
}
