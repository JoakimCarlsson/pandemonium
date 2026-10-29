//! The text modal editing works on: a rope with character-counted places, the selections over it and the few things about the file that vim's commands read.
//!
//! The window's own documents count bytes and keep their history elsewhere, so modal editing works on a copy of the text that it is given before a key and gives back after it: [`Buffer::new`] takes the text and where the carets are, the engine changes it, and the window compares what is left with what it handed in. Undo is the window's: asking this buffer to undo only says so, in [`Buffer::take_history`], for the window to carry out on the document.

use std::ops::Range;

use ropey::Rope;

/// A place in a buffer: which line, and how many characters into it.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position {
    /// Which line, counted from zero.
    pub line: usize,
    /// How many characters into the line, counted from zero.
    pub column: usize,
}

impl Position {
    /// The position at `line` and `column`.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Where `text` leaves the cursor, having been put in here.
    pub fn after(self, text: &str) -> Self {
        match text.rsplit_once('\n') {
            Some((before, rest)) => Self::new(
                self.line + before.matches('\n').count() + 1,
                rest.chars().count(),
            ),
            None => Self::new(self.line, self.column + text.chars().count()),
        }
    }
}

/// What is selected: where the selection was begun and where it is now.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    /// The end the selection was begun at.
    pub anchor: Position,
    /// The end that moves, where the cursor is drawn.
    pub head: Position,
}

impl Selection {
    /// A selection of nothing, at `position`.
    pub const fn at(position: Position) -> Self {
        Self {
            anchor: position,
            head: position,
        }
    }

    /// Whether the selection covers no text.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The earlier of the two ends.
    pub fn start(&self) -> Position {
        self.anchor.min(self.head)
    }

    /// The later of the two ends.
    pub fn end(&self) -> Position {
        self.anchor.max(self.head)
    }

    /// The lines the selection reaches; one that ends at the very start of a line has not reached into it.
    pub fn lines(&self) -> std::ops::RangeInclusive<usize> {
        let (start, end) = (self.start(), self.end());
        let last = if end.line > start.line && end.column == 0 {
            end.line - 1
        } else {
            end.line
        };
        start.line..=last
    }
}

/// A way of moving the cursor that does not depend on where it is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    /// One character back.
    Left,
    /// One character on.
    Right,
    /// One line up, keeping the column aimed at.
    Up,
    /// One line down, keeping the column aimed at.
    Down,
    /// To the start of the word before the cursor.
    WordLeft,
    /// To the end of the word after the cursor.
    WordRight,
    /// To the first character of the line that is not a space.
    LineStart,
    /// To the end of the line.
    LineEnd,
    /// To the start of the buffer.
    BufferStart,
    /// To the end of the buffer.
    BufferEnd,
    /// Up by this many lines.
    PageUp(usize),
    /// Down by this many lines.
    PageDown(usize),
    /// To a place.
    To(Position),
}

/// One node of a syntax tree, as the text it spans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    /// What the grammar calls it: `function_item`, `class_declaration`.
    pub kind: String,
    /// The text it spans.
    pub range: Range<Position>,
    /// The text its body spans, when the grammar gives it one.
    pub body: Option<Range<Position>>,
    /// What it is called, when the grammar gives it a name.
    pub name: Option<String>,
}

/// How a file is indented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Indent {
    /// How many columns a step is.
    pub width: usize,
    /// Whether a step is a tab.
    pub tabs: bool,
}

impl Default for Indent {
    /// Four spaces.
    fn default() -> Self {
        Self {
            width: 4,
            tabs: false,
        }
    }
}

impl Indent {
    /// The text of one step.
    pub fn step(self) -> String {
        if self.tabs {
            "\t".to_owned()
        } else {
            " ".repeat(self.width)
        }
    }
}

/// The text of one file and the carets over it.
pub struct Buffer {
    /// The text.
    text: Rope,
    /// The primary selection.
    selection: Selection,
    /// The other selections.
    extra: Vec<Selection>,
    /// The column vertical motion is aiming at.
    goal_column: Option<usize>,
    /// How the file is indented.
    indent: Indent,
    /// What starts a comment that runs to the end of the line, when the language has one.
    line_comment: Option<String>,
    /// The nodes of the file's syntax tree that vim's objects and motions go by.
    nodes: Vec<SyntaxNode>,
    /// How many times the engine asked to undo and to redo.
    history: (usize, usize),
}

/// Where `position` is once the text between `start` and `end` has been replaced by text that ends at `after`.
fn shifted(position: Position, start: Position, end: Position, after: Position) -> Position {
    if position < start || (position == start && end > start) {
        return position;
    }
    if position < end {
        return after;
    }
    if position.line == end.line {
        Position::new(after.line, after.column + position.column - end.column)
    } else {
        Position::new(
            (position.line + after.line).saturating_sub(end.line),
            position.column,
        )
    }
}

/// Whether two selections have run into each other.
fn overlaps(left: Selection, right: Selection) -> bool {
    left.start().max(right.start()) <= left.end().min(right.end())
}

impl Buffer {
    /// A buffer holding `text`, with one caret at the start.
    pub fn new(text: &str) -> Self {
        Self {
            text: Rope::from_str(text),
            selection: Selection::default(),
            extra: Vec::new(),
            goal_column: None,
            indent: Indent::default(),
            line_comment: None,
            nodes: Vec::new(),
            history: (0, 0),
        }
    }

    /// Sets how the file is indented.
    pub fn set_indent(&mut self, indent: Indent) {
        self.indent = indent;
    }

    /// Sets what starts a comment that runs to the end of a line.
    pub fn set_line_comment(&mut self, token: Option<String>) {
        self.line_comment = token;
    }

    /// Sets the syntax nodes that function, class and comment objects go by.
    pub fn set_syntax_nodes(&mut self, nodes: Vec<SyntaxNode>) {
        self.nodes = nodes;
    }

    /// How many times the engine asked to undo and to redo since the last call, which the window carries out on the document.
    pub fn take_history(&mut self) -> (usize, usize) {
        std::mem::take(&mut self.history)
    }

    /// How the file is indented.
    pub fn indent(&self) -> Indent {
        self.indent
    }

    /// The width of a tab, in columns.
    fn tab_width(&self) -> usize {
        self.indent.width.max(1)
    }

    /// The whole text.
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

    /// The text `range` covers.
    pub fn text_in(&self, range: Range<Position>) -> String {
        let (start, end) = (self.char_of(range.start), self.char_of(range.end));
        self.text.slice(start.min(end)..end.max(start)).to_string()
    }

    /// `position` brought inside the text.
    pub fn clamped(&self, position: Position) -> Position {
        let line = position.line.min(self.line_count().saturating_sub(1));
        Position::new(line, position.column.min(self.line_len(line)))
    }

    /// How many characters the buffer holds.
    pub fn len_chars(&self) -> usize {
        self.text.len_chars()
    }

    /// The character `offset` characters in.
    pub fn char_at_offset(&self, offset: usize) -> Option<char> {
        (offset < self.text.len_chars()).then(|| self.text.char(offset))
    }

    /// The character offset `position` comes to.
    pub fn char_of(&self, position: Position) -> usize {
        let position = self.clamped(position);
        self.text.line_to_char(position.line) + position.column
    }

    /// The place `offset` characters in comes to.
    pub fn position_of(&self, offset: usize) -> Position {
        let offset = offset.min(self.text.len_chars());
        let line = self.text.char_to_line(offset);
        Position::new(line, offset - self.text.line_to_char(line))
    }

    /// The byte offset `position` comes to.
    pub fn byte_of(&self, position: Position) -> usize {
        self.text.char_to_byte(self.char_of(position))
    }

    /// The place the byte `offset` comes to.
    pub fn position_of_byte(&self, offset: usize) -> Position {
        self.position_of(self.text.byte_to_char(offset.min(self.text.len_bytes())))
    }

    /// The column `position` is drawn at, tabs counted in.
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
        column + position.column.saturating_sub(self.line_len(position.line))
    }

    /// The place on `line` drawn at `column`, tabs counted in.
    pub fn position_at_display(&self, line: usize, column: usize) -> Position {
        let mut drawn = 0;
        for (index, ch) in self.line_chars(line).enumerate() {
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

    /// How wide `line` is drawn.
    pub fn display_width(&self, line: usize) -> usize {
        self.display_column(Position::new(line, self.line_len(line)))
    }

    /// The nodes of the syntax tree whose kind `keep` accepts.
    pub fn syntax_nodes(&self, keep: &dyn Fn(&str) -> bool) -> Vec<SyntaxNode> {
        self.nodes
            .iter()
            .filter(|node| keep(&node.kind))
            .cloned()
            .collect()
    }

    /// The primary selection.
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Every selection, in the order they appear in the text.
    pub fn selections(&self) -> Vec<Selection> {
        let mut all = self.extra.clone();
        all.push(self.selection);
        all.sort_by_key(Selection::start);
        all
    }

    /// Whether there is more than one cursor.
    pub fn has_many_cursors(&self) -> bool {
        !self.extra.is_empty()
    }

    /// Selects exactly `selections`, the last of them being the primary; selections that ran into each other are merged.
    pub fn set_selections(&mut self, selections: Vec<Selection>) {
        let mut kept: Vec<Selection> = Vec::with_capacity(selections.len());
        for selection in selections {
            let selection = Selection {
                anchor: self.clamped(selection.anchor),
                head: self.clamped(selection.head),
            };
            if kept.iter().any(|held| overlaps(*held, selection)) {
                continue;
            }
            kept.push(selection);
        }
        self.selection = kept.pop().unwrap_or_default();
        self.extra = kept;
        self.goal_column = None;
    }

    /// Leaves one cursor, saying whether there had been others.
    pub fn collapse_cursors(&mut self) -> bool {
        let many = self.has_many_cursors();
        self.extra.clear();
        many
    }

    /// Puts the primary selection at `selection`.
    pub fn set_selection(&mut self, selection: Selection) {
        self.place_selection(selection);
    }

    /// Puts the primary selection at `selection` and forgets the column aimed at.
    fn place_selection(&mut self, selection: Selection) {
        self.selection = Selection {
            anchor: self.clamped(selection.anchor),
            head: self.clamped(selection.head),
        };
        self.goal_column = None;
    }

    /// Moves the cursor to `position`, taking the selection along when `extend`.
    pub fn place(&mut self, position: Position, extend: bool) {
        let head = self.clamped(position);
        self.set_selection(Selection {
            anchor: if extend { self.selection.anchor } else { head },
            head,
        });
    }

    /// Moves the cursor by `motion`, taking the selection along when `extend`.
    pub fn move_cursor(&mut self, motion: Motion, extend: bool) {
        let head = self.moved(motion, extend);
        let anchor = if extend { self.selection.anchor } else { head };
        self.selection = Selection {
            anchor: self.clamped(anchor),
            head: self.clamped(head),
        };
        if !matches!(
            motion,
            Motion::Up | Motion::Down | Motion::PageUp(_) | Motion::PageDown(_)
        ) {
            self.goal_column = None;
        }
    }

    /// Where `motion` takes the cursor.
    fn moved(&mut self, motion: Motion, extend: bool) -> Position {
        let head = self.selection.head;
        let last = self.line_count().saturating_sub(1);
        match motion {
            Motion::Left if !extend && !self.selection.is_empty() => self.selection.start(),
            Motion::Right if !extend && !self.selection.is_empty() => self.selection.end(),
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
            Motion::WordLeft => self.word_start(head),
            Motion::WordRight => self.word_end(head),
            Motion::LineStart => Position::new(
                head.line,
                self.line_chars(head.line)
                    .take_while(|ch| ch.is_whitespace())
                    .count(),
            ),
            Motion::LineEnd => Position::new(head.line, self.line_len(head.line)),
            Motion::BufferStart => Position::default(),
            Motion::BufferEnd => Position::new(last, self.line_len(last)),
            Motion::To(position) => self.clamped(position),
        }
    }

    /// The place `lines` lines from `head`, in the column aimed at.
    fn vertical(&mut self, head: Position, lines: isize) -> Position {
        let goal = *self.goal_column.get_or_insert(head.column);
        let line = head
            .line
            .saturating_add_signed(lines)
            .min(self.line_count().saturating_sub(1));
        Position::new(line, goal.min(self.line_len(line)))
    }

    /// The start of the word before `position`.
    fn word_start(&self, position: Position) -> Position {
        if position.column == 0 {
            return match position.line {
                0 => position,
                line => Position::new(line - 1, self.line_len(line - 1)),
            };
        }
        let chars: Vec<char> = self.line_chars(position.line).collect();
        let class = |ch: char| (ch.is_whitespace(), ch.is_alphanumeric() || ch == '_');
        let mut column = position.column.min(chars.len());
        while column > 0 && class(chars[column - 1]).0 {
            column -= 1;
        }
        let kind = chars.get(column.wrapping_sub(1)).copied().map(class);
        while column > 0 && Some(class(chars[column - 1])) == kind {
            column -= 1;
        }
        Position::new(position.line, column)
    }

    /// The end of the word after `position`.
    fn word_end(&self, position: Position) -> Position {
        let chars: Vec<char> = self.line_chars(position.line).collect();
        if position.column >= chars.len() {
            return if position.line + 1 < self.line_count() {
                Position::new(position.line + 1, 0)
            } else {
                position
            };
        }
        let class = |ch: char| (ch.is_whitespace(), ch.is_alphanumeric() || ch == '_');
        let mut column = position.column;
        while column < chars.len() && class(chars[column]).0 {
            column += 1;
        }
        let kind = chars.get(column).copied().map(class);
        while column < chars.len() && Some(class(chars[column])) == kind {
            column += 1;
        }
        Position::new(position.line, column)
    }

    /// The word `position` is in, or nothing where there is none.
    pub fn word_at(&self, position: Position) -> Range<Position> {
        let position = self.clamped(position);
        let chars: Vec<char> = self.line_chars(position.line).collect();
        let inside = |index: usize| {
            chars
                .get(index)
                .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_')
        };
        let mut start = position.column;
        while start > 0 && inside(start - 1) {
            start -= 1;
        }
        let mut end = position.column;
        while end < chars.len() && inside(end) {
            end += 1;
        }
        Position::new(position.line, start)..Position::new(position.line, end)
    }

    /// Replaces the text `range` covers with `text`, leaving the cursor after it.
    pub fn replace(&mut self, range: Range<Position>, text: &str) {
        let (start, end) = (self.clamped(range.start), self.clamped(range.end));
        let (start, end) = (start.min(end), start.max(end));
        if start == end && text.is_empty() {
            return;
        }
        let (first, last) = (self.char_of(start), self.char_of(end));
        self.text.remove(first..last);
        self.text.insert(first, text);
        let head = start.after(text);
        for other in &mut self.extra {
            other.anchor = shifted(other.anchor, start, end, head);
            other.head = shifted(other.head, start, end, head);
        }
        self.place_selection(Selection::at(head));
    }

    /// Makes `edits` as one change, latest place first so no edit moves the text a later one was measured against.
    pub fn apply_edits(&mut self, edits: Vec<(Range<Position>, String)>) {
        let mut edits: Vec<_> = edits.into_iter().enumerate().collect();
        edits.sort_by_key(|(index, (range, _))| {
            (std::cmp::Reverse(range.start), std::cmp::Reverse(*index))
        });
        let selection = self.selection;
        for (_, (range, text)) in edits {
            self.replace(range, &text);
        }
        self.set_selection(Selection {
            anchor: self.clamped(selection.anchor),
            head: self.clamped(selection.head),
        });
    }

    /// Runs `change`; the window makes one change of everything the engine did to the text in a key, so this groups nothing.
    pub fn grouped(&mut self, change: impl FnOnce(&mut Self)) {
        change(self);
    }

    /// How deep the undo history is; the window keeps it, so this is always the same.
    pub fn undo_depth(&self) -> usize {
        0
    }

    /// Makes the undo steps since the history was `depth` deep one step; the window makes one step of a key already.
    pub fn squash_since(&mut self, _depth: usize) {}

    /// Asks for the last change to be taken back, which the window does on the document.
    pub fn undo(&mut self) -> bool {
        self.history.0 += 1;
        true
    }

    /// Asks for the last change taken back to be made again, which the window does on the document.
    pub fn redo(&mut self) -> bool {
        self.history.1 += 1;
        true
    }

    /// Puts `text` in, replacing what is selected.
    pub fn insert(&mut self, text: &str) {
        let range = self.selection.start()..self.selection.end();
        self.replace(range, text);
    }

    /// Puts one typed character in.
    pub fn insert_typed(&mut self, ch: char) {
        self.insert(&ch.to_string());
    }

    /// Puts a line break in, indented as the line it leaves is.
    pub fn insert_newline(&mut self) {
        let head = self.selection.start();
        let indent: String = self
            .line_chars(head.line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .take(head.column)
            .collect();
        self.insert(&format!("\n{indent}"));
    }

    /// Puts an empty line below the one the cursor is on, indented as it is, and goes to it.
    pub fn insert_line_below(&mut self) {
        let line = self.selection.end().line;
        let end = Position::new(line, self.line_len(line));
        let indent: String = self
            .line_chars(line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        self.set_selection(Selection::at(end));
        self.insert(&format!("\n{indent}"));
    }

    /// Puts an empty line above the one the cursor is on, indented as it is, and goes to it.
    pub fn insert_line_above(&mut self) {
        let line = self.selection.start().line;
        let start = Position::new(line, 0);
        let indent: String = self
            .line_chars(line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect();
        self.replace(start..start, &format!("{indent}\n"));
        self.place(Position::new(line, indent.chars().count()), false);
    }

    /// Puts one step of indentation in, or indents every line selected.
    pub fn insert_indent(&mut self) {
        if !self.selection.is_empty() {
            return self.indent_lines();
        }
        let indent = self.indent;
        if indent.tabs {
            return self.insert("\t");
        }
        let column = self.display_column(self.selection.head);
        self.insert(&" ".repeat(indent.width - column % indent.width));
    }

    /// Indents every line the selection reaches by one step.
    pub fn indent_lines(&mut self) {
        let selection = self.selection;
        let step = self.indent.step();
        let width = step.chars().count();
        for line in selection.lines() {
            let start = Position::new(line, 0);
            self.replace(start..start, &step);
        }
        self.shift_selection(selection, width as isize);
    }

    /// Takes one step of indentation off every line the selection reaches.
    pub fn outdent_lines(&mut self) {
        let selection = self.selection;
        let step = self.indent.step().chars().count();
        let mut taken = 0;
        for line in selection.lines() {
            let lead: Vec<char> = self.line_chars(line).take(step).collect();
            let width = if lead.first() == Some(&'\t') {
                1
            } else {
                lead.iter().take_while(|ch| **ch == ' ').count()
            };
            if width > 0 {
                self.replace(Position::new(line, 0)..Position::new(line, width), "");
            }
            taken = width;
        }
        self.shift_selection(selection, -(taken as isize));
    }

    /// Moves both ends of `selection` by `columns` and makes it the selection.
    fn shift_selection(&mut self, selection: Selection, columns: isize) {
        let shift = |position: Position| {
            Position::new(
                position.line,
                position.column.saturating_add_signed(columns),
            )
        };
        self.set_selection(Selection {
            anchor: shift(selection.anchor),
            head: shift(selection.head),
        });
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
        let end = if head.column < self.line_len(head.line) {
            Position::new(head.line, head.column + 1)
        } else if head.line < last {
            Position::new(head.line + 1, 0)
        } else {
            return;
        };
        self.replace(head..end, "");
    }

    /// Takes out the selection, or the word before the cursor.
    pub fn delete_word_left(&mut self) {
        if !self.selection.is_empty() {
            return self.insert("");
        }
        let head = self.selection.head;
        let start = self.word_start(head);
        self.replace(start..head, "");
    }

    /// Joins the line below the cursor onto the line the cursor is on.
    pub fn join_lines(&mut self) {
        let lines = self.selection.lines();
        let (first, last) = (*lines.start(), *lines.end());
        let last = if last == first { first } else { last - 1 };
        for _ in first..=last {
            if first + 1 >= self.line_count() {
                break;
            }
            let end = Position::new(first, self.line_len(first));
            let text = self.line_text(first + 1);
            let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
            let separator = if text.trim().is_empty() || end.column == 0 {
                ""
            } else {
                " "
            };
            self.replace(end..Position::new(first + 1, indent), separator);
            self.place(end, false);
        }
    }

    /// Comments every line the selection reaches, or takes the comments off.
    pub fn toggle_comment(&mut self) {
        let Some(token) = self.line_comment.clone() else {
            return;
        };
        let lines = self.selection.lines();
        let filled: Vec<usize> = lines
            .clone()
            .filter(|line| !self.line_text(*line).trim().is_empty())
            .collect();
        let lines: Vec<usize> = if filled.is_empty() {
            lines.collect()
        } else {
            filled
        };
        let commented = lines
            .iter()
            .all(|line| self.line_text(*line).trim_start().starts_with(&token));
        let margin = lines
            .iter()
            .map(|line| {
                self.line_text(*line)
                    .chars()
                    .take_while(|ch| ch.is_whitespace())
                    .count()
            })
            .min()
            .unwrap_or(0);
        for line in lines {
            if commented {
                let text = self.line_text(line);
                let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
                let spaced = text.chars().nth(indent + token.chars().count()) == Some(' ');
                let width = token.chars().count() + usize::from(spaced);
                self.replace(
                    Position::new(line, indent)..Position::new(line, indent + width),
                    "",
                );
            } else {
                let at = Position::new(line, margin.min(self.line_len(line)));
                self.replace(at..at, &format!("{token} "));
            }
        }
    }

    /// Does `change` at every cursor, carrying the others past what it did, working from the end of the text backwards.
    pub fn at_each(&mut self, change: impl FnMut(&mut Self)) {
        self.at(self.selections(), change);
    }

    /// Does `change` once per line the cursors are on.
    pub fn on_each_line(&mut self, change: impl FnMut(&mut Self)) {
        let mut taken: Vec<std::ops::RangeInclusive<usize>> = Vec::new();
        let kept = self
            .selections()
            .into_iter()
            .filter(|selection| {
                let lines = selection.lines();
                let clash = taken
                    .iter()
                    .any(|held| held.start() <= lines.end() && lines.start() <= held.end());
                if !clash {
                    taken.push(lines);
                }
                !clash
            })
            .collect();
        self.at(kept, change);
    }

    /// Does `change` at each of `selections`, carrying the rest past it.
    fn at(&mut self, selections: Vec<Selection>, mut change: impl FnMut(&mut Self)) {
        if self.extra.is_empty() {
            return change(self);
        }
        let mut offsets: Vec<(usize, usize)> = selections
            .iter()
            .map(|selection| (self.char_of(selection.anchor), self.char_of(selection.head)))
            .collect();
        for index in (0..offsets.len()).rev() {
            let (anchor, head) = offsets[index];
            self.extra.clear();
            self.selection = Selection {
                anchor: self.position_of(anchor),
                head: self.position_of(head),
            };
            let before = self.len_chars();
            change(self);
            let moved = self.len_chars() as isize - before as isize;
            offsets[index] = (
                self.char_of(self.selection.anchor),
                self.char_of(self.selection.head),
            );
            for later in offsets.iter_mut().skip(index + 1) {
                later.0 = later.0.saturating_add_signed(moved);
                later.1 = later.1.saturating_add_signed(moved);
            }
        }
        let selections = offsets
            .iter()
            .map(|(anchor, head)| Selection {
                anchor: self.position_of(*anchor),
                head: self.position_of(*head),
            })
            .collect();
        self.set_selections(selections);
    }
}
