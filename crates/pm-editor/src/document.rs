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
    /// The item the language server was last asked to resolve, as it wrote it, so that it is asked once.
    pub resolving: Option<String>,
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
    /// How the language's files are edited, with the reader's own indentation put over the language's.
    pub editing: Editing,
    /// How the language's own files are edited.
    language_editing: Editing,
    /// The columns a level of indentation is and whether it is a tab, when the reader chose them over the language's.
    indentation: (Option<usize>, Option<bool>),
    /// Whether the language server was told the file is open.
    pub synced: bool,
    /// The version of the text the language server was last told of.
    pub version: i32,
    /// The characters after which the language server offers completions.
    pub triggers: Vec<String>,
    /// The characters after which the language server tells what the call the caret is in looks like.
    pub signature_triggers: Vec<String>,
    /// The characters after which the language server formats what was just typed.
    pub format_triggers: Vec<String>,
    /// Whether the language server formats a stretch of the file.
    pub formats_ranges: bool,
    /// Whether the language server says which blocks of the file can be folded.
    pub folds_supported: bool,
    /// Whether the language server says which stretches of the file are edited together.
    pub links_supported: bool,
    /// Whether the language server says what each name of the file is.
    pub semantic_tokens_supported: bool,
    /// Whether the language server says what to show inside the lines of the file.
    pub inlay_hints_supported: bool,
    /// Whether the language server says what to show above the lines of the file.
    pub code_lens_supported: bool,
    /// Whether the language server says which stretches of the file lead somewhere when pressed.
    pub document_links_supported: bool,
    /// Whether the language server says which stretches of the file spell a colour.
    pub colors_supported: bool,
    /// Whether the language server says which names of the file to show the values of while a program is paused.
    pub inline_values_supported: bool,
    /// Why the language server could not read the file, when it could not.
    pub server_problem: Option<String>,
    /// What the language server finds wrong with the file.
    pub diagnostics: Vec<Diagnostic>,
    /// The completions open at the caret, if any.
    pub completion: Option<Completions>,
    /// What is said about a place, if anything is showing.
    pub hover: Option<HoverCard>,
    /// The word the pointer is on while Ctrl is held, which pressing takes to where it is defined.
    pub reach: Option<Range<usize>>,
    /// Each time the selection grew along the syntax tree, what it was and what it grew to, the latest last.
    pub grown: Vec<(Range<usize>, Range<usize>)>,
    /// The stretches where the thing at the caret is used, as the language server said for the version of the text they are about.
    pub occurrences: (i32, Vec<Range<usize>>),
    /// The signature of the call the caret is in, while it is showing.
    pub signature: Option<pm_protocol::Signature>,
    /// What the file held in the last commit, once read, when it is in one.
    pub(crate) base: Option<String>,
    /// How each line differs from the last commit, by line.
    pub(crate) line_changes: Vec<Option<crate::gutter::LineChange>>,
    /// The blocks folded away.
    pub folds: Vec<crate::fold::Fold>,
    /// The blocks the language server said can be folded, for the version of the text they are about.
    pub(crate) server_folds: (i32, Vec<pm_protocol::FoldRange>),
    /// The stretches that are edited together with the one the caret is in, as bytes of the text, while the caret is in one.
    pub(crate) linked: Option<Vec<Range<usize>>>,
    /// The rectangle being selected with the keys, while one is.
    pub(crate) column: Option<crate::column::ColumnBox>,
    /// What modal editing is in the middle of in this document: its mode, the keys gathered towards a command and its marks.
    pub(crate) vim: pm_vim::State,
    /// Where undo stood when modal editing began typing, until it ends.
    pub(crate) vim_typing_from: Option<usize>,
    /// Where the carets were before each move that changed nothing else, the latest last, to take back.
    pub(crate) cursor_past: Vec<crate::cursor_history::Carets>,
    /// Where the carets were before each move that was taken back, the latest last, to make again.
    pub(crate) cursor_future: Vec<crate::cursor_history::Carets>,
    /// The snippet whose blanks are being filled in, while one is.
    pub(crate) snippet: Option<crate::snippet::Session>,
    /// What the language server said to show in and beside the text.
    pub(crate) annotations: crate::annotations::Annotations,
    /// The brackets worked out last, with their depth.
    pub(crate) depths: crate::depth::DepthCache,
    /// The runnable places worked out last.
    pub(crate) runs: crate::runs::RunCache,
    /// The private values worked out last.
    pub(crate) redacts: crate::redact::RedactCache,
    /// The highlights worked out last.
    painted: HighlightCache,
    /// The highlights worked out last for the minimap.
    mapped: HighlightCache,
}

/// The kind of highlight a name a language server calls `kind` is drawn as.
fn kind_of(kind: pm_protocol::TokenKind) -> Kind {
    use pm_protocol::TokenKind;
    match kind {
        TokenKind::Keyword => Kind::Keyword,
        TokenKind::Function => Kind::Function,
        TokenKind::Type => Kind::Type,
        TokenKind::String => Kind::String,
        TokenKind::Constant => Kind::Constant,
        TokenKind::Comment => Kind::Comment,
        TokenKind::Property => Kind::Property,
        TokenKind::Operator => Kind::Operator,
        TokenKind::Variable => Kind::Variable,
    }
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
            language_editing: Editing::default(),
            indentation: (None, None),
            synced: false,
            version: 0,
            triggers: Vec::new(),
            signature_triggers: Vec::new(),
            format_triggers: Vec::new(),
            formats_ranges: false,
            folds_supported: false,
            links_supported: false,
            semantic_tokens_supported: false,
            inlay_hints_supported: false,
            code_lens_supported: false,
            document_links_supported: false,
            colors_supported: false,
            inline_values_supported: false,
            server_problem: None,
            diagnostics: Vec::new(),
            completion: None,
            hover: None,
            reach: None,
            grown: Vec::new(),
            occurrences: (0, Vec::new()),
            signature: None,
            base: None,
            line_changes: Vec::new(),
            folds: Vec::new(),
            server_folds: (0, Vec::new()),
            linked: None,
            column: None,
            vim: pm_vim::State::default(),
            vim_typing_from: None,
            cursor_past: Vec::new(),
            cursor_future: Vec::new(),
            snippet: None,
            annotations: crate::annotations::Annotations::default(),
            depths: crate::depth::DepthCache::default(),
            runs: crate::runs::RunCache::default(),
            redacts: crate::redact::RedactCache::default(),
            painted: HighlightCache::default(),
            mapped: HighlightCache::default(),
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
        self.carry_annotations(&change);
        if self.base.is_some() {
            self.compare_with_base();
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
        self.mapped = HighlightCache::default();
    }

    /// Takes in `syntax`, the text parsed with the grammar of the document's language.
    pub fn with_syntax(&mut self, syntax: Syntax) {
        self.language_editing = syntax.grammar().editing().clone();
        self.syntax = Some(syntax);
        self.set_indentation(self.indentation.0, self.indentation.1);
    }

    /// Indents with `columns` columns a level and with tabs when `tabs`, each in place of the language's own when given.
    pub fn set_indentation(&mut self, columns: Option<usize>, tabs: Option<bool>) {
        self.indentation = (columns, tabs);
        let mut editing = self.language_editing.clone();
        if let Some(columns) = columns {
            editing.indent = columns;
        }
        if let Some(tabs) = tabs {
            editing.tabs = tabs;
        }
        self.field.set_word_characters(&editing.word_characters);
        self.editing = editing;
    }

    /// The highlighted stretches of each line of `lines`, as offsets into the line, worked out again only when the text was parsed again or the lines fall outside the ones worked out last.
    pub fn highlights(&self, lines: Range<usize>) -> Vec<Vec<(Range<usize>, Kind)>> {
        self.highlights_kept(&self.painted, lines)
    }

    /// The highlighted stretches of each line of `lines` for the minimap, kept apart from the ones the lines in view keep so that drawing both does not work either out again.
    pub fn minimap_highlights(&self, lines: Range<usize>) -> Vec<Vec<(Range<usize>, Kind)>> {
        self.highlights_kept(&self.mapped, lines)
    }

    /// The highlighted stretches of the line at `index`, as offsets into the line, worked out for that line alone and not kept.
    pub fn line_highlights(&self, index: usize) -> Vec<(Range<usize>, Kind)> {
        let Some(syntax) = &self.syntax else {
            return Vec::new();
        };
        self.painted_window(syntax, index, (index + 1).min(self.lines()))
            .spans
            .pop()
            .unwrap_or_default()
    }

    /// The highlighted stretches of each line of `lines`, from what `kept` holds when it covers them and from the text parsed otherwise, in which case it is made to hold them and the lines near them.
    fn highlights_kept(
        &self,
        kept: &HighlightCache,
        lines: Range<usize>,
    ) -> Vec<Vec<(Range<usize>, Kind)>> {
        let Some(syntax) = &self.syntax else {
            return vec![Vec::new(); lines.len()];
        };
        let Ok(mut painted) = kept.0.lock() else {
            return vec![Vec::new(); lines.len()];
        };
        let stale = painted.revision != syntax.revision()
            || lines.start < painted.lines.start
            || lines.end > painted.lines.end;
        if stale {
            let first = lines.start.saturating_sub(HIGHLIGHT_MARGIN);
            let last = (lines.end + HIGHLIGHT_MARGIN).min(self.lines());
            *painted = self.painted_window(syntax, first, last);
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

    /// The highlights of the lines from `first` until `last`, worked out from `syntax`.
    fn painted_window(&self, syntax: &Syntax, first: usize, last: usize) -> Painted {
        let text = self.field.text();
        let window =
            self.lines.start(first, text)..self.line(last.saturating_sub(1).max(first)).end;
        let mut spans = vec![Vec::new(); last.saturating_sub(first)];
        for highlight in syntax.highlights(text, window.clone()) {
            let mut row = self.line_of(highlight.range.start);
            while row < last && row >= first {
                let line = self.line(row);
                if line.start > highlight.range.end {
                    break;
                }
                let low = highlight.range.start.max(line.start);
                let high = highlight.range.end.min(line.end);
                if low < high {
                    spans[row - first].push((low - line.start..high - line.start, highlight.kind));
                }
                if highlight.range.end <= line.end + 1 {
                    break;
                }
                row += 1;
            }
        }
        for (range, kind) in self.names_in(window) {
            let row = self.line_of(range.start);
            if row < first || row >= last {
                continue;
            }
            let line = self.line(row);
            let (low, high) = (
                range.start - line.start,
                range.end.min(line.end) - line.start,
            );
            if low >= high {
                continue;
            }
            let held = &mut spans[row - first];
            *held = held
                .drain(..)
                .flat_map(|(held, tone)| {
                    let before = held.start..held.end.min(low);
                    let after = held.start.max(high)..held.end;
                    let apart = held.end <= low || held.start >= high;
                    let pieces = if apart {
                        vec![held]
                    } else {
                        vec![before, after]
                    };
                    pieces
                        .into_iter()
                        .filter(|piece| piece.start < piece.end)
                        .map(move |piece| (piece, tone))
                })
                .collect();
            held.push((low..high, kind_of(*kind)));
        }
        Painted {
            revision: syntax.revision(),
            lines: first..last,
            spans,
        }
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
