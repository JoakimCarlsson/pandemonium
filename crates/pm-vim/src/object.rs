//! Text objects: the word, the quotes or the brackets the cursor is in.
//!
//! An object is a span found around the cursor rather than reached from it,
//! which is why an operator given one does not care where in the object the
//! cursor stands. Each comes in two sizes: inner, the contents alone, and
//! around, the contents with what delimits them.

use pm_text::{Buffer, Position};

use crate::operator::Span;
use crate::text::{self, Class, class, first_non_blank, is_empty_line};

/// A text object: what `i` or `a` is followed by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Object {
    /// `w` or `W`: a word.
    Word { big: bool },
    /// `"`, `'` or `` ` ``: a quoted string on one line.
    Quotes(char),
    /// `(`, `[`, `{` or `<`, or `b` and `B`: a bracketed block.
    Brackets(char, char),
    /// `p`: a paragraph.
    Paragraph,
}

impl Object {
    /// The object a key after `i` or `a` names.
    pub(crate) fn of(ch: char) -> Option<Self> {
        Some(match ch {
            'w' => Self::Word { big: false },
            'W' => Self::Word { big: true },
            '"' | '\'' | '`' => Self::Quotes(ch),
            '(' | ')' | 'b' => Self::Brackets('(', ')'),
            '[' | ']' => Self::Brackets('[', ']'),
            '{' | '}' | 'B' => Self::Brackets('{', '}'),
            '<' | '>' => Self::Brackets('<', '>'),
            'p' => Self::Paragraph,
            _ => return None,
        })
    }

    /// The span of the object around `at`, its delimiters too when `around`.
    pub(crate) fn span(self, buffer: &Buffer, at: Position, around: bool) -> Option<Span> {
        match self {
            Self::Word { big } => word(buffer, at, big, around),
            Self::Quotes(quote) => quoted(buffer, at, quote, around),
            Self::Brackets(open, close) => bracketed(buffer, at, open, close, around),
            Self::Paragraph => paragraph(buffer, at, around),
        }
    }
}

/// The word at `at`, with the blanks after it (or before it) when `around`.
fn word(buffer: &Buffer, at: Position, big: bool, around: bool) -> Option<Span> {
    let chars = buffer.line_chars(at.line).collect::<Vec<_>>();
    if chars.is_empty() {
        return None;
    }
    let column = at.column.min(chars.len() - 1);
    let kind = class(chars[column], big);
    let same = |index: usize| class(chars[index], big) == kind;

    let mut start = column;
    while start > 0 && same(start - 1) {
        start -= 1;
    }
    let mut end = column + 1;
    while end < chars.len() && same(end) {
        end += 1;
    }

    if around {
        let blank = |index: usize| class(chars[index], big) == Class::Blank;
        if kind == Class::Blank {
            let next = class(*chars.get(end)?, big);
            while end < chars.len() && class(chars[end], big) == next {
                end += 1;
            }
        } else if end < chars.len() && blank(end) {
            while end < chars.len() && blank(end) {
                end += 1;
            }
        } else {
            while start > 0 && blank(start - 1) {
                start -= 1;
            }
        }
    }
    Some(Span::chars(
        Position::new(at.line, start),
        Position::new(at.line, end),
    ))
}

/// The string quoted by `quote` around `at` on its line, or the next one.
fn quoted(buffer: &Buffer, at: Position, quote: char, around: bool) -> Option<Span> {
    let chars = buffer.line_chars(at.line).collect::<Vec<_>>();
    let quotes = chars
        .iter()
        .enumerate()
        .filter(|(index, ch)| **ch == quote && (*index == 0 || chars[index - 1] != '\\'))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let pairs = quotes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[open, close]| (*open, *close));
    let (open, close) = pairs
        .clone()
        .find(|(open, close)| (*open..=*close).contains(&at.column))
        .or_else(|| pairs.clone().find(|(open, _)| *open > at.column))?;

    let (mut start, mut end) = match around {
        true => (open, close + 1),
        false => (open + 1, close),
    };
    if around {
        let blank = |index: usize| chars.get(index).is_some_and(|ch| ch.is_whitespace());
        if blank(end) {
            while blank(end) {
                end += 1;
            }
        } else {
            while start > 0 && blank(start - 1) {
                start -= 1;
            }
        }
    }
    Some(Span::chars(
        Position::new(at.line, start),
        Position::new(at.line, end),
    ))
}

/// The block between `open` and `close` around `at`.
///
/// A block whose brackets stand at the end of one line and the start of
/// another is taken, inside, as the whole lines between them, so that
/// emptying it leaves the brackets where they were.
fn bracketed(buffer: &Buffer, at: Position, open: char, close: char, around: bool) -> Option<Span> {
    let offset = buffer.char_of(at);
    let start = match buffer.char_at_offset(offset) {
        Some(ch) if ch == open => offset,
        Some(ch) if ch == close => text::enclosing_open(buffer, offset, open, close)?,
        _ => text::enclosing_open(buffer, offset, open, close)?,
    };
    let end = text::enclosing_close(buffer, start + 1, open, close)?;
    if around {
        return Some(Span::chars(
            buffer.position_of(start),
            buffer.position_of(end + 1),
        ));
    }

    let (opening, closing) = (buffer.position_of(start), buffer.position_of(end));
    let opens_line = buffer
        .line_chars(opening.line)
        .skip(opening.column + 1)
        .all(char::is_whitespace);
    let closes_line = closing.column == first_non_blank(buffer, closing.line);
    if opens_line && closes_line && closing.line > opening.line + 1 {
        return Some(Span::lines(opening.line + 1, closing.line - 1));
    }
    Some(Span::chars(buffer.position_of(start + 1), closing))
}

/// The paragraph at `at`, with the empty lines after it (or before) when
/// `around`.
fn paragraph(buffer: &Buffer, at: Position, around: bool) -> Option<Span> {
    let last = buffer.line_count().saturating_sub(1);
    let empty = is_empty_line(buffer, at.line);
    let same = |line: usize| is_empty_line(buffer, line) == empty;

    let mut first = at.line;
    while first > 0 && same(first - 1) {
        first -= 1;
    }
    let mut end = at.line;
    while end < last && same(end + 1) {
        end += 1;
    }
    if around {
        let other = |line: usize| is_empty_line(buffer, line) != empty;
        if end < last && other(end + 1) {
            end += 1;
            while end < last && other(end + 1) {
                end += 1;
            }
        } else {
            while first > 0 && other(first - 1) {
                first -= 1;
            }
        }
    }
    Some(Span::lines(first, end))
}
