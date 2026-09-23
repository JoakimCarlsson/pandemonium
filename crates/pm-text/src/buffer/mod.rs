//! One open file: its text, where it is being edited, and what it is.
//!
//! A buffer is the whole of what the editor knows about a file — the rope it
//! is stored in, the syntax tree that follows the rope, the selection, what
//! it has been through and whatever a language server has said about it.
//! Reading it is here; [`edit`] holds everything that changes the text and
//! [`motion`] everything that moves the cursor over it, because the two are
//! different jobs over the same rope.

mod cursors;
mod edit;
mod folds;
mod motion;

use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ropey::Rope;

use crate::cursor::{Position, Selection};
use crate::diagnostic::Diagnostic;
use crate::hint::Hint;
use crate::history::History;
use crate::indent::Indent;
use crate::language::Language;
use crate::syntax::{Highlight, Highlights, Syntax};

/// One file, open for reading and editing.
pub struct Buffer {
    /// Where the file lives.
    path: PathBuf,
    /// The text itself.
    text: Rope,
    /// The language it is written in, when the editor knows the extension.
    language: Option<Language>,
    /// Its syntax tree, when its language has a grammar that loaded.
    syntax: Option<Syntax>,
    /// How the file is indented, judged by the lines that are.
    indent: Indent,
    /// How the reader indents a file that does not say, which is also how
    /// wide a tab character is drawn.
    habit: Indent,
    /// What is selected, and where the cursor is.
    selection: Selection,
    /// The other cursors, when the reader has asked for more than one.
    extra: Vec<Selection>,
    /// The column vertical motion is aiming at.
    goal_column: Option<usize>,
    /// How many times the text has changed, as a language server counts.
    version: i32,
    /// What the text has been through, and what can be taken back.
    history: History,
    /// How deep the history was when the file was last on disk.
    saved_depth: usize,
    /// What a language server last said about this file.
    diagnostics: Vec<Diagnostic>,
    /// What a server has written into the lines that the file does not hold.
    hints: Vec<Hint>,
    /// What a language server makes of every name in the file.
    semantics: Vec<(Range<Position>, Highlight)>,
}

impl Buffer {
    /// Reads the file at `path` into a buffer.
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let text = Rope::from_str(&std::fs::read_to_string(&path)?);
        Ok(Self::of(path, text))
    }

    /// A buffer holding `text`, called `path` though nothing is there.
    ///
    /// This is how a view onto something that is not a file on disk — a
    /// commit message, a scratch buffer — gets the same editor the working
    /// copy has, without a second kind of document behind it.
    pub fn holding(path: impl Into<PathBuf>, text: &str) -> Self {
        Self::of(path.into(), Rope::from_str(text))
    }

    /// A buffer holding `text`, as though it had been read from `path`.
    pub fn of(path: PathBuf, text: Rope) -> Self {
        let language = Language::of(&path);
        let mut syntax = language.and_then(Syntax::new);
        if let Some(syntax) = syntax.as_mut() {
            syntax.parse(&text);
        }

        Self {
            indent: Indent::of(&text, Indent::default()),
            habit: Indent::default(),
            path,
            text,
            language,
            syntax,
            selection: Selection::default(),
            extra: Vec::new(),
            goal_column: None,
            version: 0,
            history: History::default(),
            saved_depth: 0,
            diagnostics: Vec::new(),
            hints: Vec::new(),
            semantics: Vec::new(),
        }
    }

    /// Where the file lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's own name, without the directories above it.
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// The language it is written in, when the editor knows the extension.
    pub fn language(&self) -> Option<Language> {
        self.language
    }

    /// How the file is indented.
    pub fn indent(&self) -> Indent {
        self.indent
    }

    /// Indents the way `habit` says wherever the file does not, and draws a
    /// tab as wide as it says.
    pub fn set_habit(&mut self, habit: Indent) {
        self.habit = habit;
        self.indent = Indent::of(&self.text, habit);
    }

    /// How wide a tab character is drawn, in characters.
    pub fn tab_width(&self) -> usize {
        self.habit.width.max(1)
    }

    /// Whether the text differs from what is on disk.
    pub fn is_dirty(&self) -> bool {
        self.history.depth() != self.saved_depth
    }

    /// How many times the text has changed, as a language server counts.
    pub fn version(&self) -> i32 {
        self.version
    }

    /// The whole text, as a language server wants it.
    pub fn contents(&self) -> String {
        self.text.to_string()
    }

    /// How many lines the buffer holds.
    pub fn line_count(&self) -> usize {
        self.text.len_lines()
    }

    /// How many characters `line` holds, the line break excluded.
    pub fn line_len(&self, line: usize) -> usize {
        if line >= self.text.len_lines() {
            return 0;
        }
        let text = self.text.line(line);
        let mut len = text.len_chars();
        for ending in ['\n', '\r'] {
            if len > 0 && text.char(len - 1) == ending {
                len -= 1;
            }
        }
        len
    }

    /// The characters of `line`, the line break excluded.
    pub fn line_chars(&self, line: usize) -> impl Iterator<Item = char> + '_ {
        let len = self.line_len(line);
        let start = if line < self.text.len_lines() {
            self.text.line_to_char(line)
        } else {
            self.text.len_chars()
        };
        self.text.slice(start..start + len).chars()
    }

    /// The text of `line`, the line break excluded.
    pub fn line_text(&self, line: usize) -> String {
        self.line_chars(line).collect()
    }

    /// The character at `position`, if the text reaches that far.
    pub fn char_at(&self, position: Position) -> Option<char> {
        self.line_chars(position.line).nth(position.column)
    }

    /// The text `range` covers, however many lines it spans.
    pub fn text_in(&self, range: Range<Position>) -> String {
        let (start, end) = (self.char_of(range.start), self.char_of(range.end));
        self.text.slice(start.min(end)..end.max(start)).to_string()
    }

    /// What a server has written into the lines that the file does not hold.
    pub fn hints(&self) -> &[Hint] {
        &self.hints
    }

    /// Replaces what a server had written into the lines.
    pub fn set_hints(&mut self, hints: Vec<Hint>) {
        self.hints = hints;
        self.hints.sort_by_key(|hint| hint.position);
    }

    /// The hints drawn on `line`, in the order they are drawn.
    pub fn hints_on(&self, line: usize) -> impl Iterator<Item = &Hint> {
        self.hints
            .iter()
            .filter(move |hint| hint.position.line == line)
    }

    /// How many columns the hints in front of `position` take.
    fn hinted(&self, position: Position) -> usize {
        self.hints_on(position.line)
            .take_while(|hint| hint.position.column <= position.column)
            .map(Hint::width)
            .sum()
    }

    /// The column `position` is drawn at, tabs and hints counted in.
    pub fn display_column(&self, position: Position) -> usize {
        let mut column = 0;
        for (index, ch) in self.line_chars(position.line).enumerate() {
            if index >= position.column {
                break;
            }
            column += if ch == '\t' {
                self.tab_width() - column % self.tab_width()
            } else {
                1
            };
        }
        column
            + position.column.saturating_sub(self.line_len(position.line))
            + self.hinted(position)
    }

    /// The place on `line` drawn at `column`, tabs and hints counted in.
    pub fn position_at_display(&self, line: usize, column: usize) -> Position {
        let mut drawn = 0;
        for (index, ch) in self.line_chars(line).enumerate() {
            drawn += self
                .hints_on(line)
                .filter(|hint| hint.position.column == index)
                .map(Hint::width)
                .sum::<usize>();
            let width = if ch == '\t' {
                self.tab_width() - drawn % self.tab_width()
            } else {
                1
            };
            if drawn + width > column {
                return Position::new(line, index + usize::from(drawn + width / 2 < column));
            }
            drawn += width;
        }
        Position::new(line, self.line_len(line) + column.saturating_sub(drawn))
    }

    /// How wide `line` is drawn, in characters.
    pub fn display_width(&self, line: usize) -> usize {
        self.display_column(Position::new(line, self.line_len(line)))
    }

    /// The widest any of `lines` is drawn, in characters.
    pub fn widest(&self, lines: Range<usize>) -> usize {
        lines
            .take_while(|line| *line < self.line_count())
            .map(|line| self.display_width(line))
            .max()
            .unwrap_or(0)
    }

    /// What is selected, and where the cursor is.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// What a language server last said about this file.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The diagnostic covering `position`, the most serious one first.
    pub fn diagnostic_at(&self, position: Position) -> Option<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|found| (found.range.start..=found.range.end).contains(&position))
            .min_by_key(|found| found.severity)
    }

    /// Replaces what a language server had said about this file.
    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics = diagnostics;
    }

    /// Replaces what a language server makes of the names in this file.
    pub fn set_semantics(&mut self, semantics: Vec<(Range<Position>, Highlight)>) {
        self.semantics = semantics;
    }

    /// The highlights of `lines`: what the grammar found, then what a server
    /// knows.
    ///
    /// The grammar answers about every file the moment it is opened and the
    /// server answers about the ones it serves a moment later, so the two are
    /// one pass with the better answer written last rather than a choice
    /// between them.
    pub fn highlights(&mut self, lines: Range<usize>) -> Highlights {
        let mut highlights = match self.syntax.as_mut() {
            Some(syntax) => syntax.highlights(&self.text, lines.clone()),
            None => Highlights::default(),
        };
        for (span, highlight) in &self.semantics {
            if span.end.line < lines.start || span.start.line >= lines.end {
                continue;
            }
            highlights.repaint(span.clone(), *highlight);
        }
        highlights
    }

    /// The bracket matching the one at or before the cursor, if there is one.
    pub fn matching_bracket(&self) -> Option<(Position, Position)> {
        let head = self.selection.head;
        for at in [head, Position::new(head.line, head.column.checked_sub(1)?)] {
            if let Some(other) = self.matched(at) {
                return Some((at, other));
            }
        }
        None
    }

    /// Writes the buffer to disk.
    pub fn save(&mut self) -> io::Result<()> {
        std::fs::write(&self.path, self.text.to_string())?;
        self.history.commit();
        self.saved_depth = self.history.depth();
        Ok(())
    }

    /// `position` brought inside the text it points into.
    pub fn clamped(&self, position: Position) -> Position {
        let line = position.line.min(self.line_count().saturating_sub(1));
        Position::new(line, position.column.min(self.line_len(line)))
    }

    /// How many characters the buffer holds.
    pub(crate) fn len_chars(&self) -> usize {
        self.text.len_chars()
    }

    /// The character offset `position` comes to.
    pub(crate) fn char_of(&self, position: Position) -> usize {
        let position = self.clamped(position);
        self.text.line_to_char(position.line) + position.column
    }

    /// The place in the text `offset` characters in comes to.
    pub(crate) fn position_of(&self, offset: usize) -> Position {
        let offset = offset.min(self.text.len_chars());
        let line = self.text.char_to_line(offset);
        Position::new(line, offset - self.text.line_to_char(line))
    }

    /// The partner of the bracket at `at`, when one is there and it matches.
    fn matched(&self, at: Position) -> Option<Position> {
        const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

        let bracket = self.char_at(at)?;
        let (forward, partner) = PAIRS.iter().find_map(|(open, close)| match bracket {
            ch if ch == *open => Some((true, *close)),
            ch if ch == *close => Some((false, *open)),
            _ => None,
        })?;

        let start = self.char_of(at);
        let mut depth = 0i32;
        let steps: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new(start..self.text.len_chars())
        } else {
            Box::new((0..=start).rev())
        };
        for offset in steps {
            let ch = self.text.char(offset);
            if ch == bracket {
                depth += 1;
            } else if ch == partner {
                depth -= 1;
                if depth == 0 {
                    return Some(self.position_of(offset));
                }
            }
        }
        None
    }
}
