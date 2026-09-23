//! Operators: what `d`, `c`, `y` and the rest do to the span they are given.
//!
//! A motion or an object decides the span; the operator decides what happens
//! to it. Every operator is one undo step, however many replacements it
//! takes, because it is one thing the reader asked for.

use pm_text::{Buffer, Position, Selection};

use crate::format;
use crate::motion::{Kind, Moved};
use crate::register::{Clipboard, Filling, Registers};
use crate::text::{self, first_non_blank};

/// What an operator does to its span.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
    /// `=`: indent the lines the span touches the way the lines above are.
    AutoIndent,
    /// `gu`: lower the case of the span.
    Lowercase,
    /// `gU`: raise the case of the span.
    Uppercase,
    /// `g~`: swap the case of the span.
    ToggleCase,
    /// `g?`: move every letter of the span thirteen places on.
    Rot13,
    /// `gq`: wrap the lines the span touches, leaving the cursor after them.
    Rewrap,
    /// `gw`: wrap the lines the span touches, leaving the cursor where it was.
    RewrapKeep,
    /// `gc`: comment the lines the span touches, or take their comments off.
    ToggleComment,
    /// `ys`: put a pair of delimiters around the span.
    AddSurround,
    /// `gR`: put a register's text in place of the span.
    ReplaceWithRegister,
    /// `cx`: swap the span with the one marked by the `cx` before.
    Exchange,
}

impl Operator {
    /// Every operator, for reading one back from what it is called.
    pub(crate) const ALL: [Self; 16] = [
        Self::Delete,
        Self::Change,
        Self::Yank,
        Self::Indent,
        Self::Outdent,
        Self::AutoIndent,
        Self::Lowercase,
        Self::Uppercase,
        Self::ToggleCase,
        Self::Rot13,
        Self::Rewrap,
        Self::RewrapKeep,
        Self::ToggleComment,
        Self::AddSurround,
        Self::ReplaceWithRegister,
        Self::Exchange,
    ];

    /// What the keymap's `op` clause calls the operator: the keys it is
    /// typed with, as Zed's `vim_operator` names them.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Delete => "d",
            Self::Change => "c",
            Self::Yank => "y",
            Self::Indent => ">",
            Self::Outdent => "<",
            Self::AutoIndent => "eq",
            Self::Lowercase => "gu",
            Self::Uppercase => "gU",
            Self::ToggleCase => "g~",
            Self::Rot13 => "g?",
            Self::Rewrap => "gq",
            Self::RewrapKeep => "gw",
            Self::ToggleComment => "gc",
            Self::AddSurround => "ys",
            Self::ReplaceWithRegister => "gR",
            Self::Exchange => "cx",
        }
    }

    /// The name bindings give the action that begins the operator.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Delete => "PushDelete",
            Self::Change => "PushChange",
            Self::Yank => "PushYank",
            Self::Indent => "PushIndent",
            Self::Outdent => "PushOutdent",
            Self::AutoIndent => "PushAutoIndent",
            Self::Lowercase => "PushLowercase",
            Self::Uppercase => "PushUppercase",
            Self::ToggleCase => "PushOppositeCase",
            Self::Rot13 => "PushRot13",
            Self::Rewrap => "PushRewrap",
            Self::RewrapKeep => "PushRewrapKeep",
            Self::ToggleComment => "PushToggleComments",
            Self::AddSurround => "PushAddSurrounds",
            Self::ReplaceWithRegister => "PushReplaceWithRegister",
            Self::Exchange => "PushExchange",
        }
    }

    /// Whether the operator changes the text, and so is repeated by `.`.
    pub(crate) fn changes(self) -> bool {
        self != Self::Yank
    }

    /// Whether the operator acts on whole lines whatever span it is given.
    pub(crate) fn is_linewise(self) -> bool {
        matches!(
            self,
            Self::Indent
                | Self::Outdent
                | Self::AutoIndent
                | Self::Rewrap
                | Self::RewrapKeep
                | Self::ToggleComment
        )
    }
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

    /// The span as the selection standing for it, for doing one thing at
    /// every span at once.
    pub(crate) fn as_selection(&self) -> Selection {
        Selection {
            anchor: self.start,
            head: self.end,
        }
    }

    /// The span a selection made by [`Self::as_selection`] stands for.
    pub(crate) fn from_selection(selection: Selection, linewise: bool) -> Self {
        match linewise {
            true => Self::lines(selection.start().line, selection.end().line),
            false => Self::chars(selection.start(), selection.end()),
        }
    }

    /// The first and last line the span touches.
    pub(crate) fn line_range(&self) -> (usize, usize) {
        (self.start.line, self.end.line)
    }

    /// The text the span covers, a line break after every line when linewise.
    pub(crate) fn text(&self, buffer: &Buffer) -> String {
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

    /// The text of the span's lines without their last line break, or the
    /// span itself: what a rewrite of the span replaces.
    pub(crate) fn body(&self, buffer: &Buffer) -> std::ops::Range<Position> {
        match self.linewise {
            true => {
                Position::new(self.start.line, 0)
                    ..Position::new(self.end.line, buffer.line_len(self.end.line))
            }
            false => self.start..self.end,
        }
    }

    /// The range taking the span out removes, its lines' breaks included.
    ///
    /// The last line has no break after it, so taking it out takes the
    /// break before it instead.
    pub(crate) fn removal(&self, buffer: &Buffer) -> std::ops::Range<Position> {
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

/// Where registers are read and filled while an operator runs at every
/// cursor, gathering one piece of text per cursor.
pub(crate) struct Store<'a> {
    /// Every register.
    pub registers: &'a mut Registers,
    /// The register named for this command, if one was.
    pub register: Option<char>,
    /// The system clipboard.
    pub clipboard: &'a mut dyn Clipboard,
    /// The text each cursor gave, in the order the cursors were done.
    pub pieces: Vec<String>,
    /// Why the text is being kept.
    pub filling: Option<Filling>,
    /// What a register put back in place of a span, for `gR`.
    pub source: Option<String>,
    /// How wide `gq` wraps.
    pub wrap: usize,
    /// Whether the spans are the lines of a block.
    pub block: bool,
}

impl Store<'_> {
    /// Keeps `text` from one cursor, as `filling` says.
    pub(crate) fn fill(&mut self, text: String, filling: Filling) {
        self.pieces.push(text);
        self.filling = Some(filling);
    }

    /// Puts what the cursors gave into the registers, in the order the
    /// cursors appear in the text.
    pub(crate) fn finish(mut self) {
        let Some(filling) = self.filling else {
            return;
        };
        self.pieces.reverse();
        self.registers.fill(
            self.register,
            self.pieces,
            filling,
            self.block,
            &mut *self.clipboard,
        );
    }
}

impl Operator {
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
        let line_start =
            |buffer: &Buffer, line: usize| Position::new(line, first_non_blank(buffer, line));
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
                line_start(buffer, span.start.line)
            }
            Self::AutoIndent => {
                let (first, last) = span.line_range();
                format::reindent(buffer, first, last);
                line_start(buffer, first)
            }
            Self::Rewrap | Self::RewrapKeep => {
                let (first, last) = span.line_range();
                let lines = buffer.line_count();
                format::rewrap(buffer, first, last, store.wrap);
                let last = (last + buffer.line_count()).saturating_sub(lines);
                match self == Self::Rewrap {
                    true => line_start(
                        buffer,
                        (last + 1).min(buffer.line_count().saturating_sub(1)),
                    ),
                    false => from,
                }
            }
            Self::ToggleComment => {
                let (first, last) = span.line_range();
                buffer.set_selection(Selection {
                    anchor: Position::new(first, 0),
                    head: Position::new(last, buffer.line_len(last).max(1)),
                });
                buffer.toggle_comment();
                line_start(buffer, first)
            }
            Self::Lowercase | Self::Uppercase | Self::ToggleCase | Self::Rot13 => {
                let range = span.body(buffer);
                let converted = convert(&buffer.text_in(range.clone()), self);
                buffer.grouped(|buffer| buffer.replace(range.clone(), &converted));
                range.start
            }
            Self::ReplaceWithRegister => {
                let Some(source) = store.source.clone() else {
                    return from;
                };
                let range = span.body(buffer);
                let source = match span.linewise {
                    true => source.strip_suffix('\n').unwrap_or(&source).to_owned(),
                    false => source.trim_end_matches('\n').to_owned(),
                };
                buffer.grouped(|buffer| buffer.replace(range.clone(), &source));
                range.start
            }
            Self::AddSurround | Self::Exchange => from,
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
        Operator::Rot13 => format::rot13(text),
        _ => text
            .chars()
            .flat_map(|ch| match ch.is_uppercase() {
                true => ch.to_lowercase().collect::<Vec<_>>(),
                false => ch.to_uppercase().collect::<Vec<_>>(),
            })
            .collect(),
    }
}
