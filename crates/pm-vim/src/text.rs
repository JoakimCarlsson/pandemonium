//! Reading a buffer the way modal editing counts it: words, blanks, lines.
//!
//! Word motions cross lines, so they walk character offsets rather than
//! positions; everything that walks is here, and the motions and objects
//! only say where to stop.

use pm_text::{Buffer, Position};

/// The kinds of character a word motion tells apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Class {
    /// A space, a tab or a line break.
    Blank,
    /// A letter, a digit or an underscore; any non-blank for a WORD.
    Word,
    /// Punctuation, which is a word of its own unless counting WORDs.
    Punctuation,
}

/// Which kind of character `ch` is, counting WORDs when `big`.
pub(crate) fn class(ch: char, big: bool) -> Class {
    match ch {
        ch if ch.is_whitespace() => Class::Blank,
        _ if big => Class::Word,
        ch if ch.is_alphanumeric() || ch == '_' => Class::Word,
        _ => Class::Punctuation,
    }
}

/// The class of the character `offset` characters in, a blank past the end.
fn class_at(buffer: &Buffer, offset: usize, big: bool) -> Class {
    buffer
        .char_at_offset(offset)
        .map_or(Class::Blank, |ch| class(ch, big))
}

/// Whether `offset` begins a line that holds nothing.
fn starts_empty_line(buffer: &Buffer, offset: usize) -> bool {
    let position = buffer.position_of(offset);
    offset < buffer.len_chars() && position.column == 0 && buffer.line_len(position.line) == 0
}

/// The column of the first character of `line` that is not blank.
pub(crate) fn first_non_blank(buffer: &Buffer, line: usize) -> usize {
    buffer
        .line_chars(line)
        .take_while(|ch| ch.is_whitespace())
        .count()
}

/// The last column the cursor may rest on in `line` outside insert mode.
pub(crate) fn last_column(buffer: &Buffer, line: usize) -> usize {
    buffer.line_len(line).saturating_sub(1)
}

/// `position` brought onto a character, as the normal-mode cursor must be.
pub(crate) fn on_char(buffer: &Buffer, position: Position) -> Position {
    let position = buffer.clamped(position);
    Position::new(
        position.line,
        position.column.min(last_column(buffer, position.line)),
    )
}

/// The position one character past `position`, onto the next line if need be.
pub(crate) fn after(buffer: &Buffer, position: Position) -> Position {
    let position = buffer.clamped(position);
    if position.column < buffer.line_len(position.line) {
        return Position::new(position.line, position.column + 1);
    }
    match position.line + 1 < buffer.line_count() {
        true => Position::new(position.line + 1, 0),
        false => position,
    }
}

/// Whether `line` holds nothing at all.
pub(crate) fn is_empty_line(buffer: &Buffer, line: usize) -> bool {
    buffer.line_len(line) == 0
}

/// The offset the next word starting after `offset` begins at.
pub(crate) fn next_word_start(buffer: &Buffer, offset: usize, big: bool) -> usize {
    let len = buffer.len_chars();
    let mut at = offset;
    let start = class_at(buffer, at, big);
    if start != Class::Blank {
        while at < len && class_at(buffer, at, big) == start {
            at += 1;
        }
    }
    while at < len {
        match buffer.char_at_offset(at) {
            Some('\n') => {
                at += 1;
                if starts_empty_line(buffer, at) {
                    return at;
                }
            }
            Some(ch) if ch.is_whitespace() => at += 1,
            _ => break,
        }
    }
    at
}

/// The offset the word ending after `offset` ends at, its last character.
pub(crate) fn next_word_end(buffer: &Buffer, offset: usize, big: bool) -> usize {
    let len = buffer.len_chars();
    let mut at = offset + 1;
    while at < len && class_at(buffer, at, big) == Class::Blank {
        at += 1;
    }
    if at >= len {
        return len.saturating_sub(1);
    }
    let kind = class_at(buffer, at, big);
    while at + 1 < len && class_at(buffer, at + 1, big) == kind {
        at += 1;
    }
    at
}

/// The offset the word starting before `offset` begins at.
pub(crate) fn previous_word_start(buffer: &Buffer, offset: usize, big: bool) -> usize {
    if offset == 0 {
        return 0;
    }
    let mut at = offset - 1;
    while at > 0 && class_at(buffer, at, big) == Class::Blank {
        if starts_empty_line(buffer, at) {
            return at;
        }
        at -= 1;
    }
    let kind = class_at(buffer, at, big);
    while at > 0 && class_at(buffer, at - 1, big) == kind {
        at -= 1;
    }
    at
}

/// The offset the word ending before `offset` ends at.
pub(crate) fn previous_word_end(buffer: &Buffer, offset: usize, big: bool) -> usize {
    let mut at = offset;
    let start = class_at(buffer, at, big);
    if start != Class::Blank {
        while at > 0 && class_at(buffer, at, big) == start {
            at -= 1;
        }
    }
    while at > 0 && class_at(buffer, at, big) == Class::Blank {
        if starts_empty_line(buffer, at) {
            return at;
        }
        at -= 1;
    }
    at
}

/// The bracket partner of the one at `offset`, if it is one and has one.
pub(crate) fn matching_bracket(buffer: &Buffer, offset: usize) -> Option<usize> {
    const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];
    let bracket = buffer.char_at_offset(offset)?;
    let (open, close) = PAIRS
        .into_iter()
        .find(|(open, close)| bracket == *open || bracket == *close)?;
    match bracket == open {
        true => enclosing_close(buffer, offset + 1, open, close),
        false => enclosing_open(buffer, offset, open, close),
    }
}

/// The unmatched `open` before `offset`, skipping pairs closed in between.
pub(crate) fn enclosing_open(
    buffer: &Buffer,
    offset: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for at in (0..offset).rev() {
        match buffer.char_at_offset(at)? {
            ch if ch == close => depth += 1,
            ch if ch == open && depth == 0 => return Some(at),
            ch if ch == open => depth -= 1,
            _ => {}
        }
    }
    None
}

/// The unmatched `close` from `offset` on, skipping pairs opened in between.
pub(crate) fn enclosing_close(
    buffer: &Buffer,
    offset: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    for at in offset..buffer.len_chars() {
        match buffer.char_at_offset(at)? {
            ch if ch == open => depth += 1,
            ch if ch == close && depth == 0 => return Some(at),
            ch if ch == close => depth -= 1,
            _ => {}
        }
    }
    None
}
