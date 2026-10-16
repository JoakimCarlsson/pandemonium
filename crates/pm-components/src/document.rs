//! The text of a file open in the window: edited in place, line by line, told apart from what was last saved, parsed with its language's grammar and kept in step with its language server.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use pm_buffer::{Change, Lines, Span};
use pm_protocol::{Completion, Diagnostic, TextChange};
use pm_text::Field;
use pm_treesitter::{Editing, Kind, Syntax};

/// How many lines beyond the ones asked for are highlighted with them, so that scrolling a little needs no highlighting again.
const HIGHLIGHT_MARGIN: usize = 120;

/// The completions open at the caret.
#[derive(Clone, Debug, PartialEq)]
pub struct Completions {
    /// Where the word being completed starts, as a byte offset of the text.
    pub start: usize,
    /// What the language server offered, best first, once it answered.
    pub items: Vec<Completion>,
    /// The ones that match the word typed so far, as positions in `items`, best first.
    pub shown: Vec<usize>,
    /// The one picked, as a position in `shown`.
    pub picked: usize,
    /// Where the list hangs, below the word, in logical pixels of the window.
    pub at: (f32, f32),
    /// The version of the text they were asked for at.
    pub asked: i32,
    /// Whether the language server has yet to answer.
    pub loading: bool,
}

/// What is said about the place the pointer rests on or the caret is at.
#[derive(Clone, Debug, PartialEq)]
pub struct HoverCard {
    /// The place, as a byte offset of the text.
    pub offset: usize,
    /// Where the place's line is, in logical pixels of the window: the left edge of the place and the top of its line; the card hangs below the line, or above it when there is more room there.
    pub at: (f32, f32),
    /// What the language server said, in markdown, once it answered.
    pub text: Option<String>,
    /// What the language server finds wrong there.
    pub diagnostics: Vec<Diagnostic>,
    /// How far down the card is scrolled, in logical pixels.
    pub scroll: f32,
}

/// The highlights of a window of lines, kept until the text is parsed again.
#[derive(Clone, Debug, Default)]
struct Painted {
    /// The parse they came from.
    revision: u64,
    /// The lines they cover.
    lines: Range<usize>,
    /// For each of those lines, its highlighted stretches as offsets into the line.
    spans: Vec<Vec<(Range<usize>, Kind)>>,
}

/// The highlights worked out last, shared by the copies of a document so a frame does not work them out again.
#[derive(Clone, Default)]
struct HighlightCache(Arc<Mutex<Painted>>);

impl fmt::Debug for HighlightCache {
    /// Says nothing of what is held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HighlightCache")
    }
}

impl PartialEq for HighlightCache {
    /// What is held is worked out from the rest of the document, so it never tells two documents apart.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// A file's text, the caret and selection in it, whether it differs from what the folder holds, its syntax and what its language server says of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    /// The text, with the caret, the selection and what undo remembers.
    pub field: Field,
    /// Where each line starts.
    lines: Lines,
    /// A fingerprint of the text as it is now.
    current: u64,
    /// A fingerprint of the text as the folder holds it, or nothing once saving it failed.
    saved: Option<u64>,
    /// Whether the file ended its lines with a carriage return as well, which saving puts back.
    crlf: bool,
    /// Whether the file was longer than could be read, so that saving would cut it.
    pub truncated: bool,
    /// The installed language the file is written in, by folder name, when the server knows one.
    pub language: Option<String>,
    /// The text parsed with the language's grammar, once the grammar is loaded.
    pub syntax: Option<Syntax>,
    /// How the language's files are edited.
    pub editing: Editing,
    /// Whether the language server was told the file is open.
    pub synced: bool,
    /// The version of the text the language server was last told of.
    pub version: i32,
    /// The characters after which the language server offers completions.
    pub triggers: Vec<String>,
    /// Why the language server could not read the file, when it could not.
    pub server_problem: Option<String>,
    /// What the language server finds wrong with the file.
    pub diagnostics: Vec<Diagnostic>,
    /// The completions open at the caret, if any.
    pub completion: Option<Completions>,
    /// What is said about a place, if anything is showing.
    pub hover: Option<HoverCard>,
    /// The highlights worked out last.
    painted: HighlightCache,
}

/// A fingerprint of `text`.
fn fingerprint(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

impl Document {
    /// The document of a file that held `text`, which cannot be changed when `read_only`; a file read only in part is always read only.
    pub fn new(text: &str, read_only: bool, truncated: bool) -> Self {
        let crlf = text.contains("\r\n");
        let text = if crlf {
            text.replace("\r\n", "\n")
        } else {
            text.to_owned()
        };
        let lines = Lines::new(&text);
        let current = fingerprint(&text);
        Self {
            field: Field::new(text)
                .multiline(true)
                .read_only(read_only || truncated),
            lines,
            current,
            saved: Some(current),
            crlf,
            truncated,
            language: None,
            syntax: None,
            editing: Editing::default(),
            synced: false,
            version: 0,
            triggers: Vec::new(),
            server_problem: None,
            diagnostics: Vec::new(),
            completion: None,
            hover: None,
            painted: HighlightCache::default(),
        }
    }

    /// Notes what the text has become after an edit from `before`, whose lines `before_lines` indexed, parses it again and says how it changed, if it did.
    pub fn refresh(&mut self, before: &str, before_lines: &Lines) -> Option<Change> {
        let current = fingerprint(self.field.text());
        if current == self.current {
            return None;
        }
        self.current = current;
        self.lines = Lines::new(self.field.text());
        let change = Change::between(before, self.field.text())?;
        if let Some(syntax) = &mut self.syntax {
            syntax.edit(&change, before_lines, self.field.text(), &self.lines);
        }
        Some(change)
    }

    /// The change `change` of the text whose lines `before_lines` indexed, as the language server is told it.
    pub fn text_change(change: &Change, before: &str, before_lines: &Lines) -> TextChange {
        TextChange {
            span: Some(before_lines.span_of(change.range.clone(), before)),
            text: change.text.clone(),
        }
    }

    /// Whether the text differs from what the folder holds.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.current)
    }

    /// Notes that the folder now holds the text as it is.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current);
    }

    /// Notes that the folder may not hold the text, because writing it failed.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    /// The text as it is written to the folder, with the file's own line endings.
    pub fn to_save(&self) -> String {
        if self.crlf {
            self.field.text().replace('\n', "\r\n")
        } else {
            self.field.text().to_owned()
        }
    }

    /// The index of the lines.
    pub fn line_index(&self) -> &Lines {
        &self.lines
    }

    /// How many lines there are.
    pub fn lines(&self) -> usize {
        self.lines.count()
    }

    /// Where line `index` is in the text, without its line break.
    pub fn line(&self, index: usize) -> Range<usize> {
        self.lines.line(index, self.field.text())
    }

    /// The line the byte `offset` of the text is on.
    pub fn line_of(&self, offset: usize) -> usize {
        self.lines.line_of(offset)
    }

    /// The bytes of the text `span` covers.
    pub fn range_of(&self, span: Span) -> Range<usize> {
        self.lines.range_of(span, self.field.text())
    }

    /// The byte `offset` as a place a language server names.
    pub fn position_of(&self, offset: usize) -> pm_buffer::Position {
        self.lines.position_of(offset, self.field.text())
    }

    /// Forgets the highlights worked out last, as when the grammar of a language a stretch of the text is written in was loaded since.
    pub fn repaint(&mut self) {
        self.painted = HighlightCache::default();
    }

    /// Takes in `syntax`, the text parsed with the grammar of the document's language.
    pub fn with_syntax(&mut self, syntax: Syntax) {
        self.editing = syntax.grammar().editing().clone();
        self.syntax = Some(syntax);
    }

    /// The highlighted stretches of each line of `lines`, as offsets into the line, worked out again only when the text was parsed again or the lines fall outside the ones worked out last.
    pub fn highlights(&self, lines: Range<usize>) -> Vec<Vec<(Range<usize>, Kind)>> {
        let Some(syntax) = &self.syntax else {
            return vec![Vec::new(); lines.len()];
        };
        let Ok(mut painted) = self.painted.0.lock() else {
            return vec![Vec::new(); lines.len()];
        };
        let stale = painted.revision != syntax.revision()
            || lines.start < painted.lines.start
            || lines.end > painted.lines.end;
        if stale {
            let first = lines.start.saturating_sub(HIGHLIGHT_MARGIN);
            let last = (lines.end + HIGHLIGHT_MARGIN).min(self.lines());
            let text = self.field.text();
            let window =
                self.lines.start(first, text)..self.line(last.saturating_sub(1).max(first)).end;
            let mut spans = vec![Vec::new(); last.saturating_sub(first)];
            for highlight in syntax.highlights(text, window) {
                let mut row = self.line_of(highlight.range.start);
                while row < last && row >= first {
                    let line = self.line(row);
                    if line.start > highlight.range.end {
                        break;
                    }
                    let low = highlight.range.start.max(line.start);
                    let high = highlight.range.end.min(line.end);
                    if low < high {
                        spans[row - first]
                            .push((low - line.start..high - line.start, highlight.kind));
                    }
                    if highlight.range.end <= line.end + 1 {
                        break;
                    }
                    row += 1;
                }
            }
            *painted = Painted {
                revision: syntax.revision(),
                lines: first..last,
                spans,
            };
        }
        lines
            .map(|row| {
                row.checked_sub(painted.lines.start)
                    .and_then(|place| painted.spans.get(place))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }

    /// The diagnostics that touch line `row`, each with the stretch of the line it covers, as offsets into the line.
    pub fn diagnostics_on(&self, row: usize) -> Vec<(Range<usize>, &Diagnostic)> {
        let line = self.line(row);
        self.diagnostics
            .iter()
            .filter_map(|diagnostic| {
                let range = self.range_of(diagnostic.span);
                if range.end < line.start || range.start > line.end {
                    return None;
                }
                let low = range.start.max(line.start) - line.start;
                let high = range.end.min(line.end).saturating_sub(line.start);
                Some((low..high.max(low), diagnostic))
            })
            .collect()
    }

    /// The diagnostics whose stretch holds the byte `offset`.
    pub fn diagnostics_at(&self, offset: usize) -> Vec<Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| {
                let range = self.range_of(diagnostic.span);
                range.start <= offset && offset <= range.end.max(range.start + 1)
            })
            .cloned()
            .collect()
    }

    /// The word of the text that ends at the byte `offset`: where it starts.
    pub fn word_start(&self, offset: usize) -> usize {
        let text = self.field.text();
        text[..offset.min(text.len())]
            .char_indices()
            .rev()
            .take_while(|(_, character)| character.is_alphanumeric() || *character == '_')
            .last()
            .map_or(offset, |(at, _)| at)
    }

    /// The leading spaces and tabs of the line the byte `offset` is on.
    pub fn indentation_at(&self, offset: usize) -> String {
        let line = self.line(self.line_of(offset));
        self.field.text()[line]
            .chars()
            .take_while(|character| *character == ' ' || *character == '\t')
            .collect()
    }
}
