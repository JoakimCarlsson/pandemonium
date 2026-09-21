//! One open file: its text, where it is being edited, and what it is.
//!
//! A buffer is the whole of what the editor knows about a file — the rope it
//! is stored in, the syntax tree that follows the rope, the selection, and
//! whatever a language server has said about it. Every change to the text
//! goes through one method, because the rope, the tree and the version a
//! server is told about have to move together or not at all.

use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ropey::Rope;
use tree_sitter::{InputEdit, Point};

use crate::cursor::{Motion, Position, Selection};
use crate::diagnostic::Diagnostic;
use crate::language::Language;
use crate::syntax::{Highlights, Syntax};

/// How many spaces the Tab key puts in.
const INDENT: usize = 4;

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
    /// What is selected, and where the cursor is.
    selection: Selection,
    /// The column vertical motion is aiming at.
    goal_column: Option<usize>,
    /// How many times the text has changed, as a language server counts.
    version: i32,
    /// Whether the text differs from what is on disk.
    dirty: bool,
    /// What a language server last said about this file.
    diagnostics: Vec<Diagnostic>,
}

impl Buffer {
    /// Reads the file at `path` into a buffer.
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let text = Rope::from_str(&std::fs::read_to_string(&path)?);
        let language = Language::of(&path);
        let mut syntax = language.and_then(Syntax::new);
        if let Some(syntax) = syntax.as_mut() {
            syntax.parse(&text);
        }

        Ok(Self {
            path,
            text,
            language,
            syntax,
            selection: Selection::default(),
            goal_column: None,
            version: 0,
            dirty: false,
            diagnostics: Vec::new(),
        })
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

    /// Whether the text differs from what is on disk.
    pub fn is_dirty(&self) -> bool {
        self.dirty
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

    /// What is selected, and where the cursor is.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// What a language server last said about this file.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Replaces what a language server had said about this file.
    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics = diagnostics;
    }

    /// The highlights of `lines`, when the buffer has a syntax tree.
    pub fn highlights(&mut self, lines: Range<usize>) -> Highlights {
        match self.syntax.as_mut() {
            Some(syntax) => syntax.highlights(&self.text, lines),
            None => Highlights::default(),
        }
    }

    /// Puts the cursor at `position`, extending the selection when asked.
    pub fn place(&mut self, position: Position, extend: bool) {
        let head = self.clamped(position);
        self.selection = Selection {
            anchor: if extend { self.selection.anchor } else { head },
            head,
        };
        self.goal_column = None;
    }

    /// Selects the word `position` falls in, as a double click does.
    pub fn select_word(&mut self, position: Position) {
        let position = self.clamped(position);
        let chars = self.line_chars(position.line).collect::<Vec<_>>();
        let kind = chars.get(position.column).copied().map(class);
        let matches = |index: usize| chars.get(index).copied().map(class) == kind;

        let mut start = position.column;
        while start > 0 && matches(start - 1) {
            start -= 1;
        }
        let mut end = position.column;
        while end < chars.len() && matches(end) {
            end += 1;
        }

        self.selection = Selection {
            anchor: Position::new(position.line, start),
            head: Position::new(position.line, end),
        };
        self.goal_column = None;
    }

    /// Selects the whole of the line `position` falls on.
    pub fn select_line(&mut self, position: Position) {
        let line = position.line.min(self.line_count().saturating_sub(1));
        self.selection = Selection {
            anchor: Position::new(line, 0),
            head: Position::new(line, self.line_len(line)),
        };
        self.goal_column = None;
    }

    /// Selects everything the buffer holds.
    pub fn select_all(&mut self) {
        let last = self.line_count().saturating_sub(1);
        self.selection = Selection {
            anchor: Position::default(),
            head: Position::new(last, self.line_len(last)),
        };
        self.goal_column = None;
    }

    /// Moves the cursor, extending the selection when asked.
    pub fn move_cursor(&mut self, motion: Motion, extend: bool) {
        let head = self.moved(motion);
        self.selection = Selection {
            anchor: if extend { self.selection.anchor } else { head },
            head,
        };
        if !motion.keeps_goal_column() {
            self.goal_column = None;
        }
    }

    /// The text the selection covers.
    pub fn selected_text(&self) -> String {
        let (start, end) = (
            self.char_of(self.selection.start()),
            self.char_of(self.selection.end()),
        );
        self.text.slice(start..end).to_string()
    }

    /// Puts `text` in, replacing whatever was selected.
    pub fn insert(&mut self, text: &str) {
        let range = self.selection.start()..self.selection.end();
        self.replace(range, text);
    }

    /// Puts a line break in, indented the way the line before it is.
    pub fn insert_newline(&mut self) {
        let indent = self
            .line_chars(self.selection.start().line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .take(self.selection.start().column)
            .collect::<String>();
        self.insert(&format!("\n{indent}"));
    }

    /// Puts one step of indentation in.
    pub fn insert_indent(&mut self) {
        self.insert(&" ".repeat(INDENT));
    }

    /// Takes out the selection, or the character before the cursor.
    pub fn backspace(&mut self) {
        if !self.selection.is_empty() {
            return self.insert("");
        }
        let head = self.selection.head;
        let start = match (head.line, head.column) {
            (0, 0) => return,
            (line, 0) => Position::new(line - 1, self.line_len(line - 1)),
            (line, column) => Position::new(line, column - 1),
        };
        self.replace(start..head, "");
    }

    /// Takes out the selection, or the character after the cursor.
    pub fn delete(&mut self) {
        if !self.selection.is_empty() {
            return self.insert("");
        }
        let head = self.selection.head;
        let last = self.line_count().saturating_sub(1);
        let end = match (head.line, head.column) {
            (line, column) if column < self.line_len(line) => Position::new(line, column + 1),
            (line, _) if line < last => Position::new(line + 1, 0),
            _ => return,
        };
        self.replace(head..end, "");
    }

    /// Writes the buffer to disk.
    pub fn save(&mut self) -> io::Result<()> {
        std::fs::write(&self.path, self.text.to_string())?;
        self.dirty = false;
        Ok(())
    }

    /// `position` brought inside the text it points into.
    pub fn clamped(&self, position: Position) -> Position {
        let line = position.line.min(self.line_count().saturating_sub(1));
        Position::new(line, position.column.min(self.line_len(line)))
    }

    /// Where `motion` takes the cursor from where it is.
    fn moved(&mut self, motion: Motion) -> Position {
        let head = self.selection.head;
        let last = self.line_count().saturating_sub(1);
        match motion {
            Motion::Left if head.column > 0 => Position::new(head.line, head.column - 1),
            Motion::Left if head.line > 0 => {
                Position::new(head.line - 1, self.line_len(head.line - 1))
            }
            Motion::Left => head,
            Motion::Right if head.column < self.line_len(head.line) => {
                Position::new(head.line, head.column + 1)
            }
            Motion::Right if head.line < last => Position::new(head.line + 1, 0),
            Motion::Right => head,
            Motion::Up => self.vertical(head, -1),
            Motion::Down => self.vertical(head, 1),
            Motion::PageUp(lines) => self.vertical(head, -(lines as isize)),
            Motion::PageDown(lines) => self.vertical(head, lines as isize),
            Motion::WordLeft => self.word_left(head),
            Motion::WordRight => self.word_right(head),
            Motion::LineStart => Position::new(head.line, self.indent_of(head.line, head.column)),
            Motion::LineEnd => Position::new(head.line, self.line_len(head.line)),
            Motion::BufferStart => Position::default(),
            Motion::BufferEnd => Position::new(last, self.line_len(last)),
        }
    }

    /// Where the cursor lands `lines` below where it is, or above.
    fn vertical(&mut self, head: Position, lines: isize) -> Position {
        let goal = *self.goal_column.get_or_insert(head.column);
        let line = head
            .line
            .saturating_add_signed(lines)
            .min(self.line_count().saturating_sub(1));
        Position::new(line, goal.min(self.line_len(line)))
    }

    /// The start of the word before `head`.
    fn word_left(&self, head: Position) -> Position {
        if head.column == 0 {
            return match head.line {
                0 => head,
                line => Position::new(line - 1, self.line_len(line - 1)),
            };
        }
        let chars = self.line_chars(head.line).collect::<Vec<_>>();
        let mut column = head.column;
        while column > 0 && class(chars[column - 1]) == Class::Space {
            column -= 1;
        }
        let kind = chars.get(column.wrapping_sub(1)).copied().map(class);
        while column > 0 && Some(class(chars[column - 1])) == kind {
            column -= 1;
        }
        Position::new(head.line, column)
    }

    /// The end of the word after `head`.
    fn word_right(&self, head: Position) -> Position {
        let chars = self.line_chars(head.line).collect::<Vec<_>>();
        if head.column >= chars.len() {
            return match head.line < self.line_count().saturating_sub(1) {
                true => Position::new(head.line + 1, 0),
                false => head,
            };
        }
        let mut column = head.column;
        while column < chars.len() && class(chars[column]) == Class::Space {
            column += 1;
        }
        let kind = chars.get(column).copied().map(class);
        while column < chars.len() && Some(class(chars[column])) == kind {
            column += 1;
        }
        Position::new(head.line, column)
    }

    /// The column the text of `line` begins at, or zero when already there.
    ///
    /// Home goes to the first character that is not a space, and to the
    /// margin from there, so the key alternates between the two places a
    /// line can be said to start.
    fn indent_of(&self, line: usize, column: usize) -> usize {
        let indent = self
            .line_chars(line)
            .take_while(|ch| ch.is_whitespace())
            .count();
        if column == indent { 0 } else { indent }
    }

    /// Replaces the text `range` covers with `text`, and moves on.
    ///
    /// This is the only path a change takes: the rope, the syntax tree, the
    /// version and the cursor all move here, so nothing can be told about
    /// one of them without the others.
    fn replace(&mut self, range: Range<Position>, text: &str) {
        let (start, end) = (self.clamped(range.start), self.clamped(range.end));
        if start == end && text.is_empty() {
            return;
        }

        let (first, last) = (self.char_of(start), self.char_of(end));
        let start_byte = self.text.char_to_byte(first);
        let old_end_byte = self.text.char_to_byte(last);

        self.text.remove(first..last);
        self.text.insert(first, text);

        let head = advanced(start, text);
        let edit = InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + text.len(),
            start_position: point(start, &self.text),
            old_end_position: Point::new(end.line, old_end_byte - line_byte(&self.text, end.line)),
            new_end_position: point(head, &self.text),
        };
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.edit(&edit);
            syntax.parse(&self.text);
        }

        self.selection = Selection::at(head);
        self.goal_column = None;
        self.version += 1;
        self.dirty = true;
    }

    /// The character offset `position` comes to.
    fn char_of(&self, position: Position) -> usize {
        let position = self.clamped(position);
        self.text.line_to_char(position.line) + position.column
    }
}

/// Where `text` leaves the cursor, having been put in at `start`.
fn advanced(start: Position, text: &str) -> Position {
    match text.rsplit_once('\n') {
        Some((before, rest)) => Position::new(
            start.line + before.matches('\n').count() + 1,
            rest.chars().count(),
        ),
        None => Position::new(start.line, start.column + text.chars().count()),
    }
}

/// `position` as the row and byte column a syntax tree counts in.
fn point(position: Position, text: &Rope) -> Point {
    let line = position.line.min(text.len_lines().saturating_sub(1));
    let start = text.line_to_char(line);
    let column = text.char_to_byte(start + position.column) - text.char_to_byte(start);
    Point::new(line, column)
}

/// The byte `line` begins at.
fn line_byte(text: &Rope, line: usize) -> usize {
    text.line_to_byte(line.min(text.len_lines().saturating_sub(1)))
}

/// The kinds of character a word motion tells apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    /// A space, a tab or another blank.
    Space,
    /// A letter, a digit or an underscore.
    Word,
    /// Anything else: punctuation, operators, brackets.
    Symbol,
}

/// Which kind of character `ch` is, for a word motion.
fn class(ch: char) -> Class {
    match ch {
        ch if ch.is_whitespace() => Class::Space,
        ch if ch.is_alphanumeric() || ch == '_' => Class::Word,
        _ => Class::Symbol,
    }
}
