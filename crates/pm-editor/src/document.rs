//! The text of a file open in the window: edited in place, line by line, told apart from what was last saved, parsed with its language's grammar and kept in step with its language server.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use crate::Field;
use crate::text::Taken;
use pm_buffer::{Change, Lines, Span, Step};
use pm_protocol::{Completion, Diagnostic, TextChange};
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
    /// Where the word's line is, in logical pixels of the window: the word's left edge and the
    /// line's top, which the list hangs under, or over when there is no room under it.
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

/// Something worked out from the rest of a document and kept, shared by its copies so a frame does not work it out again.
#[derive(Default)]
pub(crate) struct Kept<T>(pub(crate) Arc<Mutex<T>>);

impl<T> Clone for Kept<T> {
    /// Another handle on what is kept.
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> fmt::Debug for Kept<T> {
    /// Says nothing of what is held.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Kept")
    }
}

impl<T> PartialEq for Kept<T> {
    /// What is held is worked out from the rest of the document, so it never tells two documents apart.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The highlights worked out last.
type HighlightCache = Kept<Painted>;

/// Something that follows from the text, which the field already compares, so it never tells two documents apart.
#[derive(Clone, Debug, Default)]
pub(crate) struct Derived<T>(pub(crate) T);

impl<T> PartialEq for Derived<T> {
    /// What is held follows from the text, which is compared on its own.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> std::ops::Deref for Derived<T> {
    type Target = T;

    /// What is held.
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Derived<T> {
    /// What is held, to change.
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// The lines the last edit replaced, from the text of one revision to that of another.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LineEdit {
    /// The revision of the text before the edit.
    pub(crate) from: u64,
    /// The revision of the text the edit made.
    pub(crate) to: u64,
    /// The lines of the text before that the edit touched.
    pub(crate) old_rows: Range<usize>,
    /// The lines of the text after that hold what the edit put there.
    pub(crate) new_rows: Range<usize>,
}

/// What the folder holds, as compared with the text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Saved {
    /// The revision of the text when it was saved.
    revision: u64,
    /// How long the text was.
    length: usize,
    /// A fingerprint of it.
    print: u64,
}

/// A file's text, the caret and selection in it, whether it differs from what the folder holds, its syntax and what its language server says of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    /// The text, with the caret, the selection and what undo remembers.
    pub field: Field,
    /// Where each line starts.
    lines: Derived<Lines>,
    /// The text as the folder holds it, or nothing once saving it failed.
    saved: Option<Saved>,
    /// Whether the text of a revision is what the folder holds, as worked out last.
    clean: (u64, bool),
    /// The revision of the text the index of the lines and the syntax tree were last brought up to.
    indexed: u64,
    /// The lines the last edit replaced.
    pub(crate) last_edit: Option<LineEdit>,
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
    /// Whether the language server says what each name of a stretch of the file is, without the rest of it.
    pub semantic_ranges_supported: bool,
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
    pub(crate) base: Option<Arc<str>>,
    /// How each line differs from the last commit, by line.
    pub(crate) line_changes: crate::gutter::Changes,
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
    /// The functions, classes and comments of the syntax tree as of the parse numbered first, which modal editing's objects and motions go by.
    pub(crate) vim_nodes: Option<(u64, Arc<Vec<pm_vim::SyntaxNode>>)>,
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
    /// The lines folded away, worked out last.
    pub(crate) hidden: Kept<crate::fold::Hidden>,
    /// The rows the lines take as they wrap, worked out last.
    pub(crate) wraps: Kept<Option<crate::wrap::WrapIndex>>,
    /// The highlights worked out last.
    painted: HighlightCache,
    /// The highlights worked out last for the minimap.
    mapped: HighlightCache,
    /// The stretch finding is limited to, when it is.
    pub(crate) find_scope: Option<Range<usize>>,
    /// What the language server offered at the caret last.
    pub(crate) bulb: Option<crate::bulb::Bulb>,
    /// The selected text being carried to another place, while it is.
    pub(crate) drag: Option<crate::drag::Carried>,
}

/// Work on the text under way away from the thread that edits it: parsing it again and comparing it with the last commit; what was known before stands in until it is done.
#[derive(Clone)]
pub struct Pending {
    /// Parsing the text again, while it is under way.
    parsing: Option<pm_treesitter::Parsing>,
    /// Comparing the text with the last commit, while it is under way.
    diffing: Option<crate::gutter::Settling>,
}

impl Pending {
    /// Whether all of the work is done or was given up, so that what it found can be drawn.
    pub fn is_settled(&self) -> bool {
        self.parsing.as_ref().is_none_or(|held| held.is_settled())
            && self.diffing.as_ref().is_none_or(|held| held.is_settled())
    }

    /// Numbers that tell this work apart from other work under way.
    pub fn id(&self) -> (usize, usize) {
        (
            self.parsing.as_ref().map_or(0, |held| held.id()),
            self.diffing.as_ref().map_or(0, |held| held.id()),
        )
    }
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
        let saved = Saved {
            revision: 0,
            length: text.len(),
            print: fingerprint(&text),
        };
        let field = Field::new(text)
            .multiline(true)
            .read_only(read_only || truncated);
        let revision = field.revision();
        Self {
            field,
            lines: Derived(lines),
            saved: Some(Saved { revision, ..saved }),
            clean: (revision, true),
            indexed: revision,
            last_edit: None,
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
            semantic_ranges_supported: false,
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
            line_changes: crate::gutter::Changes::default(),
            folds: Vec::new(),
            server_folds: (0, Vec::new()),
            linked: None,
            column: None,
            vim: pm_vim::State::default(),
            vim_typing_from: None,
            vim_nodes: None,
            cursor_past: Vec::new(),
            cursor_future: Vec::new(),
            snippet: None,
            annotations: crate::annotations::Annotations::default(),
            depths: crate::depth::DepthCache::default(),
            runs: crate::runs::RunCache::default(),
            redacts: crate::redact::RedactCache::default(),
            hidden: Kept::default(),
            wraps: Kept::default(),
            painted: HighlightCache::default(),
            mapped: HighlightCache::default(),
            find_scope: None,
            bulb: None,
            drag: None,
        }
    }

    /// Notes what the text has become after the edits made to the field since this was last called, parses it again and says how it changed, if it did; `before` and `before_lines`, the text and lines before those edits, are no longer read, the field telling the steps it took.
    pub fn refresh(&mut self, _before: &str, _before_lines: &Lines) -> Option<Change> {
        self.settle_edit().map(|(change, _)| change)
    }

    /// Notes what the text has become after the edits made to the field since this was last called: moves the index of the lines, the syntax tree, the folds kept by bytes and the comparison with the last commit along the one stretch the edits replaced, and says what it was, as the text changed and as the language server is told it; nothing when the text is as it was.
    pub(crate) fn settle_edit(&mut self) -> Option<(Change, TextChange)> {
        let from = std::mem::replace(&mut self.indexed, self.field.revision());
        let step = match self.field.take_steps() {
            Taken::Steps(steps) => Step::compose(&steps, self.field.text()),
            Taken::Lost(length) => return Some(self.settle_whole(length)),
        };
        let Some(Step { change, removed }) = step else {
            self.note_revision();
            return None;
        };
        let text = self.field.text();
        let told = TextChange {
            span: Some(self.lines.span_replaced(&change, &removed, text)),
            text: change.text.clone(),
        };
        let (start, old_end) = (
            self.lines.point_of(change.range.start),
            self.lines.point_of(change.range.end),
        );
        self.lines.splice(&change);
        let new_end = self.lines.point_of(change.new_end());
        let edit = LineEdit {
            from,
            to: self.field.revision(),
            old_rows: start.0..old_end.0 + 1,
            new_rows: start.0..new_end.0 + 1,
        };
        if let Some(syntax) = &mut self.syntax {
            syntax.edit_at(&change, start, old_end, new_end, text);
        }
        self.carry_annotations(&change);
        self.carry_find_scope(&change);
        self.compare_with_base(Some(&edit));
        self.last_edit = Some(edit);
        self.note_revision();
        Some((change, told))
    }

    /// Takes in a text the field changed too often to follow step by step, from one `length` bytes long: indexes its lines and parses it afresh, and tells the change as the whole text replaced.
    fn settle_whole(&mut self, length: usize) -> (Change, TextChange) {
        let text = self.field.text().to_owned();
        let change = Change {
            range: 0..length,
            text: text.clone(),
        };
        self.lines = Derived(Lines::new(&text));
        self.last_edit = None;
        self.indexed = self.field.revision();
        if let Some(syntax) = &self.syntax {
            self.syntax = Syntax::parse(syntax.grammar().clone(), &text).or(self.syntax.take());
        }
        self.carry_annotations(&change);
        self.carry_find_scope(&change);
        self.compare_with_base(None);
        self.note_revision();
        (change, TextChange { span: None, text })
    }

    /// Works out whether the text as it is now is what the folder holds, looking at the text itself only when it is as long as what was saved.
    fn note_revision(&mut self) {
        let revision = self.field.revision();
        if self.clean.0 != revision {
            self.clean = (revision, self.matches_saved());
        }
    }

    /// Whether the text as it is now is what the folder holds.
    fn matches_saved(&self) -> bool {
        let text = self.field.text();
        self.saved.is_some_and(|saved| {
            saved.revision == self.field.revision()
                || (saved.length == text.len() && saved.print == fingerprint(text))
        })
    }

    /// The change `change` of the text whose lines `before_lines` indexed, as the language server is told it.
    pub fn text_change(change: &Change, before: &str, before_lines: &Lines) -> TextChange {
        TextChange {
            span: Some(before_lines.span_of(change.range.clone(), before)),
            text: change.text.clone(),
        }
    }

    /// The work on the text under way away from the thread that edits it, while there is any.
    pub fn pending(&self) -> Option<Pending> {
        let pending = Pending {
            parsing: self.syntax.as_ref().and_then(Syntax::parsing),
            diffing: self.line_changes.pending(),
        };
        (pending.parsing.is_some() || pending.diffing.is_some()).then_some(pending)
    }

    /// Whether the text differs from what the folder holds.
    pub fn is_dirty(&self) -> bool {
        if self.clean.0 == self.field.revision() {
            !self.clean.1
        } else {
            !self.matches_saved()
        }
    }

    /// Notes that the folder now holds the text as it is.
    pub fn mark_saved(&mut self) {
        let text = self.field.text();
        self.saved = Some(Saved {
            revision: self.field.revision(),
            length: text.len(),
            print: fingerprint(text),
        });
        self.clean = (self.field.revision(), true);
    }

    /// Notes that the folder may not hold the text, because writing it failed.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
        self.clean = (self.field.revision(), false);
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
        &self.lines.0
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
        if painted.revision != syntax.revision()
            && let Some(reparsed) = syntax.reparsed()
            && reparsed.from == painted.revision
        {
            *painted = self.patched(syntax, &painted, reparsed);
        }
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

    /// The highlights `painted` held for the parse `reparsed` follows, brought up to `syntax`: the lines the edit moved moved with it, and only the lines it touched and those whose syntax changed worked out again.
    fn patched(
        &self,
        syntax: &Syntax,
        painted: &Painted,
        reparsed: &pm_treesitter::Reparsed,
    ) -> Painted {
        let window = painted.lines.start.min(self.lines())..painted.lines.end.min(self.lines());
        let grown = reparsed.new_rows.end as isize - reparsed.old_rows.end as isize;
        let mut spans = Vec::with_capacity(window.len());
        let mut dirty = vec![false; window.len()];
        for (place, row) in window.clone().enumerate() {
            let old = if row < reparsed.new_rows.start {
                Some(row)
            } else if row >= reparsed.new_rows.end {
                row.checked_add_signed(-grown)
            } else {
                None
            };
            let held = old
                .filter(|old| painted.lines.contains(old))
                .filter(|_| {
                    !reparsed
                        .changed
                        .iter()
                        .any(|changed| changed.contains(&row))
                })
                .and_then(|old| painted.spans.get(old - painted.lines.start));
            match held {
                Some(held) => spans.push(held.clone()),
                None => {
                    spans.push(Vec::new());
                    dirty[place] = true;
                }
            }
        }
        let mut place = 0;
        while place < dirty.len() {
            if !dirty[place] {
                place += 1;
                continue;
            }
            let end = dirty[place..]
                .iter()
                .position(|held| !held)
                .map_or(dirty.len(), |gap| place + gap);
            let fresh = self.painted_window(syntax, window.start + place, window.start + end);
            for (offset, held) in fresh.spans.into_iter().enumerate() {
                spans[place + offset] = held;
            }
            place = end;
        }
        Painted {
            revision: syntax.revision(),
            lines: window,
            spans,
        }
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
