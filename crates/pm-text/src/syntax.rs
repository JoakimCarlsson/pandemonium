//! Syntax trees: parsing a buffer, and what its characters are.
//!
//! The tree is kept alongside the text and edited with it, so a keypress
//! reparses the part of the file that changed rather than the file. What
//! comes out is a highlight per character over the lines a pane can see —
//! which is what a screen needs and all a screen needs, however long the
//! file behind it is.

use std::ops::Range;

use ropey::Rope;
use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Node, Parser, Query, QueryCursor, TextProvider, Tree};

use crate::cursor::Position;
use crate::language::Language;

/// What a character is, as far as colour is concerned.
///
/// These are the distinctions a theme draws, not the ones a grammar draws: a
/// query captures three dozen kinds of thing, and this is what they come to
/// once they reach a palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Highlight {
    /// A keyword.
    Keyword,
    /// A string, a character literal or an escape in one.
    String,
    /// A function, a method or a macro being named.
    Function,
    /// A comment, documentation included.
    Comment,
    /// A number or a boolean.
    Number,
    /// A type or a trait.
    Type,
    /// A bracket, a delimiter or another piece of punctuation.
    Punctuation,
    /// A variable, a parameter or a plain identifier.
    Variable,
    /// A field, a member or a key.
    Property,
    /// A named constant.
    Constant,
    /// An operator.
    Operator,
    /// A markup element, such as an HTML or JSX tag.
    Tag,
    /// A markup attribute, or an annotation on a declaration.
    Attribute,
}

impl Highlight {
    /// What a query's capture name comes to, of the ones a theme draws.
    fn of(capture: &str) -> Option<Self> {
        let kind = capture.split('.').next().unwrap_or(capture);
        match (kind, capture) {
            (_, "text.literal") => Some(Self::String),
            (_, "text.title") => Some(Self::Keyword),
            (_, "variable.member") => Some(Self::Property),
            ("comment", _) => Some(Self::Comment),
            ("string", _) | ("escape", _) | ("character", _) => Some(Self::String),
            (_, "constant.builtin") => Some(Self::Number),
            ("number", _) | ("boolean", _) => Some(Self::Number),
            ("constant", _) => Some(Self::Constant),
            ("function", _) | ("constructor", _) => Some(Self::Function),
            ("keyword", _) | ("label", _) => Some(Self::Keyword),
            ("operator", _) => Some(Self::Operator),
            ("attribute", _) | ("annotation", _) => Some(Self::Attribute),
            ("tag", _) => Some(Self::Tag),
            ("property", _) | ("field", _) => Some(Self::Property),
            ("type", _) | ("module", _) | ("namespace", _) => Some(Self::Type),
            ("punctuation", _) => Some(Self::Punctuation),
            ("variable", _) | ("parameter", _) => Some(Self::Variable),
            _ => None,
        }
    }

    /// What one of the protocol's semantic token types comes to.
    ///
    /// A server names what it found in the language's own words; these are
    /// the same distinctions a theme draws, so a token a theme has no colour
    /// for leaves the grammar's own answer standing.
    pub fn of_token(kind: &str) -> Option<Self> {
        match kind {
            "namespace" | "type" | "class" | "enum" | "interface" | "struct" | "typeParameter" => {
                Some(Self::Type)
            }
            "parameter" | "variable" => Some(Self::Variable),
            "property" | "event" => Some(Self::Property),
            "enumMember" => Some(Self::Constant),
            "function" | "method" | "macro" => Some(Self::Function),
            "keyword" | "modifier" => Some(Self::Keyword),
            "comment" => Some(Self::Comment),
            "string" | "regexp" => Some(Self::String),
            "number" => Some(Self::Number),
            "operator" => Some(Self::Operator),
            "decorator" => Some(Self::Attribute),
            _ => None,
        }
    }
}

/// The highlights of a range of lines, one entry per character.
///
/// A line is as long as its text and no longer: a column past the end of a
/// line has no highlight, the same as a column whose grammar says nothing
/// about it.
#[derive(Debug, Default)]
pub struct Highlights {
    /// The first line this covers.
    first: usize,
    /// One row per line covered, one entry per character of it.
    rows: Vec<Vec<Option<Highlight>>>,
}

impl Highlights {
    /// Writes `highlight` over the characters `span` covers.
    ///
    /// What a language server says about a span is written over what the
    /// grammar guessed, because the server knows which of two things a name
    /// is and the grammar only knows that it is a name.
    pub fn repaint(&mut self, span: Range<Position>, highlight: Highlight) {
        for line in span.start.line..=span.end.line {
            let Some(index) = line.checked_sub(self.first) else {
                continue;
            };
            let Some(row) = self.rows.get_mut(index) else {
                break;
            };
            let from = if line == span.start.line {
                span.start.column
            } else {
                0
            };
            let to = if line == span.end.line {
                span.end.column
            } else {
                row.len()
            };
            for slot in row.iter_mut().take(to).skip(from) {
                *slot = Some(highlight);
            }
        }
    }

    /// The highlight of the character at `line` and `column`, if it has one.
    pub fn at(&self, line: usize, column: usize) -> Option<Highlight> {
        *self
            .rows
            .get(line.checked_sub(self.first)?)?
            .get(column)
            .unwrap_or(&None)
    }
}

/// A parsed buffer: the grammar, the query and the tree as it stands.
pub struct Syntax {
    /// The parser the tree is produced by.
    parser: Parser,
    /// The query the highlights are captured by.
    query: Query,
    /// The tree as of the last parse.
    tree: Option<Tree>,
}

impl Syntax {
    /// The syntax of a buffer in `language`, before it has been parsed.
    ///
    /// A grammar the build shipped is expected to load and its own query to
    /// compile; a language whose either fails is treated as a language the
    /// editor does not know, so the file still opens.
    pub fn new(language: Language) -> Option<Self> {
        let grammar = language.grammar();
        let mut parser = Parser::new();
        parser.set_language(&grammar).ok()?;
        let query = Query::new(&grammar, &language.highlights()).ok()?;

        Some(Self {
            parser,
            query,
            tree: None,
        })
    }

    /// Parses `text`, reusing what the last tree still has right.
    pub fn parse(&mut self, text: &Rope) {
        let tree = self.parser.parse_with_options(
            &mut |byte, _| chunk_at(text, byte),
            self.tree.as_ref(),
            None,
        );
        if tree.is_some() {
            self.tree = tree;
        }
    }

    /// Tells the tree where the text was changed, before the next parse.
    pub fn edit(&mut self, edit: &InputEdit) {
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(edit);
        }
    }

    /// The highlights of `lines`, as one entry per character.
    pub fn highlights(&mut self, text: &Rope, lines: Range<usize>) -> Highlights {
        let Some(tree) = self.tree.as_ref() else {
            return Highlights::default();
        };
        let last = lines.end.min(text.len_lines());
        if lines.start >= last {
            return Highlights::default();
        }

        let mut highlights = Highlights {
            first: lines.start,
            rows: (lines.start..last)
                .map(|line| vec![None; text.line(line).len_chars()])
                .collect(),
        };

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(text.line_to_byte(lines.start)..line_end_byte(text, last - 1));
        let mut captures = cursor.captures(&self.query, tree.root_node(), RopeText(text));
        let mut spans = Vec::new();
        while let Some((matched, index)) = captures.next() {
            let capture = matched.captures()[*index];
            let name = self.query.capture_names()[capture.index as usize];
            if let Some(highlight) = Highlight::of(name) {
                spans.push((capture.node.byte_range(), highlight));
            }
        }

        spans.sort_by_key(|(range, _)| std::cmp::Reverse(range.end - range.start));
        for (range, highlight) in spans {
            paint(&mut highlights, text, range, highlight);
        }
        highlights
    }
}

/// Writes `highlight` over the characters `range` covers.
fn paint(highlights: &mut Highlights, text: &Rope, range: Range<usize>, highlight: Highlight) {
    let len = text.len_bytes();
    let (start, end) = (range.start.min(len), range.end.min(len));
    let (first, last) = (text.byte_to_line(start), text.byte_to_line(end));

    for line in first..=last {
        let Some(index) = line.checked_sub(highlights.first) else {
            continue;
        };
        let Some(row) = highlights.rows.get_mut(index) else {
            break;
        };
        let opens = text.line_to_byte(line);
        let from = text.byte_to_char(start.max(opens)) - text.line_to_char(line);
        let to = text.byte_to_char(end.min(opens + text.line(line).len_bytes()))
            - text.line_to_char(line);
        for slot in row.iter_mut().take(to).skip(from) {
            *slot = Some(highlight);
        }
    }
}

/// The byte one past the end of `line`.
fn line_end_byte(text: &Rope, line: usize) -> usize {
    text.line_to_byte(line) + text.line(line).len_bytes()
}

/// The text from `byte` to the end of the chunk it falls in.
fn chunk_at(text: &Rope, byte: usize) -> &[u8] {
    if byte >= text.len_bytes() {
        return &[];
    }
    let (chunk, start, _, _) = text.chunk_at_byte(byte);
    &chunk.as_bytes()[byte - start..]
}

/// The rope, as the bytes a query reads node text out of.
struct RopeText<'a>(&'a Rope);

impl<'a> TextProvider<&'a [u8]> for RopeText<'a> {
    type I = std::iter::Map<ropey::iter::Chunks<'a>, fn(&'a str) -> &'a [u8]>;

    /// The chunks of rope `node` spans, as bytes.
    fn text(&mut self, node: Node<'_>) -> Self::I {
        self.0
            .byte_slice(node.byte_range())
            .chunks()
            .map(str::as_bytes as fn(&'a str) -> &'a [u8])
    }
}
