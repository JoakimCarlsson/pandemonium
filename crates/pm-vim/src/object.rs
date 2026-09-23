//! Text objects: the word, the quotes, the brackets, the function the
//! cursor is in.
//!
//! An object is a span found around the cursor rather than reached from it,
//! which is why an operator given one does not care where in the object the
//! cursor stands. Each comes in two sizes: inner, the contents alone, and
//! around, the contents with what delimits them.

use pm_text::{Buffer, Position};

use crate::operator::Span;
use crate::syntax;
use crate::text::{self, Class, class, first_non_blank, indent_width, is_empty_line};

/// A text object: what `i` or `a` is followed by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Object {
    /// `w` or `W`: a word.
    Word { big: bool },
    /// `s`: a sentence.
    Sentence,
    /// `p`: a paragraph.
    Paragraph,
    /// `"`, `'`, `` ` `` or `|`: a quoted string on one line.
    Quotes(char),
    /// `q`: whichever quotes are nearest around the cursor.
    AnyQuotes,
    /// `(`, `[`, `{` or `<`, or `b`, `r` and `B`: a bracketed block.
    Brackets(char, char),
    /// `a`: one argument of a call or a parameter list.
    Argument,
    /// `t`: an HTML or XML element.
    Tag,
    /// `i` or `I`: the block of lines indented as far as this one, with the
    /// line below it too for `I`.
    Indent { below: bool },
    /// `f`: a function or method.
    Function,
    /// `c`: a class, struct or something like one.
    Class,
    /// `e`: the whole file.
    EntireFile,
}

impl Object {
    /// The span of the object around `at`, its delimiters too when `around`,
    /// `times` levels out where the object nests.
    pub(crate) fn span(
        self,
        buffer: &Buffer,
        at: Position,
        around: bool,
        times: usize,
    ) -> Option<Span> {
        let times = times.max(1);
        match self {
            Self::Word { big } => word(buffer, at, big, around, times),
            Self::Sentence => sentence(buffer, at, around),
            Self::Paragraph => paragraph(buffer, at, around, times),
            Self::Quotes(quote) => quoted(buffer, at, quote, around),
            Self::AnyQuotes => ['"', '\'', '`']
                .into_iter()
                .filter_map(|quote| quoted(buffer, at, quote, around))
                .filter(|span| span.start <= at && at <= span.end)
                .min_by_key(|span| buffer.char_of(span.end) - buffer.char_of(span.start)),
            Self::Brackets(open, close) => bracketed(buffer, at, open, close, around, times),
            Self::Argument => argument(buffer, at, around),
            Self::Tag => tag(buffer, at, around, times),
            Self::Indent { below } => indented(buffer, at, around, below),
            Self::Function => node(buffer, at, around, &syntax::is_function),
            Self::Class => node(buffer, at, around, &syntax::is_class),
            Self::EntireFile => Some(Span::lines(0, buffer.line_count().saturating_sub(1))),
        }
    }
}

/// The word at `at`, with the blanks after it (or before it) when `around`,
/// and the `times - 1` runs that follow.
fn word(buffer: &Buffer, at: Position, big: bool, around: bool, times: usize) -> Option<Span> {
    let chars = buffer.line_chars(at.line).collect::<Vec<_>>();
    if chars.is_empty() {
        return None;
    }
    let column = at.column.min(chars.len() - 1);
    let kind = |index: usize| class(chars[index], big);
    let blank = |index: usize| kind(index) == Class::Blank;
    let run_end = |from: usize| {
        let mut end = from + 1;
        while end < chars.len() && kind(end) == kind(from) {
            end += 1;
        }
        end
    };

    let mut start = column;
    while start > 0 && kind(start - 1) == kind(column) {
        start -= 1;
    }
    let mut end = run_end(column);
    let on_blank = blank(column);
    for _ in 1..times {
        if end >= chars.len() {
            break;
        }
        end = run_end(end);
    }
    if around {
        if on_blank {
            if end < chars.len() {
                end = run_end(end);
            }
        } else if end < chars.len() && blank(end) {
            end = run_end(end);
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

/// The sentence at `at`, with the blanks after it when `around`.
fn sentence(buffer: &Buffer, at: Position, around: bool) -> Option<Span> {
    let offset = buffer.char_of(at);
    let start = text::sentence_start(buffer, offset);
    let next = text::next_sentence(buffer, offset);
    let next = match next + 1 == buffer.len_chars() {
        true => buffer.len_chars(),
        false => next,
    };
    let mut end = next;
    if !around {
        while end > start
            && buffer
                .char_at_offset(end - 1)
                .is_some_and(char::is_whitespace)
        {
            end -= 1;
        }
    }
    Some(Span::chars(
        buffer.position_of(start),
        buffer.position_of(end),
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

/// The offsets of the `open` and `close` around `at`, `times` levels out.
fn brackets_around(
    buffer: &Buffer,
    at: Position,
    open: char,
    close: char,
    times: usize,
) -> Option<(usize, usize)> {
    let offset = buffer.char_of(at);
    let mut start = match buffer.char_at_offset(offset) {
        Some(ch) if ch == open => offset,
        _ => text::enclosing_open(buffer, offset, open, close)?,
    };
    for _ in 1..times {
        start = text::enclosing_open(buffer, start, open, close)?;
    }
    let end = text::enclosing_close(buffer, start + 1, open, close)?;
    Some((start, end))
}

/// The block between `open` and `close` around `at`.
///
/// A block whose brackets stand at the end of one line and the start of
/// another is taken, inside, as the whole lines between them, so that
/// emptying it leaves the brackets where they were.
fn bracketed(
    buffer: &Buffer,
    at: Position,
    open: char,
    close: char,
    around: bool,
    times: usize,
) -> Option<Span> {
    let (start, end) = brackets_around(buffer, at, open, close, times)?;
    if around {
        return Some(Span::chars(
            buffer.position_of(start),
            buffer.position_of(end + 1),
        ));
    }
    Some(inside(
        buffer,
        buffer.position_of(start),
        buffer.position_of(end),
    ))
}

/// What lies between an opening delimiter at `opening` and a closing one at
/// `closing`, as whole lines when each stands at the edge of its line.
fn inside(buffer: &Buffer, opening: Position, closing: Position) -> Span {
    let opens_line = buffer
        .line_chars(opening.line)
        .skip(opening.column + 1)
        .all(char::is_whitespace);
    let closes_line = closing.column == first_non_blank(buffer, closing.line);
    if opens_line && closes_line && closing.line > opening.line + 1 {
        return Span::lines(opening.line + 1, closing.line - 1);
    }
    Span::chars(text::after(buffer, opening), closing)
}

/// The argument at `at` in the innermost brackets around it.
///
/// Arguments are split at the commas that are not inside a nested bracket
/// or string; around one takes the comma after it, or the one before it
/// when it is the last.
fn argument(buffer: &Buffer, at: Position, around: bool) -> Option<Span> {
    let (start, end) = [('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')]
        .into_iter()
        .filter_map(|(open, close)| brackets_around(buffer, at, open, close, 1))
        .filter(|(start, _)| buffer.char_of(at) > *start)
        .max_by_key(|(start, _)| *start)?;
    let mut commas = Vec::new();
    let mut depth = 0i32;
    let mut quote = None;
    for offset in start + 1..end {
        let ch = buffer.char_at_offset(offset)?;
        match (quote, ch) {
            (Some(open), ch) if ch == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`') => quote = Some(ch),
            (None, '(' | '[' | '{' | '<') => depth += 1,
            (None, ')' | ']' | '}' | '>') => depth -= 1,
            (None, ',') if depth == 0 => commas.push(offset),
            _ => {}
        }
    }
    let cursor = buffer.char_of(at);
    let bounds = std::iter::once(start)
        .chain(commas.iter().copied())
        .chain(std::iter::once(end))
        .collect::<Vec<_>>();
    let index = bounds.windows(2).position(|pair| cursor <= pair[1])?;
    let (left, right) = (bounds[index], bounds[index + 1]);
    let blank = |offset: usize| {
        buffer
            .char_at_offset(offset)
            .is_some_and(char::is_whitespace)
    };

    let mut from = left + 1;
    while from < right && blank(from) {
        from += 1;
    }
    let mut to = right;
    while to > from && blank(to - 1) {
        to -= 1;
    }
    if around {
        if right < end {
            to = right + 1;
            while to < end && blank(to) {
                to += 1;
            }
        } else if left > start {
            from = left;
        }
    }
    Some(Span::chars(
        buffer.position_of(from),
        buffer.position_of(to),
    ))
}

/// The element around `at`, `times` levels out: its contents, or the whole
/// of it with its tags when `around`.
fn tag(buffer: &Buffer, at: Position, around: bool, times: usize) -> Option<Span> {
    let contents = buffer.contents();
    let tags = regex::Regex::new(r"<(/?)([A-Za-z][\w:.-]*)[^<>]*?(/?)>").ok()?;
    let offset_of = |byte: usize| contents[..byte].chars().count();
    let mut open = Vec::<(String, usize, usize)>::new();
    let mut elements = Vec::new();
    for found in tags.captures_iter(&contents) {
        let whole = found.get(0)?;
        let (closing, name) = (&found[1] == "/", found[2].to_owned());
        if &found[3] == "/" {
            continue;
        }
        let (start, end) = (offset_of(whole.start()), offset_of(whole.end()));
        match closing {
            false => open.push((name, start, end)),
            true => {
                if let Some(index) = open.iter().rposition(|(opened, _, _)| *opened == name) {
                    let (_, outer, inner) = open.remove(index);
                    open.truncate(index);
                    elements.push((outer, inner, start, end));
                }
            }
        }
    }
    let cursor = buffer.char_of(at);
    let mut around_cursor = elements
        .into_iter()
        .filter(|(outer, _, _, end)| *outer <= cursor && cursor < *end)
        .collect::<Vec<_>>();
    around_cursor.sort_by_key(|(outer, _, _, end)| end - outer);
    let (outer, inner, close, end) = *around_cursor.get(times - 1)?;
    Some(match around {
        true => Span::chars(buffer.position_of(outer), buffer.position_of(end)),
        false => Span::chars(buffer.position_of(inner), buffer.position_of(close)),
    })
}

/// The block of lines around `at` indented at least as far as its line,
/// with the line above when `around` and the line below when `below`.
fn indented(buffer: &Buffer, at: Position, around: bool, below: bool) -> Option<Span> {
    let last = buffer.line_count().saturating_sub(1);
    let blank = |line: usize| buffer.line_text(line).trim().is_empty();
    let line = (at.line..=last)
        .chain((0..at.line).rev())
        .find(|line| !blank(*line))?;
    let width = indent_width(buffer, line);
    let inside = |line: usize| blank(line) || indent_width(buffer, line) >= width;

    let mut first = line;
    while first > 0 && inside(first - 1) {
        first -= 1;
    }
    let mut end = line;
    while end < last && inside(end + 1) {
        end += 1;
    }
    while first < end && blank(first) {
        first += 1;
    }
    while end > first && blank(end) {
        end -= 1;
    }
    if around && first > 0 {
        first -= 1;
    }
    if below && end < last {
        end += 1;
    }
    Some(Span::lines(first, end))
}

/// The syntax node around `at` that `keep` accepts: the whole of it when
/// `around`, its body without the braces otherwise.
fn node(buffer: &Buffer, at: Position, around: bool, keep: &dyn Fn(&str) -> bool) -> Option<Span> {
    let found = syntax::enclosing(buffer, at, keep)?;
    let whole = found.range.clone();
    if around {
        let starts_line = whole.start.column <= first_non_blank(buffer, whole.start.line);
        let ends_line = buffer
            .line_chars(whole.end.line)
            .skip(whole.end.column)
            .all(char::is_whitespace);
        return Some(match starts_line && ends_line {
            true => Span::lines(whole.start.line, whole.end.line),
            false => Span::chars(whole.start, whole.end),
        });
    }
    let body = found.body.unwrap_or(whole);
    let opening = body.start;
    let closing = text::before(buffer, body.end);
    let braced = matches!(buffer.char_at(opening), Some('{' | '(' | '[' | ':'))
        && matches!(buffer.char_at(closing), Some('}' | ')' | ']'));
    Some(match braced {
        true => inside(buffer, opening, closing),
        false => Span::chars(body.start, body.end),
    })
}

/// The paragraph at `at`, with the empty lines after it (or before) when
/// `around`, and the `times - 1` paragraphs that follow.
fn paragraph(buffer: &Buffer, at: Position, around: bool, times: usize) -> Option<Span> {
    let last = buffer.line_count().saturating_sub(1);
    let empty = is_empty_line(buffer, at.line);
    let same = |line: usize, empty: bool| is_empty_line(buffer, line) == empty;

    let mut first = at.line;
    while first > 0 && same(first - 1, empty) {
        first -= 1;
    }
    let mut end = at.line;
    let mut runs = if around { times * 2 } else { times };
    let mut kind = empty;
    loop {
        while end < last && same(end + 1, kind) {
            end += 1;
        }
        runs -= 1;
        if runs == 0 || end >= last {
            break;
        }
        end += 1;
        kind = !kind;
    }
    if around && runs > 0 {
        while first > 0 && !same(first - 1, empty) {
            first -= 1;
        }
    }
    Some(Span::lines(first, end))
}
