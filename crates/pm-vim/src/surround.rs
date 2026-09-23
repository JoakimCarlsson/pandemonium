//! Surrounding text with a pair of delimiters, and taking or changing the
//! pair around it: vim-surround's `ys`, `ds` and `cs`, as Zed has them.

use pm_text::{Buffer, Position};

use crate::object::Object;
use crate::operator::Span;

/// The pair a surround character stands for, and whether the text inside
/// gets a space on each side.
///
/// An opening bracket pads what it wraps and a closing one does not, which
/// is how `ys iw (` and `ys iw )` come to differ.
pub(crate) fn pair(ch: char) -> Option<(String, String)> {
    let (open, close, padded) = match ch {
        '(' => ('(', ')', true),
        ')' | 'b' => ('(', ')', false),
        '[' => ('[', ']', true),
        ']' | 'r' => ('[', ']', false),
        '{' => ('{', '}', true),
        '}' | 'B' => ('{', '}', false),
        '<' => ('<', '>', true),
        '>' | 'a' => ('<', '>', false),
        '"' | '\'' | '`' | '|' | '*' | '_' | '/' => (ch, ch, false),
        _ => return None,
    };
    let pad = if padded { " " } else { "" };
    Some((format!("{open}{pad}"), format!("{pad}{close}")))
}

/// The object whose delimiters a surround character names, for `ds` and
/// `cs` to find.
fn object(ch: char) -> Option<Object> {
    Some(match ch {
        '(' | ')' | 'b' => Object::Brackets('(', ')'),
        '[' | ']' | 'r' => Object::Brackets('[', ']'),
        '{' | '}' | 'B' => Object::Brackets('{', '}'),
        '<' | '>' | 'a' => Object::Brackets('<', '>'),
        '"' | '\'' | '`' => Object::Quotes(ch),
        'q' => Object::AnyQuotes,
        't' => Object::Tag,
        _ => return None,
    })
}

/// Puts the pair `ch` stands for around `span`, answering where the cursor
/// goes: onto the opening delimiter.
pub(crate) fn add(buffer: &mut Buffer, span: Span, ch: char) -> Option<Position> {
    let (open, close) = pair(ch)?;
    let (start, end) = match span.linewise {
        true => (
            Position::new(
                span.start.line,
                crate::text::first_non_blank(buffer, span.start.line),
            ),
            Position::new(span.end.line, buffer.line_len(span.end.line)),
        ),
        false => (span.start, span.end),
    };
    buffer.grouped(|buffer| {
        buffer.replace(end..end, &close);
        buffer.replace(start..start, &open);
    });
    Some(start)
}

/// The spans of the delimiters `ch` names around `at`, the opening one first.
fn delimiters(buffer: &Buffer, at: Position, ch: char) -> Option<(Span, Span)> {
    let object = object(ch)?;
    let outer = object.span(buffer, at, true, 1)?;
    let inner = object.span(buffer, at, false, 1)?;
    let (outer_start, outer_end) = (outer.start, outer.end);
    if inner.linewise {
        return Some((
            Span::chars(outer_start, crate::text::after(buffer, outer_start)),
            Span::chars(crate::text::before(buffer, outer_end), outer_end),
        ));
    }
    let (inner_start, inner_end) = (inner.start, inner.end);
    let quoted = matches!(object, Object::Quotes(_) | Object::AnyQuotes);
    let outer_end = match quoted {
        true => {
            let text = buffer.text_in(inner_end..outer_end);
            let kept = text.trim_end().chars().count();
            Position::new(inner_end.line, inner_end.column + kept)
        }
        false => outer_end,
    };
    let outer_start = match quoted {
        true => {
            let text = buffer.text_in(outer_start..inner_start);
            let blanks = text.chars().count() - text.trim_start().chars().count();
            Position::new(outer_start.line, outer_start.column + blanks)
        }
        false => outer_start,
    };
    Some((
        Span::chars(outer_start, inner_start),
        Span::chars(inner_end, outer_end),
    ))
}

/// Takes away the pair `ch` names around `at`, answering where the cursor
/// goes.
pub(crate) fn delete(buffer: &mut Buffer, at: Position, ch: char) -> Option<Position> {
    let (open, close) = delimiters(buffer, at, ch)?;
    buffer.grouped(|buffer| {
        buffer.replace(close.start..close.end, "");
        buffer.replace(open.start..open.end, "");
    });
    Some(open.start)
}

/// Puts the pair `to` stands for in place of the pair `from` names around
/// `at`, answering where the cursor goes.
pub(crate) fn change(buffer: &mut Buffer, at: Position, from: char, to: char) -> Option<Position> {
    let (open, close) = delimiters(buffer, at, from)?;
    let (opening, closing) = pair(to)?;
    buffer.grouped(|buffer| {
        buffer.replace(close.start..close.end, &closing);
        buffer.replace(open.start..open.end, &opening);
    });
    Some(open.start)
}
