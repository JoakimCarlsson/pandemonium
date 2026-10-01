//! Syntax trees: parsing a buffer, and what its characters are.
//!
//! The tree is kept alongside the text and edited with it, so a keypress
//! reparses the part of the file that changed rather than the file. What
//! comes out is a highlight per character over the lines a pane can see —
//! which is what a screen needs and all a screen needs, however long the
//! file behind it is.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap, VecDeque};
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

/// Words in syntax node kinds that indicate a declaration.
const DECLARING: &[&str] = &[
    "function",
    "method",
    "class",
    "struct",
    "impl",
    "trait",
    "enum",
    "interface",
    "module",
    "mod_item",
    "namespace",
    "union",
    "object",
    "protocol",
];

/// Words in syntax node kinds that indicate a use rather than a declaration.
const USING: &[&str] = &["call", "invocation", "parameter", "argument", "identifier"];

/// Whether a syntax node declares a named symbol.
pub fn is_declaration(kind: &str) -> bool {
    DECLARING.iter().any(|word| kind.contains(word))
        && !USING.iter().any(|word| kind.contains(word))
}

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

/// The kinds of node that are one tag of a markup element: HTML's, with the
/// closing one a grammar gives up on matching once its name has been
/// changed, and JSX's.
const TAGS: [&str; 5] = [
    "start_tag",
    "end_tag",
    "erroneous_end_tag",
    "jsx_opening_element",
    "jsx_closing_element",
];

/// The brackets the grammars have tokens for.
const BRACKETS: [&str; 6] = ["(", ")", "[", "]", "{", "}"];

/// How many pairs of brackets each bracket token within `bytes` is inside.
///
/// A bracket of a string or a comment is part of that node's text and not a
/// token of its own, so only the brackets that are code are found. A node
/// that has an opening bracket among its children is a pair, and its
/// brackets and everything inside are one level in from what holds it.
fn bracket_depths(tree: &Tree, text: &Rope, bytes: Range<usize>) -> HashMap<(usize, usize), usize> {
    let mut found = HashMap::new();
    let mut cursor = tree.walk();
    let mut stack: Vec<(usize, bool)> = Vec::new();
    loop {
        let node = cursor.node();
        if node.start_byte() < bytes.end && node.end_byte() > bytes.start {
            if node.child_count() > 0 {
                let pair = (0..node.child_count()).any(|index| {
                    node.child(index).is_some_and(|child| {
                        !child.is_named() && ["(", "[", "{"].contains(&child.kind())
                    })
                });
                let outer = stack
                    .last()
                    .map_or(0, |(outer, pair)| outer + usize::from(*pair));
                stack.push((outer, pair));
                if cursor.goto_first_child() {
                    continue;
                }
                stack.pop();
            } else if !node.is_named() && BRACKETS.contains(&node.kind()) {
                let point = node.start_position();
                let column = text.line(point.row).byte_to_char(point.column);
                let depth = stack.last().map_or(0, |(outer, _)| *outer);
                found.insert((point.row, column), depth);
            }
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return found;
            }
            stack.pop();
        }
    }
}

/// The highlight query of every language compiled so far, by name.
///
/// A query takes longer to compile than most files take to parse, so each
/// language's is compiled once and shared by every buffer in it. A language
/// whose query does not compile is remembered as such.
static QUERIES: LazyLock<Mutex<Queries>> = LazyLock::new(Mutex::default);

/// Remembers a failed extension query by language name.
pub(crate) fn remember_failed_query(name: &'static str) {
    if let Ok(mut queries) = QUERIES.lock() {
        queries.insert(name, None);
    }
}

/// Each language's compiled highlight query, or `None` for one that failed.
type Queries = HashMap<&'static str, Option<Arc<Query>>>;

/// The highlights [`highlight`] worked out last, the most recent at the back.
static HIGHLIGHTED: LazyLock<Mutex<VecDeque<Highlighted>>> = LazyLock::new(Mutex::default);

/// Drops cached queries and excerpts for reloaded extension languages.
pub(crate) fn forget_languages(names: &[&str]) {
    if let Ok(mut queries) = QUERIES.lock() {
        queries.retain(|name, _| !names.contains(name));
    }
    if let Ok(mut highlighted) = HIGHLIGHTED.lock() {
        highlighted.retain(|entry| !names.contains(&entry.language));
    }
}

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
    /// The lines and columns of the characters a server said name something
    /// that can be assigned to again.
    mutable: BTreeSet<(usize, usize)>,
    /// How many pairs of brackets each bracket is inside, by line and column.
    depths: HashMap<(usize, usize), usize>,
}

impl Highlights {
    /// Writes `highlight` over the characters `span` covers.
    ///
    /// What a language server says about a span is written over what the
    /// grammar guessed, because the server knows which of two things a name
    /// is and the grammar only knows that it is a name.
    pub fn repaint(&mut self, span: Range<Position>, highlight: Highlight, mutable: bool) {
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
            if mutable {
                self.mutable
                    .extend((from..to.min(row.len())).map(|column| (line, column)));
            }
        }
    }

    /// How many pairs of brackets the bracket at `line` and `column` is
    /// inside, when the character is a bracket the grammar has a token for.
    pub fn bracket_depth(&self, line: usize, column: usize) -> Option<usize> {
        self.depths.get(&(line, column)).copied()
    }

    /// Whether the character at `line` and `column` is part of a name a
    /// server said can be assigned to again.
    pub fn is_mutable(&self, line: usize, column: usize) -> bool {
        self.mutable.contains(&(line, column))
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
        if language.is_wasm() {
            crate::grammar::prepare(&mut parser).ok()?;
        }
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

    /// The name of the tag whose name holds `byte`, and the name of the tag
    /// that closes or opens the same element, as spans of bytes.
    ///
    /// Only markup has such pairs: an HTML element's start and end tags and
    /// a JSX element's opening and closing ones. Anything else, and an
    /// element missing either of its tags, has none.
    pub fn tag_names(&self, byte: usize) -> Option<(Range<usize>, Range<usize>)> {
        let root = self.tree.as_ref()?.root_node();
        let found = [byte, byte.saturating_sub(1)]
            .into_iter()
            .filter_map(|probe| root.descendant_for_byte_range(probe, probe))
            .find_map(|node| {
                let mut node = node;
                loop {
                    if TAGS.contains(&node.kind()) {
                        return Some(node);
                    }
                    node = node.parent()?;
                }
            })?;
        let element = found.parent()?;
        let mut cursor = element.walk();
        let tags = element
            .children(&mut cursor)
            .filter(|child| TAGS.contains(&child.kind()))
            .collect::<Vec<_>>();
        let [first, second] = tags[..] else {
            return None;
        };
        let name_of = |tag: tree_sitter::Node<'_>| {
            let name = match tag.kind() {
                "start_tag" | "end_tag" | "erroneous_end_tag" => tag
                    .children(&mut tag.walk())
                    .find(|child| child.kind().ends_with("tag_name")),
                _ => tag.child_by_field_name("name"),
            }?;
            Some(name.byte_range())
        };
        let (here, there) = match found.id() == first.id() {
            true => (name_of(first)?, name_of(second)?),
            false => (name_of(second)?, name_of(first)?),
        };
        (here.start <= byte && byte <= here.end).then_some((here, there))
    }

    /// The name of the tag that opens at the `>` just before `byte`, as a
    /// span of bytes, when one does and it is not closed on its own.
    ///
    /// A fragment has no name and comes to an empty span at `byte`.
    pub fn open_tag_name_before(&self, byte: usize) -> Option<Range<usize>> {
        let root = self.tree.as_ref()?.root_node();
        let at = byte.checked_sub(1)?;
        let closer = root.descendant_for_byte_range(at, at)?;
        let tag = closer.parent()?;
        if closer.kind() != ">" || tag.end_byte() != byte {
            return None;
        }
        match tag.kind() {
            "start_tag" => {
                let mut cursor = tag.walk();
                let name = tag
                    .children(&mut cursor)
                    .find(|child| child.kind() == "tag_name")?;
                Some(name.byte_range())
            }
            "jsx_opening_element" => Some(
                tag.child_by_field_name("name")
                    .map_or(byte..byte, |name| name.byte_range()),
            ),
            _ => None,
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
            mutable: BTreeSet::new(),
            depths: bracket_depths(
                tree,
                text,
                text.line_to_byte(lines.start)..line_end_byte(text, last - 1),
            ),
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
