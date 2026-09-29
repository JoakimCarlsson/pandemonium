//! Everything that changes the text, and the one seam it all goes through.
//!
//! [`Buffer::replace`] is that seam: the rope, the syntax tree, the version a
//! server is told about, the cursor and the undo history all move there, so
//! nothing can be told about one of them without the others. Everything else
//! in this file is a way of working out which span to replace with what.

use std::ops::Range;

use ropey::Rope;
use tree_sitter::{InputEdit, Point};

use crate::buffer::Buffer;
use crate::cursor::{Position, Selection};
use crate::history::Change;

/// The brackets and quotes typing one of puts the other in.
const PAIRS: [(char, char); 5] = [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')];

impl Buffer {
    /// The text the selection covers.
    pub fn selected_text(&self) -> String {
        self.text_in(self.selection().start()..self.selection().end())
    }

    /// What copying right now would put on the clipboard.
    ///
    /// With nothing selected it is the whole line, line break and all, which
    /// is what every editor copies from an empty selection and what makes
    /// copy-and-paste with no selection duplicate a line.
    pub fn copied_text(&self) -> String {
        if !self.selection().is_empty() {
            return self.selected_text();
        }
        let line = self.selection().head.line;
        format!("{}\n", self.line_text(line))
    }

    /// Takes out what copying right now would have taken.
    pub fn cut(&mut self) {
        if self.selection().is_empty() {
            return self.delete_lines();
        }
        self.insert("");
    }

    /// Puts `text` in, as a line of its own when it was copied as one.
    ///
    /// Text that ends in a line break was a line when it was copied, so with
    /// nothing selected it goes in as a line rather than into the middle of
    /// the one the cursor is on.
    pub fn paste(&mut self, text: &str) {
        if !self.selection().is_empty() || !text.ends_with('\n') {
            return self.insert(text);
        }
        let line = self.selection().head.line;
        let column = self.selection().head.column;
        self.grouped(|buffer| {
            let start = Position::new(line, 0);
            buffer.replace(start..start, text);
            buffer.place(
                Position::new(line + text.matches('\n').count(), column),
                false,
            );
        });
    }

    /// Puts `text` in, replacing whatever was selected.
    pub fn insert(&mut self, text: &str) {
        let range = self.selection().start()..self.selection().end();
        self.replace(range, text);
    }

    /// Puts one typed character in, closing what it opens.
    ///
    /// Typing an opening bracket over a selection wraps the selection rather
    /// than replacing it; typing a closing bracket where that bracket
    /// already is steps over it instead of doubling it. Both are what makes
    /// automatic pairs help rather than fight.
    pub fn insert_typed(&mut self, ch: char) {
        let closing = PAIRS.iter().find(|(_, close)| *close == ch);
        if closing.is_some()
            && self.selection().is_empty()
            && self.char_at(self.selection().head) == Some(ch)
        {
            return self.move_cursor(crate::cursor::Motion::Right, false);
        }

        let Some((open, close)) = PAIRS.iter().copied().find(|(open, _)| *open == ch) else {
            return self.insert(&ch.to_string());
        };

        if !self.selection().is_empty() {
            let selected = self.selected_text();
            return self.surround(open, close, &selected);
        }
        if open == close && self.follows_word() {
            return self.insert(&ch.to_string());
        }

        let head = self.selection().head;
        self.grouped(|buffer| {
            buffer.insert(&format!("{open}{close}"));
            buffer.place(Position::new(head.line, head.column + 1), false);
        });
    }

    /// Puts `open` and `close` around `selected`, keeping it selected.
    fn surround(&mut self, open: char, close: char, selected: &str) {
        let (start, end) = (self.selection().start(), self.selection().end());
        self.grouped(|buffer| {
            buffer.replace(start..end, &format!("{open}{selected}{close}"));
            buffer.place(Position::new(start.line, start.column + 1), false);
            let head = if end.line == start.line {
                Position::new(end.line, end.column + 1)
            } else {
                end
            };
            buffer.place(head, true);
        });
    }

    /// Whether the character before the cursor is part of a word.
    fn follows_word(&self) -> bool {
        let head = self.selection().head;
        head.column
            .checked_sub(1)
            .and_then(|column| self.char_at(Position::new(head.line, column)))
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    }

    /// Puts a line break in, indented the way the line it leaves is.
    ///
    /// A break taken between a bracket and its partner leaves the partner on
    /// a line of its own, with the body indented between them, which is what
    /// pressing Enter inside a pair of braces is asking for.
    pub fn insert_newline(&mut self) {
        let head = self.selection().start();
        let indent = self
            .line_chars(head.line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .take(head.column)
            .collect::<String>();
        let opens = head
            .column
            .checked_sub(1)
            .and_then(|column| self.char_at(Position::new(head.line, column)))
            .is_some_and(|ch| matches!(ch, '(' | '[' | '{'));
        let closes = self
            .char_at(self.selection().end())
            .is_some_and(|ch| matches!(ch, ')' | ']' | '}'));

        if !opens {
            return self.insert(&format!("\n{indent}"));
        }
        let inner = format!("{indent}{}", self.indent().step());
        if !closes {
            return self.insert(&format!("\n{inner}"));
        }
        self.grouped(|buffer| {
            buffer.insert(&format!("\n{inner}\n{indent}"));
            buffer.place(Position::new(head.line + 1, inner.chars().count()), false);
        });
    }

    /// Puts one step of indentation in, or indents every line selected.
    pub fn insert_indent(&mut self) {
        if !self.selection().is_empty() {
            return self.indent_lines();
        }
        let indent = self.indent();
        if indent.tabs {
            return self.insert("\t");
        }
        let column = self.display_column(self.selection().head);
        self.insert(&" ".repeat(indent.width - column % indent.width));
    }

    /// Indents every line the selection reaches by one step.
    pub fn indent_lines(&mut self) {
        let selection = self.selection();
        let step = self.indent().step();
        let width = step.chars().count();
        self.grouped(|buffer| {
            for line in selection.lines() {
                let start = Position::new(line, 0);
                buffer.replace(start..start, &step);
            }
            buffer.shift_selection(selection, width as isize);
        });
    }

    /// Takes one step of indentation off every line the selection reaches.
    pub fn outdent_lines(&mut self) {
        let selection = self.selection();
        let step = self.indent().step().chars().count();
        let mut taken = std::collections::BTreeMap::new();
        self.grouped(|buffer| {
            for line in selection.lines() {
                let width = buffer
                    .line_chars(line)
                    .take(step)
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .count();
                if width == 0 {
                    continue;
                }
                taken.insert(line, width);
                buffer.replace(Position::new(line, 0)..Position::new(line, width), "");
            }
            let back = |position: Position| {
                let width = taken.get(&position.line).copied().unwrap_or_default();
                Position::new(position.line, position.column.saturating_sub(width))
            };
            buffer.set_selection(Selection {
                anchor: back(selection.anchor),
                head: back(selection.head),
            });
        });
    }

    /// Puts `selection` back, moved along its lines by `columns`.
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
    ///
    /// Backspacing through the indentation at the start of a line takes back
    /// a whole step of it, so a line indented by pressing Tab is un-indented
    /// by pressing Backspace once.
    pub fn backspace(&mut self) {
        if !self.selection().is_empty() {
            return self.insert("");
        }
        let head = self.selection().head;
        if let Some(pair) = self.empty_pair(head) {
            return self.replace(pair, "");
        }
        let start = match (head.line, head.column) {
            (0, 0) => return,
            (line, 0) => Position::new(line - 1, self.line_len(line - 1)),
            (line, column) => Position::new(line, column - self.indent_step(line, column)),
        };
        self.replace(start..head, "");
    }

    /// The pair the cursor sits between, when it sits between an empty one.
    ///
    /// Typing an opening bracket puts its partner in; taking the opening one
    /// back takes the partner with it, because the pair was one keystroke.
    fn empty_pair(&self, head: Position) -> Option<Range<Position>> {
        let before = self.char_at(Position::new(head.line, head.column.checked_sub(1)?))?;
        let after = self.char_at(head)?;
        PAIRS
            .iter()
            .any(|(open, close)| *open == before && *close == after)
            .then(|| {
                Position::new(head.line, head.column - 1)..Position::new(head.line, head.column + 1)
            })
    }

    /// Puts an empty line below the one the cursor is on, and goes to it.
    pub fn insert_line_below(&mut self) {
        let line = self.selection().end().line;
        let end = Position::new(line, self.line_len(line));
        let indent = self
            .line_chars(line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect::<String>();
        self.set_selection(Selection::at(end));
        self.insert(&format!("\n{indent}"));
    }

    /// Puts an empty line above the one the cursor is on, and goes to it.
    pub fn insert_line_above(&mut self) {
        let line = self.selection().start().line;
        let start = Position::new(line, 0);
        let indent = self
            .line_chars(line)
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect::<String>();
        self.grouped(|buffer| {
            buffer.replace(start..start, &format!("{indent}\n"));
            buffer.place(Position::new(line, indent.chars().count()), false);
        });
    }

    /// How many characters backspacing at `column` of `line` takes out.
    fn indent_step(&self, line: usize, column: usize) -> usize {
        let width = self.indent().width;
        let blank = self
            .line_chars(line)
            .take(column)
            .all(|ch| ch == ' ' || ch == '\t');
        let step = column % width;
        match blank && self.char_at(Position::new(line, column - 1)) == Some(' ') {
            true if step == 0 => width.min(column),
            true => step,
            false => 1,
        }
    }

    /// Takes out the selection, or the character after the cursor.
    pub fn delete(&mut self) {
        if !self.selection().is_empty() {
            return self.insert("");
        }
        let head = self.selection().head;
        let last = self.line_count().saturating_sub(1);
        let end = match (head.line, head.column) {
            (line, column) if column < self.line_len(line) => Position::new(line, column + 1),
            (line, _) if line < last => Position::new(line + 1, 0),
            _ => return,
        };
        self.replace(head..end, "");
    }

    /// Takes out the word before the cursor.
    pub fn delete_word_left(&mut self) {
        if !self.selection().is_empty() {
            return self.insert("");
        }
        let head = self.selection().head;
        let start = self.word_start(head);
        self.replace(start..head, "");
    }

    /// Takes out the word after the cursor.
    pub fn delete_word_right(&mut self) {
        if !self.selection().is_empty() {
            return self.insert("");
        }
        let head = self.selection().head;
        let end = self.word_end(head);
        self.replace(head..end, "");
    }

    /// Takes out everything from the cursor to the end of its line.
    pub fn delete_to_line_end(&mut self) {
        let head = self.selection().head;
        let end = Position::new(head.line, self.line_len(head.line));
        if head == end {
            return self.delete();
        }
        self.replace(head..end, "");
    }

    /// Takes out everything from the start of the line to the cursor.
    pub fn delete_to_line_start(&mut self) {
        let head = self.selection().head;
        let start = Position::new(head.line, 0);
        if head == start {
            return self.backspace();
        }
        self.replace(start..head, "");
    }

    /// Takes out every line the selection reaches.
    pub fn delete_lines(&mut self) {
        let lines = self.selection().lines();
        let (first, last) = (*lines.start(), *lines.end());
        let end = match last + 1 < self.line_count() {
            true => Position::new(last + 1, 0),
            false => Position::new(last, self.line_len(last)),
        };
        let start = match (last + 1 < self.line_count(), first) {
            (false, line) if line > 0 => Position::new(line - 1, self.line_len(line - 1)),
            _ => Position::new(first, 0),
        };
        self.replace(start..end, "");
    }

    /// Puts another copy of every line the selection reaches below it.
    pub fn duplicate_lines(&mut self) {
        let selection = self.selection();
        let lines = selection.lines();
        let (first, last) = (*lines.start(), *lines.end());
        let copied = (first..=last)
            .map(|line| self.line_text(line))
            .collect::<Vec<_>>()
            .join("\n");
        let at = Position::new(last, self.line_len(last));
        let span = last - first + 1;

        self.grouped(|buffer| {
            buffer.replace(at..at, &format!("\n{copied}"));
            buffer.set_selection(Selection {
                anchor: Position::new(selection.anchor.line + span, selection.anchor.column),
                head: Position::new(selection.head.line + span, selection.head.column),
            });
        });
    }

    /// Moves every line the selection reaches one line up.
    pub fn move_lines_up(&mut self) {
        let lines = self.selection().lines();
        let (first, last) = (*lines.start(), *lines.end());
        if first == 0 {
            return;
        }
        let mut moved = (first..=last)
            .map(|line| self.line_text(line))
            .collect::<Vec<_>>();
        moved.push(self.line_text(first - 1));
        self.reorder(first - 1, last, moved, -1);
    }

    /// Moves every line the selection reaches one line down.
    pub fn move_lines_down(&mut self) {
        let lines = self.selection().lines();
        let (first, last) = (*lines.start(), *lines.end());
        if last + 1 >= self.line_count() {
            return;
        }
        let mut moved = vec![self.line_text(last + 1)];
        moved.extend((first..=last).map(|line| self.line_text(line)));
        self.reorder(first, last + 1, moved, 1);
    }

    /// Writes `lines` over the lines `first` to `last`, taking the cursor along.
    ///
    /// Moving a block up and moving it down are the same rewrite of one run
    /// of lines read in a different order, so both go through here rather
    /// than through two nearly identical rewrites.
    fn reorder(&mut self, first: usize, last: usize, lines: Vec<String>, step: isize) {
        let selection = self.selection();
        let start = Position::new(first, 0);
        let end = Position::new(last, self.line_len(last));
        let moved = |position: Position| {
            Position::new(position.line.saturating_add_signed(step), position.column)
        };

        self.grouped(|buffer| {
            buffer.replace(start..end, &lines.join("\n"));
            buffer.set_selection(Selection {
                anchor: moved(selection.anchor),
                head: moved(selection.head),
            });
        });
    }

    /// Joins the line below the cursor onto the line the cursor is on.
    pub fn join_lines(&mut self) {
        let lines = self.selection().lines();
        let (first, last) = (*lines.start(), *lines.end());
        let last = if last == first { first } else { last - 1 };
        self.grouped(|buffer| {
            for _ in first..=last {
                if first + 1 >= buffer.line_count() {
                    break;
                }
                let end = Position::new(first, buffer.line_len(first));
                let text = buffer.line_text(first + 1);
                let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
                let separator = if text.trim().is_empty() || end.column == 0 {
                    ""
                } else {
                    " "
                };
                buffer.replace(end..Position::new(first + 1, indent), separator);
                buffer.place(end, false);
            }
        });
    }

    /// Comments every line the selection reaches, or takes the comments off.
    ///
    /// A block is commented when every line of it that holds anything is
    /// already commented, which is the rule that makes the key a toggle
    /// rather than two keys: a half-commented block comments the rest.
    pub fn toggle_comment(&mut self) {
        let Some(token) = self
            .language()
            .and_then(crate::language::Language::line_comment)
        else {
            return;
        };
        let selection = self.selection();
        let lines = selection.lines();
        let filled = lines
            .clone()
            .filter(|line| !self.line_text(*line).trim().is_empty())
            .collect::<Vec<_>>();
        let lines = if filled.is_empty() {
            lines.collect::<Vec<_>>()
        } else {
            filled
        };
        let commented = lines
            .iter()
            .all(|line| self.line_text(*line).trim_start().starts_with(token));
        let margin = lines
            .iter()
            .map(|line| {
                let text = self.line_text(*line);
                text.chars().take_while(|ch| ch.is_whitespace()).count()
            })
            .min()
            .unwrap_or(0);

        self.grouped(|buffer| {
            for line in lines {
                if commented {
                    buffer.uncomment_line(line, token);
                } else {
                    let at = Position::new(line, margin.min(buffer.line_len(line)));
                    buffer.replace(at..at, &format!("{token} "));
                }
            }
            buffer.set_selection(selection);
        });
    }

    /// Takes `token` and the space after it off `line`.
    fn uncomment_line(&mut self, line: usize, token: &str) {
        let text = self.line_text(line);
        let indent = text.chars().take_while(|ch| ch.is_whitespace()).count();
        if !text.trim_start().starts_with(token) {
            return;
        }
        let rest = &text.trim_start()[token.len()..];
        let width = token.chars().count() + usize::from(rest.starts_with(' '));
        self.replace(
            Position::new(line, indent)..Position::new(line, indent + width),
            "",
        );
    }

    /// Takes the last step back, saying whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some(replay) = self.history.undo() else {
            return false;
        };
        self.replay(replay)
    }

    /// Puts the last step taken back, saying whether there was one.
    pub fn redo(&mut self) -> bool {
        let Some(replay) = self.history.redo() else {
            return false;
        };
        self.replay(replay)
    }

    /// Makes the changes of a replayed step and puts its selection back.
    fn replay(&mut self, replay: crate::history::Replay) -> bool {
        for change in &replay.changes {
            let end = change.at.after(&change.before);
            self.apply(change.at..end, &change.after);
        }
        self.set_selection(replay.selection);
        true
    }

    /// Whether there is a step to take back.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Whether there is a step to put back.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Ends the undo step being gathered, so the next change starts its own.
    pub fn commit(&mut self) {
        self.history.commit();
    }

    /// How many undo steps have been made, for [`Self::squash_since`].
    pub fn undo_depth(&self) -> usize {
        self.history.depth()
    }

    /// Makes every undo step since the history was `depth` deep one step.
    ///
    /// A stretch of typing that began with a command is one thing the reader
    /// did, however many pauses it took: taking it back is one keypress.
    pub fn squash_since(&mut self, depth: usize) {
        self.history.squash(depth);
    }

    /// Replaces the text `range` covers with `text`, cursor and history included.
    pub fn replace(&mut self, range: Range<Position>, text: &str) {
        let before = self.selection();
        let Some(change) = self.apply(range, text) else {
            return;
        };
        let head = change.at.after(&change.after);
        self.place_selection(Selection::at(head));
        self.history.record(change, before, self.selection());
    }

    /// Replaces everything the buffer holds, keeping where the cursor was.
    ///
    /// This is what a formatter or a server-side rewrite comes back as: one
    /// replacement of the whole file, taken back in one step, with the
    /// cursor left as near to where it was as the new text allows.
    pub fn set_contents(&mut self, text: &str) {
        let selection = self.selection();
        let last = self.line_count().saturating_sub(1);
        let whole = Position::default()..Position::new(last, self.line_len(last));
        self.grouped(|buffer| {
            buffer.replace(whole, text);
            buffer.set_selection(Selection {
                anchor: buffer.clamped(selection.anchor),
                head: buffer.clamped(selection.head),
            });
        });
    }

    /// Makes `edits` as one step, latest place in the file first.
    ///
    /// Edits arrive in the order a server thought of them and overlap only
    /// by accident; making them back to front means an earlier one never
    /// moves the text a later one was measured against. Equal-position
    /// inserts are applied in reverse so they read in the server's order.
    pub fn apply_edits(&mut self, edits: Vec<(Range<Position>, String)>) {
        let mut edits = edits.into_iter().enumerate().collect::<Vec<_>>();
        edits.sort_by_key(|(index, (range, _))| {
            (std::cmp::Reverse(range.start), std::cmp::Reverse(*index))
        });
        let selection = self.selection();
        self.grouped(|buffer| {
            for (_, (range, text)) in edits {
                buffer.replace(range, &text);
            }
            buffer.set_selection(Selection {
                anchor: buffer.clamped(selection.anchor),
                head: buffer.clamped(selection.head),
            });
        });
    }

    /// Puts a chosen completion in: `text` over `range`, and the `extra`
    /// edits it brings along, as one step, with the cursor left after `text`.
    ///
    /// The extra edits are the server's — the import a name needs, most
    /// often — measured against the text before any of it changed, so they
    /// are made latest place first, and the cursor is carried past each one
    /// made before it. One that overlaps the completion itself is dropped:
    /// it was measured against text the completion replaces.
    ///
    /// Answers the character `text` begins at once everything is in, which
    /// is where a snippet's places are counted from.
    pub fn complete(
        &mut self,
        range: Range<Position>,
        text: &str,
        extra: Vec<(Range<Position>, String)>,
    ) -> usize {
        let mut edits = extra
            .into_iter()
            .filter(|(span, _)| span.end <= range.start || span.start >= range.end)
            .map(|(span, text)| (span, text, false))
            .chain(std::iter::once((range.clone(), text.to_owned(), true)))
            .collect::<Vec<_>>();
        edits.sort_by_key(|(span, _, main)| (std::cmp::Reverse(span.start), !main));
        let length = text.chars().count();
        let mut cursor = None;
        self.grouped(|buffer| {
            for (span, text, main) in edits {
                let start = buffer.char_of(span.start);
                let removed = buffer.char_of(span.end) - start;
                let added = text.chars().count();
                buffer.replace(span, &text);
                cursor = match main {
                    true => Some(start + added),
                    false => cursor.map(|at: usize| (at + added).saturating_sub(removed)),
                };
            }
            if let Some(at) = cursor {
                let at = buffer.position_of(at);
                buffer.set_selection(Selection::at(at));
            }
        });
        cursor.map_or(0, |at: usize| at - length)
    }

    /// Takes the spaces and tabs off the end of every line, as one step.
    pub fn trim_trailing_whitespace(&mut self) {
        let edits = (0..self.line_count())
            .filter_map(|line| {
                let len = self.line_len(line);
                let text = self.line_text(line);
                let kept = text.trim_end_matches([' ', '\t']).chars().count();
                (kept < len).then(|| {
                    (
                        Position::new(line, kept)..Position::new(line, len),
                        String::new(),
                    )
                })
            })
            .collect::<Vec<_>>();
        if !edits.is_empty() {
            self.apply_edits(edits);
        }
    }

    /// Ends the text with a line break, when it holds any and does not.
    pub fn ensure_final_newline(&mut self) {
        let last = self.line_count().saturating_sub(1);
        let len = self.line_len(last);
        if len == 0 {
            return;
        }
        let end = Position::new(last, len);
        self.apply_edits(vec![(end..end, "\n".to_owned())]);
    }

    /// Runs `change`, gathering everything it does into one undo step.
    pub fn grouped(&mut self, change: impl FnOnce(&mut Self)) {
        self.history.begin();
        change(self);
        self.history.end();
    }

    /// Replaces the text `range` covers with `text`, without writing it down.
    ///
    /// This is the floor everything else stands on: the rope, the syntax
    /// tree and the version move here and nowhere else. It is private
    /// because a change that is made without being recorded is a change undo
    /// cannot take back.
    fn apply(&mut self, range: Range<Position>, text: &str) -> Option<Change> {
        let (start, end) = (self.clamped(range.start), self.clamped(range.end));
        let (start, end) = (start.min(end), start.max(end));
        if start == end && text.is_empty() {
            return None;
        }

        let (first, last) = (self.char_of(start), self.char_of(end));
        let start_byte = self.text.char_to_byte(first);
        let old_end_byte = self.text.char_to_byte(last);
        let start_position = point(start, &self.text);
        let old_end_position = point(end, &self.text);
        let removed = self.text.slice(first..last).to_string();

        self.text.remove(first..last);
        self.text.insert(first, text);
        self.shift_places(first, last - first, text.chars().count());

        let head = start.after(text);
        let edit = InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + text.len(),
            start_position,
            old_end_position,
            new_end_position: point(head, &self.text),
        };
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.edit(&edit);
            syntax.parse(&self.text);
        }
        self.version += 1;

        Some(Change {
            at: start,
            before: removed,
            after: text.to_owned(),
        })
    }
}

/// `position` as the row and byte column a syntax tree counts in.
fn point(position: Position, text: &Rope) -> Point {
    let line = position.line.min(text.len_lines().saturating_sub(1));
    let start = text.line_to_char(line);
    let column = text.char_to_byte(start + position.column) - text.char_to_byte(start);
    Point::new(line, column)
}
