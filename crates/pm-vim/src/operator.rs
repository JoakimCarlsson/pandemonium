//! Operators: what `d`, `c`, `y` and the rest do to the span they are given.
//!
//! A motion or an object decides the span; the operator decides what happens
//! to it. Every operator is one undo step, however many replacements it
//! takes, because it is one thing the reader asked for.

use pm_text::{Buffer, Position, Selection};

use crate::motion::{Kind, Moved};
use crate::register::{Clipboard, Filling, Registers};
use crate::text::{self, first_non_blank};

/// What an operator does to its span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operator {
    /// `d`: take the span out, into a register.
    Delete,
    /// `c`: take the span out and type in its place.
    Change,
    /// `y`: copy the span into a register.
    Yank,
    /// `>`: indent the lines the span touches.
    Indent,
    /// `<`: outdent the lines the span touches.
    Outdent,
    /// `gu`: lower the case of the span.
    Lowercase,
    /// `gU`: raise the case of the span.
    Uppercase,
    /// `g~`: swap the case of the span.
    ToggleCase,
}

/// A span of text an operator acts on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Span {
    /// Where it begins.
    pub start: Position,
    /// Where it ends: the character after it, or its last line when linewise.
    pub end: Position,
    /// Whether it is whole lines.
    pub linewise: bool,
}

impl Span {
    /// The characters from `start` up to `end`.
    pub(crate) fn chars(start: Position, end: Position) -> Self {
        Self {
            start: start.min(end),
            end: start.max(end),
            linewise: false,
        }
    }

    /// The lines from `first` to `last`, whole.
    pub(crate) fn lines(first: usize, last: usize) -> Self {
        Self {
            start: Position::new(first.min(last), 0),
            end: Position::new(first.max(last), 0),
            linewise: true,
        }
    }

    /// The span a motion from `from` covers.
    ///
    /// An exclusive motion that ends at the start of a later line stops at
    /// the end of the line before instead, and becomes linewise when it
    /// began at or before the first non-blank: that is what makes `d}` take
    /// a paragraph without the empty line after it.
    pub(crate) fn of_motion(buffer: &Buffer, from: Position, moved: Moved) -> Self {
        let (start, end) = (from.min(moved.to), from.max(moved.to));
        match moved.kind {
            Kind::Linewise => Self::lines(start.line, end.line),
            Kind::Inclusive => Self::chars(start, text::after(buffer, end)),
            Kind::Exclusive if end.column == 0 && end.line > start.line => {
                match start.column <= first_non_blank(buffer, start.line) {
                    true => Self::lines(start.line, end.line - 1),
                    false => Self::chars(
                        start,
                        Position::new(end.line - 1, buffer.line_len(end.line - 1)),
                    ),
                }
            }
            Kind::Exclusive => Self::chars(start, end),
        }
    }

    /// The first and last line the span touches.
    pub(crate) fn line_range(&self) -> (usize, usize) {
        (self.start.line, self.end.line)
    }

    /// The text the span covers, a line break after every line when linewise.
    fn text(&self, buffer: &Buffer) -> String {
        match self.linewise {
            true => {
                let (first, last) = self.line_range();
                (first..=last)
                    .map(|line| format!("{}\n", buffer.line_text(line)))
                    .collect()
            }
            false => buffer.text_in(self.start..self.end),
        }
    }

    /// The range taking the span out removes, its lines' breaks included.
    ///
    /// The last line has no break after it, so taking it out takes the
    /// break before it instead.
    fn removal(&self, buffer: &Buffer) -> std::ops::Range<Position> {
        if !self.linewise {
            return self.start..self.end;
        }
        let (first, last) = self.line_range();
        if last + 1 < buffer.line_count() {
            return Position::new(first, 0)..Position::new(last + 1, 0);
        }
        let end = Position::new(last, buffer.line_len(last));
        match first {
            0 => Position::new(0, 0)..end,
            _ => Position::new(first - 1, buffer.line_len(first - 1))..end,
        }
    }
}

/// Where a register comes from and goes to, for an operator to fill.
pub(crate) struct Store<'a> {
    /// Every register.
    pub registers: &'a mut Registers,
    /// The register named for this command, if one was.
    pub register: Option<char>,
    /// The system clipboard.
    pub clipboard: &'a mut dyn Clipboard,
}

impl Store<'_> {
    /// Keeps `text` as `filling` says.
    pub(crate) fn fill(&mut self, text: String, filling: Filling) {
        self.registers
            .fill(self.register, text, filling, &mut *self.clipboard);
    }
}

impl Operator {
    /// The operator a key names, after `g` when `after_g`.
    pub(crate) fn of(ch: char, after_g: bool) -> Option<Self> {
        Some(match (after_g, ch) {
            (false, 'd') => Self::Delete,
            (false, 'c') => Self::Change,
            (false, 'y') => Self::Yank,
            (false, '>') => Self::Indent,
            (false, '<') => Self::Outdent,
            (true, 'u') => Self::Lowercase,
            (true, 'U') => Self::Uppercase,
            (true, '~') => Self::ToggleCase,
            _ => return None,
        })
    }

    /// Whether the operator changes the text, and so is repeated by `.`.
    pub(crate) fn changes(self) -> bool {
        self != Self::Yank
    }

    /// Carries the operator out on `span`, from a cursor at `from`.
    ///
    /// Answers where the cursor ends up; a change leaves it where typing
    /// should begin.
    pub(crate) fn apply(
        self,
        buffer: &mut Buffer,
        span: Span,
        from: Position,
        store: &mut Store,
    ) -> Position {
        match self {
            Self::Delete => delete(buffer, span, store),
            Self::Change => change(buffer, span, store),
            Self::Yank => {
                store.fill(span.text(buffer), Filling::Yank);
                match span.linewise {
                    true => Position::new(span.start.line.min(from.line), from.column),
                    false => span.start.min(from),
                }
            }
            Self::Indent | Self::Outdent => {
                shift(buffer, span, self == Self::Indent, 1);
                Position::new(span.start.line, first_non_blank(buffer, span.start.line))
            }
            Self::Lowercase | Self::Uppercase | Self::ToggleCase => {
                let range = span.removal(buffer);
                let range = match span.linewise {
                    true => {
                        Position::new(span.start.line, 0)
                            ..Position::new(span.end.line, buffer.line_len(span.end.line))
                    }
                    false => range,
                };
                let converted = convert(&buffer.text_in(range.clone()), self);
                buffer.grouped(|buffer| buffer.replace(range.clone(), &converted));
                range.start
            }
        }
    }
}

/// Takes `span` out into a register, answering where the cursor lands.
pub(crate) fn delete(buffer: &mut Buffer, span: Span, store: &mut Store) -> Position {
    let text = span.text(buffer);
    store.fill(text, Filling::Delete);
    let range = span.removal(buffer);
    buffer.grouped(|buffer| buffer.replace(range.clone(), ""));
    match span.linewise {
        true => {
            let line = span.start.line.min(buffer.line_count().saturating_sub(1));
            Position::new(line, first_non_blank(buffer, line))
        }
        false => range.start,
    }
}

/// Takes `span` out into a register, leaving the place typing begins.
///
/// Changing whole lines keeps the first one's indentation, so that what is
/// typed in their place starts where they did.
fn change(buffer: &mut Buffer, span: Span, store: &mut Store) -> Position {
    if !span.linewise {
        store.fill(span.text(buffer), Filling::Delete);
        buffer.grouped(|buffer| buffer.replace(span.start..span.end, ""));
        return span.start;
    }
    let (first, last) = span.line_range();
    store.fill(span.text(buffer), Filling::Delete);
    let indent = buffer
        .line_chars(first)
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect::<String>();
    let range = Position::new(first, 0)..Position::new(last, buffer.line_len(last));
    buffer.grouped(|buffer| buffer.replace(range, &indent));
    Position::new(first, indent.chars().count())
}

/// Indents the lines `span` touches `times` steps, or outdents them.
///
/// An empty line is left empty: indenting it would only leave blanks at
/// the end of a line.
pub(crate) fn shift(buffer: &mut Buffer, span: Span, indent: bool, times: usize) {
    let (first, last) = span.line_range();
    buffer.grouped(|buffer| {
        for _ in 0..times {
            for line in first..=last {
                if buffer.line_len(line) == 0 {
                    continue;
                }
                buffer.set_selection(Selection::at(Position::new(line, 0)));
                match indent {
                    true => buffer.indent_lines(),
                    false => buffer.outdent_lines(),
                }
            }
        }
    });
}

/// `text` with its case changed as `operator` says.
pub(crate) fn convert(text: &str, operator: Operator) -> String {
    match operator {
        Operator::Lowercase => text.to_lowercase(),
        Operator::Uppercase => text.to_uppercase(),
        _ => text
            .chars()
            .flat_map(|ch| match ch.is_uppercase() {
                true => ch.to_lowercase().collect::<Vec<_>>(),
                false => ch.to_uppercase().collect::<Vec<_>>(),
            })
            .collect(),
    }
}
