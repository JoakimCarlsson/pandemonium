//! Syntax trees: parsing a buffer, and what its characters are.
//!
//! The tree is kept alongside the text and edited with it, so a keypress
//! reparses the part of the file that changed rather than the file. What
//! comes out is a highlight per character over the lines a pane can see —
//! which is what a screen needs and all a screen needs, however long the
//! file behind it is.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::ops::{ControlFlow, Range};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use ropey::Rope;
use streaming_iterator::StreamingIterator;
use tree_sitter::{
    InputEdit, Node, ParseOptions, ParseState, Parser, Query, QueryCursor, TextProvider, Tree,
};

use crate::cursor::Position;
use crate::language::Language;

/// How long one parse may run before it is given up on.
///
/// An edit reparses in well under a millisecond and even a large file
/// parses from nothing in a fraction of this; a parse that runs past it is
/// a grammar lost in a pathological file, and the frame waiting on it is
/// worth more than its colours.
const PARSE_BUDGET: Duration = Duration::from_millis(250);

/// How many pieces of text [`highlight`] remembers the highlights of.
///
/// Enough for every code block a rendered document and a hover show at
/// once, so a frame drawing them again highlights none of them afresh.
const REMEMBERED: usize = 64;

/// The highlight query of every language compiled so far, by name.
///
/// A query takes longer to compile than most files take to parse, so each
/// language's is compiled once and shared by every buffer in it. A language
/// whose query does not compile is remembered as such.
static QUERIES: LazyLock<Mutex<Queries>> = LazyLock::new(Mutex::default);

/// Each language's compiled highlight query, or `None` for one that failed.
type Queries = HashMap<&'static str, Option<Arc<Query>>>;

/// The highlights [`highlight`] worked out last, the most recent at the back.
static HIGHLIGHTED: LazyLock<Mutex<VecDeque<Highlighted>>> = LazyLock::new(Mutex::default);

/// One piece of text [`highlight`] has already highlighted.
struct Highlighted {
    /// The language it was read as.
    language: &'static str,
    /// A hash of the text, to pass over most entries without comparing it.
    hash: u64,
    /// The text itself.
    text: String,
    /// What it came to.
    highlights: Arc<Highlights>,
}

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

/// The highlights of `text` read as `language`, for text that is no file's.
///
/// A signature a server wrote into a hover is code without a buffer, and it
/// is coloured the way the same code is coloured in the file beside it.
/// Such text is drawn again every frame and changes rarely, so the last
/// [`REMEMBERED`] answers are kept and the same text is highlighted once.
pub fn highlight(language: Language, text: &str) -> Arc<Highlights> {
    let hash = hash_of(text);
    if let Some(found) = remembered(language.name(), hash, text) {
        return found;
    }
    let highlights = Arc::new(highlight_afresh(language, text));
    remember(Highlighted {
        language: language.name(),
        hash,
        text: text.to_owned(),
        highlights: highlights.clone(),
    });
    highlights
}

/// The highlights of `text` read as `language`, parsed and captured anew.
fn highlight_afresh(language: Language, text: &str) -> Highlights {
    let Some(mut syntax) = Syntax::new(language) else {
        return Highlights::default();
    };
    let rope = Rope::from_str(text);
    syntax.parse(&rope);
    syntax.highlights(&rope, 0..rope.len_lines())
}

/// The hash `text` is remembered under.
fn hash_of(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// What `text` in `language` was last highlighted as, moved to the back as
/// the most recently wanted.
fn remembered(language: &str, hash: u64, text: &str) -> Option<Arc<Highlights>> {
    let mut highlighted = HIGHLIGHTED.lock().ok()?;
    let index = highlighted
        .iter()
        .position(|entry| entry.hash == hash && entry.language == language && entry.text == text)?;
    let entry = highlighted.remove(index)?;
    let found = entry.highlights.clone();
    highlighted.push_back(entry);
    Some(found)
}

/// Keeps `entry`, forgetting the least recently wanted once there are more
/// than [`REMEMBERED`].
fn remember(entry: Highlighted) {
    let Ok(mut highlighted) = HIGHLIGHTED.lock() else {
        return;
    };
    highlighted.push_back(entry);
    while highlighted.len() > REMEMBERED {
        highlighted.pop_front();
    }
}

/// The highlight query of `language`, compiled the first time it is asked
/// for.
fn query(language: Language) -> Option<Arc<Query>> {
    let mut queries = QUERIES.lock().ok()?;
    queries
        .entry(language.name())
        .or_insert_with(|| {
            Query::new(&language.grammar(), &language.highlights())
                .ok()
                .map(Arc::new)
        })
        .clone()
}

/// A parsed buffer: the grammar, the query and the tree as it stands.
pub struct Syntax {
    /// The parser the tree is produced by.
    parser: Parser,
    /// The query the highlights are captured by, shared by its language.
    query: Arc<Query>,
    /// The tree as of the last parse.
    tree: Option<Tree>,
    /// Whether a parse has run past [`PARSE_BUDGET`], after which the
    /// buffer is not parsed again and is drawn without highlights.
    given_up: bool,
}

impl Syntax {
    /// The syntax of a buffer in `language`, before it has been parsed.
    ///
    /// A grammar the build shipped is expected to load and its own query to
    /// compile; a language whose either fails is treated as a language the
    /// editor does not know, so the file still opens.
    pub fn new(language: Language) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(&language.grammar()).ok()?;
        let query = query(language)?;

        Some(Self {
            parser,
            query,
            tree: None,
            given_up: false,
        })
    }

    /// Parses `text`, reusing what the last tree still has right.
    ///
    /// A parse that runs past [`PARSE_BUDGET`] is cut short, and the buffer
    /// keeps no tree from then on: a file that hangs its grammar is shown
    /// as plain text rather than holding up every keystroke after.
    pub fn parse(&mut self, text: &Rope) {
        if self.given_up {
            return;
        }
        let started = Instant::now();
        let mut within_budget = |_: &ParseState| match started.elapsed() < PARSE_BUDGET {
            true => ControlFlow::Continue(()),
            false => ControlFlow::Break(()),
        };
        let tree = self.parser.parse_with_options(
            &mut |byte, _| chunk_at(text, byte),
            self.tree.as_ref(),
            Some(ParseOptions::new().progress_callback(&mut within_budget)),
        );
        match tree {
            Some(tree) => self.tree = Some(tree),
            None => {
                self.parser.reset();
                self.tree = None;
                self.given_up = true;
            }
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

    /// The nodes whose kind `keep` accepts that hold the character at
    /// `byte`, outermost first.
    ///
    /// Only the one path from the root down to that character is walked, so
    /// asking this of a large file costs its depth rather than its size.
    pub fn around(&self, text: &Rope, byte: usize, keep: &dyn Fn(&str) -> bool) -> Vec<SyntaxNode> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        let mut node = tree.root_node().descendant_for_byte_range(byte, byte);
        while let Some(held) = node {
            if held.is_named() && keep(held.kind()) {
                found.push(SyntaxNode::of(held, text));
            }
            node = held.parent();
        }
        found.reverse();
        found
    }

    /// Every node of the tree whose kind `keep` accepts, outermost first.
    pub fn nodes(&self, text: &Rope, keep: &dyn Fn(&str) -> bool) -> Vec<SyntaxNode> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut found = Vec::new();
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            if node.is_named() && keep(node.kind()) {
                found.push(SyntaxNode::of(node, text));
            }
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return found;
                }
            }
        }
    }
}

/// One node of a syntax tree, as the text it spans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    /// What the grammar calls it: `function_item`, `class_declaration`.
    pub kind: String,
    /// The text it spans.
    pub range: Range<Position>,
    /// The text its `body` spans, when the grammar gives it one.
    pub body: Option<Range<Position>>,
    /// What it is called, when the grammar gives it a `name`, or the `type`
    /// an implementation is of.
    pub name: Option<String>,
}

impl SyntaxNode {
    /// `node` as the text of `text` it spans.
    fn of(node: Node<'_>, text: &Rope) -> Self {
        let span =
            |node: Node<'_>| position(text, node.start_byte())..position(text, node.end_byte());
        Self {
            kind: node.kind().to_owned(),
            range: span(node),
            body: node.child_by_field_name("body").map(span),
            name: node
                .child_by_field_name("name")
                .or_else(|| node.child_by_field_name("type"))
                .map(|name| {
                    text.byte_slice(name.start_byte()..name.end_byte())
                        .to_string()
                }),
        }
    }
}

/// The position the byte `byte` of `text` falls at.
fn position(text: &Rope, byte: usize) -> Position {
    let offset = text.byte_to_char(byte.min(text.len_bytes()));
    let line = text.char_to_line(offset);
    Position::new(line, offset - text.line_to_char(line))
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
